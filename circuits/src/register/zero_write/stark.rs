use super::columns::RegisterZeroWrite;
use crate::zero_constraints_stark;

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct RegisterZeroWriteStark<F, const D: usize>(PhantomData<F>);

zero_constraints_stark!(RegisterZeroWrite, RegisterZeroWriteStark);
