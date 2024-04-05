use super::columns::RegisterZeroRead;
use crate::zero_constraints_stark;

#[derive(Clone, Copy, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct RegisterZeroReadStark<F, const D: usize>(PhantomData<F>);

zero_constraints_stark!(RegisterZeroRead, RegisterZeroReadStark);
