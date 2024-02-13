use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::{HashOutTarget, RichField};
use plonky2::iop::target::Target;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2_crypto::u32::arithmetic_u32::U32Target;

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

pub fn biguint_le_u32_target_to_le_u8_target<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    biguint_target: &[U32Target; 8],
) -> [Target; 32]
where
    F: RichField + Extendable<D>, {
    let target_arr = builder.add_virtual_target_arr::<32>();
    let zero = builder.zero();
    let base = builder.constant(F::from_canonical_u16(1 << 8));
    for i in 0..8 {
        let u32_target = target_arr[4 * i..4 * i + 4]
            .iter()
            .rev()
            .fold(zero, |acc, limb| builder.mul_add(acc, base, *limb));
        builder.connect(u32_target, biguint_target[i].0);
    }
    target_arr
}

pub fn biguint_be_u32_target_to_le_u8_target<F, const D: usize>(
    builder: &mut CircuitBuilder<F, D>,
    biguint_target: &[U32Target; 8],
) -> [Target; 32]
where
    F: RichField + Extendable<D>, {
    let target_arr = builder.add_virtual_target_arr::<32>();
    let zero = builder.zero();
    let base = builder.constant(F::from_canonical_u16(1 << 8));
    for i in 0..8 {
        let u32_target = target_arr[4 * i..4 * i + 4]
            .iter()
            .fold(zero, |acc, limb| builder.mul_add(acc, base, *limb));
        builder.connect(u32_target, biguint_target[i].0);
    }
    target_arr
}
