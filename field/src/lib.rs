use p3_field::PrimeField;
use p3_mersenne_31::Mersenne31 as Fp;
// use p3_goldilocks::Goldilocks as Fp;

// Default field element
pub trait BaseField: PrimeField {}

// Default field element
// Currently set as mersenne field
pub type FieldElement = Fp;

