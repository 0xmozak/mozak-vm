#![feature(generic_const_exprs)]

use plonky2::field::goldilocks_field::GoldilocksField;

pub mod columns;
pub mod rangecheck_stark;

pub const U8_BITS_MASK: u64 = 0xff;
pub const U16_BITS_MASK: u64 = 0xffff;

pub fn split_u16_limbs_from_field(value: &GoldilocksField) -> (u64, u64) {
    let input = value.0;

    let limb_lo = input & U16_BITS_MASK;
    let limb_hi = input >> 16 & U16_BITS_MASK;

    (limb_lo, limb_hi)
}
