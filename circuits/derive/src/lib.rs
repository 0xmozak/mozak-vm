use std::fmt::{Debug, Display};

pub use mozak_circuits_derive_core::{
    stark_kind_lambda, stark_lambda, stark_lambda_mut, StarkNameDisplay, StarkSet,
};
pub use plonky2::field::extension::Extendable;
pub use plonky2::hash::hash_types::RichField;
pub use starky::stark::Stark;

pub trait HasNamedColumns {
    type Columns;
}

pub trait ExtendedStark<F: RichField + Extendable<D>, const D: usize>:
    Stark<F, D> + Display + HasNamedColumns
where
    Self::Columns: FromIterator<F> + Debug, {
}
impl<F: RichField + Extendable<D>, const D: usize, S: Stark<F, D> + Display + HasNamedColumns>
    ExtendedStark<F, D> for S
where
    S::Columns: FromIterator<F> + Debug,
{
}

pub trait StarkKindLambda<const D: usize> {
    type F: RichField + Extendable<D>;
    type Kind;
    type Output;
    fn call<S>(&mut self, kind: Self::Kind) -> Self::Output
    where
        S: ExtendedStark<Self::F, D>,
        S::Columns: FromIterator<Self::F> + Debug;
}

pub trait StarkLambda<const D: usize> {
    type F: RichField + Extendable<D>;
    type Kind;
    type Output;
    fn call<S>(&mut self, stark: &S, kind: Self::Kind) -> Self::Output
    where
        S: ExtendedStark<Self::F, D>,
        S::Columns: FromIterator<Self::F> + Debug;
}

pub trait StarkLambdaMut<const D: usize> {
    type F: RichField + Extendable<D>;
    type Kind;
    type Output;
    fn call<S>(&mut self, stark: &mut S, kind: Self::Kind) -> Self::Output
    where
        S: ExtendedStark<Self::F, D>,
        S::Columns: FromIterator<Self::F> + Debug;
}
