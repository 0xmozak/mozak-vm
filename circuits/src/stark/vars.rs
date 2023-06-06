use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;

#[derive(Debug, Copy, Clone)]
pub struct StarkEvaluationVars<'a, F, P, const COLUMNS: usize>
where
    F: Field,
    P: PackedField<Scalar = F>,
{
    pub local_values: &'a [P; COLUMNS],
    pub next_values: &'a [P; COLUMNS],
}
