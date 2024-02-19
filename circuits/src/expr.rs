use expr::{BinOp, Evaluator, Expr, ExprBuilder};
use plonky2::field::extension::Extendable;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::RecursiveConstraintConsumer;

use crate::memory::columns::Memory;

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
            BinOp::Div => self.builder.div_extension(left, right),
        }
    }

    fn one(&mut self) -> ExtensionTarget<D> { self.builder.one_extension() }
}

struct Constraint<E> {
    constraint_type: ConstraintType,
    constraint: E,
}

enum ConstraintType {
    ConstraintFirstRow,
    Constraint,
    ConstraintTransition,
}

pub struct ConstraintBuilderExt<'a, const D: usize> {
    constraints: Vec<Constraint<Expr<'a, ExtensionTarget<D>>>>,
}

impl<'a, const D: usize> ConstraintBuilderExt<'a, D> {
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }

    pub fn constraint_first_row(&mut self, constraint: Expr<'a, ExtensionTarget<D>>) {
        let c = Constraint {
            constraint_type: ConstraintType::ConstraintFirstRow,
            constraint,
        };
        self.constraints.push(c)
    }

    pub fn constraint(&mut self, constraint: Expr<'a, ExtensionTarget<D>>) {
        let c = Constraint {
            constraint_type: ConstraintType::Constraint,
            constraint,
        };
        self.constraints.push(c)
    }

    pub fn constraint_transition(&mut self, constraint: Expr<'a, ExtensionTarget<D>>) {
        let c = Constraint {
            constraint_type: ConstraintType::ConstraintTransition,
            constraint,
        };
        self.constraints.push(c)
    }

    pub fn build<F>(
        self,
        circuit_builder: &mut CircuitBuilder<F, D>,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) where
        F: RichField,
        F: Extendable<D>, {
        let mut evaluator = CircuitBuilderEvaluator {
            builder: circuit_builder,
        };

        let evaluated = self
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
}

pub fn inject_memory<'a, V>(expr_builder: &'a ExprBuilder, m: &Memory<V>) -> Memory<Expr<'a, V>>
where
    V: Clone, {
    let m = m.clone();
    Memory {
        is_writable: expr_builder.lit(m.is_writable),
        addr: expr_builder.lit(m.addr),
        clk: expr_builder.lit(m.clk),
        is_store: expr_builder.lit(m.is_store),
        is_load: expr_builder.lit(m.is_load),
        is_init: expr_builder.lit(m.is_init),
        value: expr_builder.lit(m.value),
        diff_clk: expr_builder.lit(m.diff_clk),
        diff_addr_inv: expr_builder.lit(m.diff_addr_inv),
    }
}
