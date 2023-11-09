pub use mozak_circuits_derive_core::{
    stark_kind_lambda, stark_lambda, stark_lambda_mut, StarkNameDisplay, StarkSet,
};
pub use plonky2::field::extension::Extendable;
pub use plonky2::hash::hash_types::RichField;
pub use starky::stark::Stark;

pub trait StarkKindLambda<const D: usize> {
    type F: RichField + Extendable<D>;
    type Kind;
    type Output;
    fn call<S>(&mut self, kind: Self::Kind) -> Self::Output
    where
        S: Stark<Self::F, D>;
}

pub trait StarkLambda<const D: usize> {
    type F: RichField + Extendable<D>;
    type Kind;
    type Output;
    fn call<S>(&mut self, stark: &S, kind: Self::Kind) -> Self::Output
    where
        S: Stark<Self::F, D>;
}

pub trait StarkLambdaMut<const D: usize> {
    type F: RichField + Extendable<D>;
    type Kind;
    type Output;
    fn call<S>(&mut self, stark: &mut S, kind: Self::Kind) -> Self::Output
    where
        S: Stark<Self::F, D>;
}
