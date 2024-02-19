use std::marker::PhantomData;

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

    fn one(&mut self) -> ExtensionTarget<D> { self.builder.one_extension() }
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

    fn one(&mut self) -> P { P::ONES }
}

pub struct Constraint<E> {
    constraint_type: ConstraintType,
    constraint: E,
}

enum ConstraintType {
    ConstraintFirstRow,
    Constraint,
    ConstraintTransition,
}

pub struct ConstraintBuilderExt<E> {
    constraints: Vec<Constraint<E>>,
}

impl<E> Default for ConstraintBuilderExt<E> {
    fn default() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }
}

impl<E> From<Vec<Constraint<E>>> for ConstraintBuilderExt<E> {
    fn from(constraints: Vec<Constraint<E>>) -> Self { Self { constraints } }
}

impl<E> ConstraintBuilderExt<E> {
    pub fn constraint_first_row(&mut self, constraint: E) {
        let c = Constraint {
            constraint_type: ConstraintType::ConstraintFirstRow,
            constraint,
        };
        self.constraints.push(c)
    }

    pub fn constraint(&mut self, constraint: E) {
        let c = Constraint {
            constraint_type: ConstraintType::Constraint,
            constraint,
        };
        self.constraints.push(c)
    }

    pub fn constraint_transition(&mut self, constraint: E) {
        let c = Constraint {
            constraint_type: ConstraintType::ConstraintTransition,
            constraint,
        };
        self.constraints.push(c)
    }

    pub fn collect(self) -> Vec<Constraint<E>> { self.constraints }
}

pub fn build_ext<'a, F, const D: usize>(
    cb: ConstraintBuilderExt<Expr<'a, ExtensionTarget<D>>>,
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
        .map(|c| Constraint {
            constraint_type: c.constraint_type,
            constraint: evaluator.eval(c.constraint),
        })
        .collect::<Vec<_>>();

    evaluated.into_iter().for_each(|c| match c.constraint_type {
        ConstraintType::ConstraintFirstRow =>
            yield_constr.constraint_first_row(circuit_builder, c.constraint),
        ConstraintType::Constraint => yield_constr.constraint(circuit_builder, c.constraint),
        ConstraintType::ConstraintTransition =>
            yield_constr.constraint_transition(circuit_builder, c.constraint),
    })
}

pub fn build_packed<'a, F, FE, P, const D: usize, const D2: usize>(
    cb: ConstraintBuilderExt<Expr<'a, P>>,
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
        .map(|c| Constraint {
            constraint_type: c.constraint_type,
            constraint: evaluator.eval(c.constraint),
        })
        .collect::<Vec<_>>();

    for c in evaluated {
        match c.constraint_type {
            ConstraintType::ConstraintFirstRow => yield_constr.constraint_first_row(c.constraint),
            ConstraintType::Constraint => yield_constr.constraint(c.constraint),
            ConstraintType::ConstraintTransition =>
                yield_constr.constraint_transition(c.constraint),
        }
    }
}
