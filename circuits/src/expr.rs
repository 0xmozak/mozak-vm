use std::panic::Location;

use derive_more::Display;
pub use expr::PureEvaluator;
use expr::{BinOp, Cached, Evaluator, Expr, UnaOp};
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

impl<'a, F, const D: usize> Evaluator<'a, ExtensionTarget<D>> for CircuitBuilderEvaluator<'a, F, D>
where
    F: RichField,
    F: Extendable<D>,
{
    fn bin_op(
        &mut self,
        op: BinOp,
        left: ExtensionTarget<D>,
        right: ExtensionTarget<D>,
    ) -> ExtensionTarget<D> {
        match op {
            BinOp::Add => self.builder.add_extension(left, right),
            BinOp::Sub => self.builder.sub_extension(left, right),
            BinOp::Mul => self.builder.mul_extension(left, right),
        }
    }

    fn una_op(&mut self, op: UnaOp, expr: ExtensionTarget<D>) -> ExtensionTarget<D> {
        match op {
            UnaOp::Neg => {
                let neg_one = self.builder.neg_one();
                self.builder.scalar_mul_ext(neg_one, expr)
            }
        }
    }

    fn constant(&mut self, value: i64) -> ExtensionTarget<D> {
        let f = F::from_noncanonical_i64(value);
        self.builder.constant_extension(f.into())
    }
}

#[must_use]
pub fn packed_field_evaluator<F, FE, P, const D: usize, const D2: usize>() -> PureEvaluator<P>
where
    F: RichField,
    F: Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    fn convert<F, FE, P, const D: usize, const D2: usize>(value: i64) -> P
    where
        F: RichField,
        F: Extendable<D>,
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        P::from(FE::from_noncanonical_i64(value))
    }
    PureEvaluator(convert)
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Constraint<E> {
    constraint_type: ConstraintType,
    location: &'static Location<'static>,
    term: E,
}

impl<E> Constraint<E> {
    fn map<B, F>(self, mut f: F) -> Constraint<B>
    where
        F: FnMut(E) -> B, {
        Constraint {
            constraint_type: self.constraint_type,
            location: self.location,
            term: f(self.term),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Default, Debug, Display)]
enum ConstraintType {
    FirstRow,
    #[default]
    Always,
    Transition,
    LastRow,
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
    fn constraint(&mut self, term: E, constraint_type: ConstraintType) {
        self.constraints.push(Constraint {
            constraint_type,
            location: Location::caller(),
            term,
        });
    }

    #[track_caller]
    pub fn first_row(&mut self, constraint: E) {
        self.constraint(constraint, ConstraintType::FirstRow);
    }

    #[track_caller]
    pub fn last_row(&mut self, constraint: E) {
        self.constraint(constraint, ConstraintType::LastRow);
    }

    #[track_caller]
    pub fn always(&mut self, constraint: E) { self.constraint(constraint, ConstraintType::Always); }

    #[track_caller]
    pub fn transition(&mut self, constraint: E) {
        self.constraint(constraint, ConstraintType::Transition);
    }
}

pub fn build_ext<F, const D: usize>(
    cb: ConstraintBuilder<Expr<'_, ExtensionTarget<D>>>,
    circuit_builder: &mut CircuitBuilder<F, D>,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
) where
    F: RichField,
    F: Extendable<D>, {
    for constraint in cb.constraints {
        let mut evaluator = Cached::from(CircuitBuilderEvaluator {
            builder: circuit_builder,
        });
        let constraint = constraint.map(|constraint| evaluator.eval(constraint));
        (match constraint.constraint_type {
            ConstraintType::FirstRow => RecursiveConstraintConsumer::constraint_first_row,
            ConstraintType::Always => RecursiveConstraintConsumer::constraint,
            ConstraintType::Transition => RecursiveConstraintConsumer::constraint_transition,
            ConstraintType::LastRow => RecursiveConstraintConsumer::constraint_last_row,
        })(yield_constr, circuit_builder, constraint.term);
    }
}

pub fn build_packed<F, FE, P, const D: usize, const D2: usize>(
    cb: ConstraintBuilder<Expr<'_, P>>,
    yield_constr: &mut ConstraintConsumer<P>,
) where
    F: RichField,
    F: Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    let mut evaluator = Cached::from(packed_field_evaluator());
    let evaluated = cb
        .constraints
        .into_iter()
        .map(|c| c.map(|constraint| evaluator.eval(constraint)))
        .collect::<Vec<_>>();

    for c in evaluated {
        (match c.constraint_type {
            ConstraintType::FirstRow => ConstraintConsumer::constraint_first_row,
            ConstraintType::Always => ConstraintConsumer::constraint,
            ConstraintType::Transition => ConstraintConsumer::constraint_transition,
            ConstraintType::LastRow => ConstraintConsumer::constraint_last_row,
        })(yield_constr, c.term);
    }
}
