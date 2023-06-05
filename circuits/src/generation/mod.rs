pub mod cpu;

use mozak_vm::vm::Row;
use plonky2::{
    field::{extension::Extendable, polynomial::PolynomialValues},
    hash::hash_types::RichField,
};

pub fn generate_traces<F: RichField + Extendable<D>, const D: usize>(
    _state_rows: Vec<Row>,
) -> [Vec<PolynomialValues<F>>; 1] {
    [vec![]]
}
