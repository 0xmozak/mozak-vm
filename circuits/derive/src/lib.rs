pub use mozak_circuits_derive_core::{stark_kind_lambda, StarkNameDisplay, StarkSet};
pub use plonky2::field::extension::Extendable;
pub use plonky2::hash::hash_types::RichField;
pub use starky::stark::Stark;

pub trait StarkKindFnMut<const D: usize> {
    type F: RichField + Extendable<D>;
    type Kind;
    type Output;
    fn call<S>(&mut self, kind: Self::Kind) -> Self::Output
    where
        S: Stark<Self::F, D>;
}

/*

pub trait Extendable<const D: usize>{}
pub trait RichField {}
pub trait Stark<F: RichField + Extendable<D>, const D: usize>{}

struct FakeField;
impl<const D: usize> Extendable<D> for FakeField {}
impl RichField for FakeField {}

struct FakeStark;
impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for FakeStark {}
struct FakeStark2;
impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for FakeStark2 {}

#[test]
fn test_stark_kind_lambda() {
    fn foo<F: RichField + Extendable<D>, const D: usize, T>(t: T) {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum TableKind {
            Foo,
        }
        let foo = 10;
        let mut lambda = stark_kind_lambda!(F, D, <T>, (foo, t): (u32, T), |captures, kind: TableKind| {
            assert_eq!(captures.0, 10);
            assert_eq!(kind, TableKind::Foo);
        });
        lambda.call::<FakeStark>(TableKind::Foo);
        lambda.call::<FakeStark2>(TableKind::Foo);
    }
    foo::<FakeField, 10, _>(56f32);
}
*/

