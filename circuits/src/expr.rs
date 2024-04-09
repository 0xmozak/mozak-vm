use std::marker::PhantomData;
use std::panic::Location;

use derive_more::Display;
use expr::{BinOp, Evaluator, Expr};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};

struct CircuitBuilderEvaluator<'a, F, const D: usize>
where
    F: RichField,
    F: Extendable<D>, {
    builder: &'a mut CircuitBuilder<F, D>,
}

impl<'a, F, const D: usize> Evaluator<ExtensionTarget<D>> for CircuitBuilderEvaluator<'a, F, D>
where
    F: RichField,
    F: Extendable<D>,
{
    fn bin_op(
        &mut self,
        op: &BinOp,
        left: ExtensionTarget<D>,
        right: ExtensionTarget<D>,
    ) -> ExtensionTarget<D> {
        match op {
            BinOp::Add => self.builder.add_extension(left, right),
            BinOp::Sub => self.builder.sub_extension(left, right),
            BinOp::Mul => self.builder.mul_extension(left, right),
        }
    }

    fn constant(&mut self, value: i64) -> ExtensionTarget<D> {
        let f = F::from_noncanonical_i64(value);
        self.builder.constant_extension(f.into())
    }
}

#[derive(Default)]
struct PackedFieldEvaluator<P, const D: usize, const D2: usize> {
    _marker: PhantomData<P>,
}

impl<F, FE, P, const D: usize, const D2: usize> Evaluator<P> for PackedFieldEvaluator<P, D, D2>
where
    F: RichField,
    F: Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>,
{
    fn bin_op(&mut self, op: &BinOp, left: P, right: P) -> P {
        match op {
            BinOp::Add => left + right,
            BinOp::Sub => left - right,
            BinOp::Mul => left * right,
        }
    }

    fn constant(&mut self, value: i64) -> P { P::from(FE::from_noncanonical_i64(value)) }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Constraint<E> {
    constraint_type: ConstraintType,
    location: &'static Location<'static>,
    constraint: E,
}

impl<E> Constraint<E> {
    fn map<B, F>(self, mut f: F) -> Constraint<B>
    where
        F: FnMut(E) -> B, {
        Constraint {
            constraint_type: self.constraint_type,
            location: self.location,
            constraint: f(self.constraint),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Default, Debug, Display)]
enum ConstraintType {
    FirstRow,
    #[default]
    Always,
    Transition,
}

pub struct ConstraintBuilder<E> {
    constraints: Vec<Constraint<E>>,
}
impl<E> Default for ConstraintBuilder<E> {
    fn default() -> Self {
        Self {
            constraints: Vec::default(),
        }
    }
}

impl<E> From<Vec<Constraint<E>>> for ConstraintBuilder<E> {
    fn from(constraints: Vec<Constraint<E>>) -> Self { Self { constraints } }
}

impl<E> ConstraintBuilder<E> {
    #[track_caller]
    fn constraint(&mut self, constraint: E, ty: ConstraintType) {
        let location = Location::caller();
        let c = Constraint {
            constraint_type: ty,
            location,
            constraint,
        };
        self.constraints.push(c);
    }

    #[track_caller]
    pub fn first_row(&mut self, constraint: E) {
        self.constraint(constraint, ConstraintType::FirstRow);
    }

    #[track_caller]
    pub fn always(&mut self, constraint: E) { self.constraint(constraint, ConstraintType::Always); }

    #[track_caller]
    pub fn transition(&mut self, constraint: E) {
        self.constraint(constraint, ConstraintType::Transition);
    }

    #[must_use]
    pub fn collect(self) -> Vec<Constraint<E>> { self.constraints }
}

pub fn build_ext<F, const D: usize>(
    cb: ConstraintBuilder<Expr<'_, ExtensionTarget<D>>>,
    circuit_builder: &mut CircuitBuilder<F, D>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) where
    F: RichField,
    F: Extendable<D>, {
    let mut evaluator = CircuitBuilderEvaluator {
        builder: circuit_builder,
    };

    let evaluated = cb
        .constraints
        .into_iter()
        .map(|c| c.map(|constraint| evaluator.eval(constraint)))
        .collect::<Vec<_>>();

    evaluated.into_iter().for_each(|c| {
        (match c.constraint_type {
            ConstraintType::FirstRow => RecursiveConstraintConsumer::constraint_first_row,
            ConstraintType::Always => RecursiveConstraintConsumer::constraint,
            ConstraintType::Transition => RecursiveConstraintConsumer::constraint_transition,
        })(yield_constr, circuit_builder, c.constraint)
    });
}

pub fn build_packed<F, FE, P, const D: usize, const D2: usize>(
    cb: ConstraintBuilder<Expr<'_, P>>,
    yield_constr: &mut ConstraintConsumer<P>,
) where
    F: RichField,
    F: Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    let mut evaluator = PackedFieldEvaluator::default();
    let evaluated = cb
        .constraints
        .into_iter()
        .map(|c| c.map(|constraint| evaluator.eval(constraint)))
        .collect::<Vec<_>>();

    let mozak_stark_debug = std::env::var("MOZAK_STARK_DEBUG").is_ok();
    for c in evaluated {
        if mozak_stark_debug && !c.constraint.is_zeros() {
            log::error!(
                "ConstraintConsumer - DEBUG trace (non-zero-constraint): {}",
                c.location
            );
        }

        (match c.constraint_type {
            ConstraintType::FirstRow => ConstraintConsumer::constraint_first_row,
            ConstraintType::Always => ConstraintConsumer::constraint,
            ConstraintType::Transition => ConstraintConsumer::constraint_transition,
        })(yield_constr, c.constraint);
    }
}
