use std::iter::zip;

use iter_fixed::IntoIteratorFixed;
use itertools::{chain, Itertools};
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{
    HashOut, HashOutTarget, MerkleCapTarget, RichField, NUM_HASH_OUT_ELTS,
};
use plonky2::hash::poseidon2::Poseidon2Hash;
use plonky2::iop::target::{BoolTarget, Target};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::VerifierCircuitTarget;
use plonky2::plonk::config::Hasher;

pub mod accumulate_event;
pub mod bounded;
pub mod make_tree;
pub mod merge;
pub mod propagate;
pub mod state_from_event;
pub mod state_update;
pub mod summarized;
pub mod unbounded;
pub mod unpruned;
pub mod verify_address;
pub mod verify_event;

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum EventType {
    Write = 0,
    Ensure = 1,
    Read = 2,
    GiveOwner = 3,
    TakeOwner = 4,
    CreditDelta = 5,
}

impl EventType {
    fn constant<F, const D: usize>(self, builder: &mut CircuitBuilder<F, D>) -> Target
    where
        F: RichField + Extendable<D>, {
        builder.constant(F::from_canonical_u64(self as u64))
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Event<F> {
    owner: [F; 4],
    ty: EventType,
    address: u64,
    value: [F; 4],
}

impl<F: RichField> Event<F> {
    pub fn bytes(self) -> impl Iterator<Item = F> {
        chain!(
            self.owner,
            [self.ty as u64, self.address].map(F::from_canonical_u64),
            self.value
        )
    }

    pub fn hash(self) -> HashOut<F> {
        let bytes = self.bytes().collect_vec();
        Poseidon2Hash::hash_no_pad(&bytes)
    }

    pub fn byte_wise_hash(self) -> HashOut<F> {
        let bytes = self
            .bytes()
            .flat_map(|v| v.to_canonical_u64().to_le_bytes())
            .map(|v| F::from_canonical_u8(v))
            .collect_vec();
        Poseidon2Hash::hash_no_pad(&bytes)
    }
}

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

/// Finds the index of a target `t` in an array. Useful for getting and
/// labelling the indicies for public inputs.
fn find_target(targets: &[Target], t: Target) -> usize {
    targets
        .iter()
        .position(|&pi| pi == t)
        .expect("target not found")
}

/// Finds the index of a boolean target `t` in an array. Useful for getting and
/// labelling the indicies for public inputs.
fn find_bool(targets: &[Target], t: BoolTarget) -> usize { find_target(targets, t.target) }

/// Finds the indices of targets `ts` in an array. Useful for getting and
/// labelling the indicies for public inputs.
fn find_targets<const N: usize>(targets: &[Target], ts: [Target; N]) -> [usize; N] {
    ts.map(|t| find_target(targets, t))
}

/// Finds the indices of the target elements of `ts` in an array. Useful for
/// getting and labelling the indicies for public inputs.
fn find_hash(targets: &[Target], ts: HashOutTarget) -> [usize; NUM_HASH_OUT_ELTS] {
    find_targets(targets, ts.elements)
}

/// Connects `x` to `v` if `maybe_v` is true
fn maybe_connect<F: RichField + Extendable<D>, const D: usize, const N: usize>(
    builder: &mut CircuitBuilder<F, D>,
    x: [Target; N],
    maybe_v: BoolTarget,
    v: [Target; N],
) {
    // Loop over the limbs
    for (parent, child) in zip(x, v) {
        let child = builder.select(maybe_v, child, parent);
        builder.connect(parent, child);
    }
}

fn hash_event<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    owner: [Target; 4],
    ty: Target,
    address: Target,
    value: [Target; 4],
) -> HashOutTarget {
    builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(chain!(owner, [ty, address], value,).collect())
}

fn byte_wise_hash_event<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    owner: [Target; 4],
    ty: Target,
    address: Target,
    value: [Target; 4],
) -> HashOutTarget {
    byte_wise_hash(builder, chain!(owner, [ty, address], value).collect())
}

fn split_bytes<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    mut source: Target,
) -> [Target; 8] {
    [(); 8]
        .into_iter_fixed()
        .enumerate()
        .map(|(i, ())| {
            if i == 7 {
                source
            } else {
                let (lo, rest) = builder.split_low_high(source, 8, 64 - 8 * i);
                source = rest;
                lo
            }
        })
        .collect()
}

fn byte_wise_hash<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    inputs: Vec<Target>,
) -> HashOutTarget {
    let bytes = inputs
        .into_iter()
        .flat_map(|v| split_bytes(builder, v))
        .collect();
    builder.hash_n_to_hash_no_pad::<Poseidon2Hash>(bytes)
}
