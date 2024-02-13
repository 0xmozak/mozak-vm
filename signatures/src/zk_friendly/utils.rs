use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOutTarget, RichField};
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;

pub fn get_hashout<F: RichField + Extendable<D>, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    limbs: &[Target; 32],
) -> HashOutTarget {
    let hash_out_target = builder.add_virtual_hash();
    let zero = builder.zero();
    let base = builder.constant(F::from_canonical_u16(1 << 8));
    for i in 0..4 {
        let u64_target = limbs[8 * i..8 * i + 8]
            .iter()
            .rev()
            .fold(zero, |acc, limb| builder.mul_add(acc, base, *limb));
        builder.connect(hash_out_target.elements[i], u64_target);
    }
    hash_out_target
}
