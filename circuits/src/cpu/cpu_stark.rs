use std::marker::PhantomData;
use anyhow::Result;
use plonky2::field::extension::FieldExtension;
use plonky2::field::packed::PackedField;
use plonky2::{hash::hash_types::RichField, field::extension::Extendable};
use crate::stark::constraint_consumer::ConstraintConsumer;
use crate::stark::stark::Stark;
use crate::stark::vars::StarkEvaluationVars;
use super::columns::NUM_CPU_COLS;



#[derive(Copy, Clone, Default)]
pub struct CpuStark<F, const D: usize> {
    compress_challenge: Option<F>,
    pub f: PhantomData<F>,
}

impl<F: RichField, const D: usize> CpuStark<F, D> {
    pub fn set_compress_challenge(&mut self, challenge: F) -> Result<()> {
        assert!(self.compress_challenge.is_none(), "already set?"); 
        self.compress_challenge = Some(challenge);
        Ok(())
    }
    pub fn get_compress_challenge(&self) -> Option<F> {
        self.compress_challenge
    }
}

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for CpuStark<F, D> {
    const COLUMNS: usize = NUM_CPU_COLS;
    fn eval_packed_generic<FE, P, const D2: usize>(&self, vars: StarkEvaluationVars<FE, P, NUM_CPU_COLS>, yield_constr: &mut ConstraintConsumer<P>,) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>,
    {
        // CPU related constraints here..
    }
}
