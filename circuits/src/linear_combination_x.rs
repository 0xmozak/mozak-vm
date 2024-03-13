// use core::iter::Sum;
use core::ops::Neg;

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
#[derive(Clone, Debug, Default)]
pub struct ColumnX<C> {
    /// Linear combination of the local row
    lv_linear_combination: C,
    /// Linear combination of the next row
    nv_linear_combination: C,
    /// Constant of linear combination
    constant: i64,
}

impl<C> Neg for ColumnX<C>
where
    C: Neg<Output = C>,
{
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            lv_linear_combination: -self.lv_linear_combination,
            nv_linear_combination: -self.nv_linear_combination,
            constant: self.constant.checked_neg().expect("negation overflow"),
        }
    }
}

// impl Add<Self> for Column {
//     type Output = Self;

//     #[allow(clippy::similar_names)]
//     fn add(self, other: Self) -> Self {

//         let add_lc = |mut slc: Vec<(usize, i64)>, mut rlc: Vec<(usize, i64)>|
// {             slc.sort_by_key(|&(col_idx, _)| col_idx);
//             rlc.sort_by_key(|&(col_idx, _)| col_idx);
//             slc.into_iter()
//                 .merge_join_by(rlc, |(l, _), (r, _)| l.cmp(r))
//                 .map(|item| {
//                     item.reduce(|(idx0, c0), (idx1, c1)| {
//                         assert_eq!(idx0, idx1);
//                         (idx0, c0 + c1)
//                     })
//                 })
//                 .collect()
//         };

//         Self {
//             lv_linear_combination: add_lc(self.lv_linear_combination,
// other.lv_linear_combination),             nv_linear_combination:
// add_lc(self.nv_linear_combination, other.nv_linear_combination),
// constant: self.constant + other.constant,         }
//     }
// }
