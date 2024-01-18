use itertools::Itertools;
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOutTarget, MerkleCapTarget, RichField};
use plonky2::iop::target::BoolTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::VerifierCircuitTarget;

pub mod make_tree;
pub mod summarized;
pub mod unbounded;
pub mod unpruned;

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
