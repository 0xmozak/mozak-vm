use iter_fixed::IntoIteratorFixed;
use itertools::Itertools;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOutTarget, MerkleCapTarget, RichField, NUM_HASH_OUT_ELTS};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::VerifierCircuitTarget;

pub mod make_tree;
pub mod merge;
pub mod state_from_event;
pub mod state_update;
pub mod summarized;
pub mod unbounded;
pub mod unpruned;
pub mod verify_address;

/// Computes `if b { h0 } else { h1 }`.
pub(crate) fn select_hash<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    b: BoolTarget,
    h0: HashOutTarget,
    h1: HashOutTarget,
) -> HashOutTarget
where
    F: RichField + Extendable<D>, {
    HashOutTarget {
        elements: core::array::from_fn(|i| builder.select(b, h0.elements[i], h1.elements[i])),
    }
}

/// Computes `if b { cap0 } else { cap1 }`.
pub(crate) fn select_cap<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    b: BoolTarget,
    cap0: &MerkleCapTarget,
    cap1: &MerkleCapTarget,
) -> MerkleCapTarget
where
    F: RichField + Extendable<D>, {
    assert_eq!(cap0.0.len(), cap1.0.len());
    MerkleCapTarget(
        cap0.0
            .iter()
            .zip_eq(&cap1.0)
            .map(|(h0, h1)| select_hash(builder, b, *h0, *h1))
            .collect(),
    )
}

/// Computes `if b { v0 } else { v1 }`.
pub(crate) fn select_verifier<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    b: BoolTarget,
    v0: &VerifierCircuitTarget,
    v1: &VerifierCircuitTarget,
) -> VerifierCircuitTarget
where
    F: RichField + Extendable<D>, {
    VerifierCircuitTarget {
        constants_sigmas_cap: select_cap(
            builder,
            b,
            &v0.constants_sigmas_cap,
            &v1.constants_sigmas_cap,
        ),
        circuit_digest: select_hash(builder, b, v0.circuit_digest, v1.circuit_digest),
    }
}

/// Reduce a hash-sized group of booleans by `&&`ing them together
fn and_helper<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    bools: [BoolTarget; 4],
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let bools = [
        builder.and(bools[0], bools[1]),
        builder.and(bools[2], bools[3]),
    ];
    builder.and(bools[0], bools[1])
}

/// Reduce a hash-sized group of booleans by `||`ing them together
fn or_helper<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    bools: [BoolTarget; 4],
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let bools = [
        builder.or(bools[0], bools[1]),
        builder.or(bools[2], bools[3]),
    ];
    builder.or(bools[0], bools[1])
}

/// Computes `h0 == h1`.
fn hashes_equal<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    h0: HashOutTarget,
    h1: HashOutTarget,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let eq = h0
        .elements
        .into_iter_fixed()
        .zip(h1.elements)
        .map(|(h0, h1)| builder.is_equal(h0, h1))
        .collect();
    and_helper(builder, eq)
}

/// Computes `h0 != ZERO`.
fn hash_is_nonzero<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    h0: impl Into<HashOutTarget>,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let zero = h0
        .into()
        .elements
        .into_iter_fixed()
        .map(|h0| builder.is_nonzero(h0))
        .collect();
    // If any elements are non-zero, then it's non-zero
    or_helper(builder, zero)
}

/// Computes `h0 == ZERO`.
fn hash_is_zero<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    h0: HashOutTarget,
) -> BoolTarget
where
    F: RichField + Extendable<D>, {
    let zero = h0
        .elements
        .into_iter_fixed()
        .map(|h0| {
            let non_zero = builder.is_nonzero(h0);
            builder.not(non_zero)
        })
        .collect();
    // All numbers must be zero to be zero
    and_helper(builder, zero)
}

/// Hash left and right together if both are present, otherwise forward one
fn hash_or_forward<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    left_present: BoolTarget,
    left: [Target; NUM_HASH_OUT_ELTS],
    right_present: BoolTarget,
    right: [Target; NUM_HASH_OUT_ELTS],
) -> HashOutTarget
where
    F: RichField + Extendable<D>, {
    let both_present = builder.and(left_present, right_present);

    // Construct the hash of [left, right]
    let hash_both =
        builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(left.into_iter().chain(right).collect());

    // Construct the forwarding "hash".
    let hash_absent = left
        .into_iter_fixed()
        .zip(right)
        // Since absent sides will be zero, we can just sum.
        .map(|(l, r)| builder.add(l, r))
        .collect();
    let hash_absent = HashOutTarget {
        elements: hash_absent,
    };

    // Select the hash based on presence
    select_hash(builder, both_present, hash_both, hash_absent)
}

/// `hash_or_forward` but using non-zero to determine presence
fn hash_or_forward_zero<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    left: [Target; NUM_HASH_OUT_ELTS],
    right: [Target; NUM_HASH_OUT_ELTS],
) -> HashOutTarget
where
    F: RichField + Extendable<D>, {
    let left_non_zero = hash_is_nonzero(builder, left);
    let right_non_zero = hash_is_nonzero(builder, right);

    // Select the hash based on presence
    hash_or_forward(builder, left_non_zero, left, right_non_zero, right)
}

/// Guarantee at least one `BoolTarget` is `true`.
/// Does nothing if no targets are provided
fn at_least_one_true<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    targets: impl IntoIterator<Item = BoolTarget>,
) where
    F: RichField + Extendable<D>, {
    let mut targets = targets.into_iter();
    let Some(first) = targets.next() else { return };

    // Sum all the booleans
    let total = targets.fold(first.target, |total, i| builder.add(total, i.target));

    // If all booleans were 0, self-division will be unsatisfiable
    builder.div(total, total);
}
