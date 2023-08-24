use core::iter::{Product, Sum};
use core::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

use plonky2::field::packed::PackedField;
use plonky2::field::types::{Field, Field64, PrimeField, PrimeField64, Sample};
use plonky2::util::serialization::gate_serialization::default;
use serde::{Deserialize, Serialize};
use core::fmt::{self, Debug, Display, Formatter};
use core::ops::{DivAssign};
use plonky2::field::goldilocks_field::GoldilocksField;



#[derive(Copy, Eq, PartialEq, Default, Debug, Clone, Serialize, Deserialize, Hash)]

pub struct Degrees {
    pub degree: i64,
}

unsafe impl PackedField for Degrees {
  type Scalar = GoldilocksField;
  const WIDTH: usize = 0;
  const ZEROS: Self = Degrees { degree: 0 };
  const ONES: Self = Degrees { degree: 0 };
}

impl Add for Degrees {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Degrees {
            degree: self.degree.max(rhs.degree),
        }
    }
}

impl AddAssign for Degrees {
  #[inline]
  fn add_assign(&mut self, rhs: Self) {
      *self = *self + rhs;
  }
}

impl Sub for Degrees {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Degrees {
            degree: self.degree.max(rhs.degree),
        }
    }
}

impl SubAssign for Degrees {
    fn sub_assign(&mut self, rhs: Self) { self.degree = self.degree.max(rhs.degree); }
}

impl SubAssign<()> for Degrees {
    #[inline]
    fn sub_assign(&mut self, rhs: ()) {}
}

// impl Sum for Degrees {
// }
