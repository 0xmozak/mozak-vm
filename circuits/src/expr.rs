use core::fmt::Debug;
use std::fmt::Display;
use std::marker::{Copy, PhantomData};
use std::panic::Location;

pub use expr::PureEvaluator;
use expr::{BinOp, Cached, Evaluator, Expr, ExprBuilder, StarkFrameTyped, UnaOp};
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::StarkFrame;
use starky::stark::Stark;

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
    pub constraint_type: ConstraintType,
    pub location: &'static Location<'static>,
    pub term: E,
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

#[derive(PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
pub enum ConstraintType {
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

#[must_use]
pub fn build_debug<F, FE, P, const D: usize, const D2: usize>(
    cb: ConstraintBuilder<Expr<'_, P>>,
) -> Vec<Constraint<P>>
where
    F: RichField,
    F: Extendable<D>,
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>, {
    let mut evaluator = Cached::from(packed_field_evaluator());
    cb.constraints
        .into_iter()
        .map(|c| c.map(|constraint| evaluator.eval(constraint)))
        .collect()
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

// Helper Types to Access members of GenerateConstraints
pub type PublicInputsOf<'a, S, T, const N: usize, const M: usize> =
    <S as GenerateConstraints<N, M>>::PublicInputs<T>;
pub type ViewOf<'a, S, T, const N: usize, const M: usize> =
    <S as GenerateConstraints<N, M>>::View<T>;

pub type Vars<'a, S, T, const N: usize, const M: usize> =
    StarkFrameTyped<ViewOf<'a, S, Expr<'a, T>, N, M>, PublicInputsOf<'a, S, Expr<'a, T>, N, M>>;

pub trait GenerateConstraints<const COLUMNS: usize, const PUBLIC_INPUTS: usize> {
    type View<E: Debug>: From<[E; COLUMNS]> + FromIterator<E>;
    type PublicInputs<E: Debug>: From<[E; PUBLIC_INPUTS]> + FromIterator<E>;

    // TODO: can we do a default Vars?

    fn generate_constraints<'a, T: Debug>(
        &self,
        vars: &Vars<'a, Self, T, COLUMNS, PUBLIC_INPUTS>,
    ) -> ConstraintBuilder<Expr<'a, T>>;
}

#[derive(Copy, Clone, Debug, Default)]
pub struct StarkFrom<F, G, const D: usize, const COLUMNS: usize, const PUBLIC_INPUTS: usize> {
    pub witness: G,
    pub _f: PhantomData<F>,
}

impl<G: Display, F, const D: usize, const COLUMNS: usize, const PUBLIC_INPUTS: usize> Display
    for StarkFrom<F, G, D, COLUMNS, PUBLIC_INPUTS>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.witness.fmt(f) }
}

impl<G, F, const D: usize, const COLUMNS: usize, const PUBLIC_INPUTS: usize> Stark<F, D>
    for StarkFrom<F, G, D, COLUMNS, PUBLIC_INPUTS>
where
    G: Sync + GenerateConstraints<COLUMNS, PUBLIC_INPUTS> + Copy,
    F: RichField + Extendable<D> + Debug,
{
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, { COLUMNS }, { PUBLIC_INPUTS }>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, { COLUMNS }, { PUBLIC_INPUTS }>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        constraint_consumer: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE> + Debug + Copy, {
        let expr_builder = ExprBuilder::default();
        let constraints = self
            .witness
            .generate_constraints::<_>(&expr_builder.to_typed_starkframe(vars));
        build_packed(constraints, constraint_consumer);
    }

    fn eval_ext_circuit(
        &self,
        circuit_builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        constraint_consumer: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let expr_builder = ExprBuilder::default();
        let constraints = self
            .witness
            .generate_constraints(&expr_builder.to_typed_starkframe(vars));
        build_ext(constraints, circuit_builder, constraint_consumer);
    }

    fn constraint_degree(&self) -> usize { 3 }
}
