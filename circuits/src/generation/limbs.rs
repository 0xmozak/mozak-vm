use plonky2::hash::hash_types::RichField;

use crate::limbs::columns::Limbs;
use crate::rangecheck::columns::{self};

pub fn generate_limbs_trace<F: RichField>(
    rangecheck_trace: &[Vec<F>; columns::NUM_RC_COLS],
) -> Vec<Limbs<F>> {
    unimplemented!()
}
