// use core::iter::Sum;
// use core::ops::{Add, Mul, Neg, Sub};
// use std::borrow::Borrow;
// use std::ops::Index;

// use itertools::{chain, izip, Itertools};
// use plonky2::field::extension::{Extendable, FieldExtension};
// use plonky2::field::packed::PackedField;
// use plonky2::field::polynomial::PolynomialValues;
// use plonky2::field::types::Field;
// use plonky2::hash::hash_types::RichField;
// use plonky2::iop::ext_target::ExtensionTarget;
// use plonky2::plonk::circuit_builder::CircuitBuilder;

/// Represent a linear combination of columns.
// #[derive(Clone, Debug, Default)]
pub struct ColumnX<const SIZE: usize, C: From<[i64; SIZE]> + Into<[i64; SIZE]>> {
    /// Linear combination of the local row
    lv_linear_combination: C,
    /// Linear combination of the next row
    nv_linear_combination: C,
    /// Constant of linear combination
    constant: i64,
}

// impl<const SIZE: usize, C: From<[i64; SIZE]> + Into<[i64; SIZE]>> Neg for ColumnX<SIZE, C> {
//   type Output = Self;

//   fn neg(self) -> Self::Output {
//       Self {
//           lv_linear_combination: self
//               .lv_linear_combination
//               .into()
//               .into_iter()
//               .map(Neg::neg)
//               // .collect_vec()
//               .try_into()
//               .unwrap(),
//           nv_linear_combination: self
//               .nv_linear_combination
//               .into()
//               .into_iter()
//               .map(Neg::neg)
//               // .collect_vec()
//               .try_into()
//               .unwrap(),
//           constant: -self.constant,
//       }
//   }
// }
