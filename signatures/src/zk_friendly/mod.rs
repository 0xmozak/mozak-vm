use plonky2::field::goldilocks_field::GoldilocksField;
use plonky2::field::types::Field;

pub mod keccak256;
pub mod poseidon;

/// Num of u8limbs required to hold 256 bits
pub const NUM_LIMBS_U8: usize = 32;

/// This would be hash of 256 bit long private key
pub struct PublicKey {
    limbs: [u8; NUM_LIMBS_U8],
}

/// 256 bit private key
pub struct PrivateKey {
    limbs: [u8; NUM_LIMBS_U8],
}

/// This would be poseidon hash of the message being signed
pub struct Message {
    limbs: [u8; NUM_LIMBS_U8],
}
macro_rules! impl_limbs {
    ($i: ident) => {
        impl $i {
            pub fn new(limbs: [u8; NUM_LIMBS_U8]) -> Self { Self { limbs } }

            pub fn get_limbs(&self) -> [u8; NUM_LIMBS_U8] { self.limbs }

            pub fn get_limbs_field(&self) -> [GoldilocksField; NUM_LIMBS_U8] {
                self.get_limbs().map(GoldilocksField::from_canonical_u8)
            }
        }
    };
}

impl_limbs!(PublicKey);
impl_limbs!(PrivateKey);
impl_limbs!(Message);
