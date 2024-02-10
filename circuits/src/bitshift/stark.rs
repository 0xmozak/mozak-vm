use std::marker::PhantomData;

use mozak_circuits_derive::StarkNameDisplay;
use plonky2::field::extension::{Extendable, FieldExtension};
use plonky2::field::packed::PackedField;
use plonky2::field::types::Field;
use plonky2::hash::hash_types::RichField;
use plonky2::iop::ext_target::ExtensionTarget;
use plonky2::plonk::circuit_builder::CircuitBuilder;
use starky::constraint_consumer::{ConstraintConsumer, RecursiveConstraintConsumer};
use starky::evaluation_frame::{StarkEvaluationFrame, StarkFrame};
use starky::stark::Stark;

use super::columns::{Bitshift, BitshiftView};
use crate::columns_view::{HasNamedColumns, NumberOfColumns};

/// Bitshift Trace Constraints
#[derive(Copy, Clone, Default, StarkNameDisplay)]
#[allow(clippy::module_name_repetitions)]
pub struct BitshiftStark<F, const D: usize> {
    pub _f: PhantomData<F>,
}

impl<F, const D: usize> HasNamedColumns for BitshiftStark<F, D> {
    type Columns = BitshiftView<F>;
}

const COLUMNS: usize = BitshiftView::<()>::NUMBER_OF_COLUMNS;
const PUBLIC_INPUTS: usize = 0;

impl<F: RichField + Extendable<D>, const D: usize> Stark<F, D> for BitshiftStark<F, D> {
    type EvaluationFrame<FE, P, const D2: usize> = StarkFrame<P, P::Scalar, COLUMNS, PUBLIC_INPUTS>

    where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>;
    type EvaluationFrameTarget =
        StarkFrame<ExtensionTarget<D>, ExtensionTarget<D>, COLUMNS, PUBLIC_INPUTS>;

    fn eval_packed_generic<FE, P, const D2: usize>(
        &self,
        vars: &Self::EvaluationFrame<FE, P, D2>,
        yield_constr: &mut ConstraintConsumer<P>,
    ) where
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        let lv: &BitshiftView<P> = vars.get_local_values().into();
        let nv: &BitshiftView<P> = vars.get_next_values().into();
        let lv: &Bitshift<P> = &lv.executed;
        let nv: &Bitshift<P> = &nv.executed;

        // Constraints on shift amount
        // They ensure:
        //  1. Shift amount increases with each row by 0 or 1.
        // (We allow increases of 0 in order to allow the table to add
        //  multiple same value rows. This is needed when we have multiple
        //  `SHL` or `SHR` operations with the same shift amount.)
        //  2. We have shift amounts starting from 0 to max possible value of 31.
        // (This is due to RISC-V max shift amount being 31.)

        let diff = nv.amount - lv.amount;
        // Check: initial amount value is set to 0
        yield_constr.constraint_first_row(lv.amount);
        // Check: amount value is increased by 1 or kept unchanged
        yield_constr.constraint_transition(diff * (diff - P::ONES));
        // Check: last amount value is set to 31
        yield_constr.constraint_last_row(lv.amount - P::Scalar::from_canonical_u8(31));

        // Constraints on multiplier
        // They ensure:
        //  1. Shift multiplier is multiplied by 2 only if amount increases.
        //  2. We have shift multiplier from 1 to max possible value of 2^31.

        // Check: initial multiplier value is set to 1 = 2^0
        yield_constr.constraint_first_row(lv.multiplier - P::ONES);
        // Check: multiplier value is doubled if amount is increased
        yield_constr.constraint_transition(nv.multiplier - (P::ONES + diff) * lv.multiplier);
        // Check: last multiplier value is set to 2^31
        // (Note that based on the previous constraint, this is already
        //  satisfied if the last amount value is 31. We leave it for readability.)
        yield_constr.constraint_last_row(lv.multiplier - P::Scalar::from_canonical_u32(1 << 31));
    }

    fn constraint_degree(&self) -> usize { 3 }

    fn eval_ext_circuit(
        &self,
        builder: &mut CircuitBuilder<F, D>,
        vars: &Self::EvaluationFrameTarget,
        yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    ) {
        let lv: &BitshiftView<ExtensionTarget<D>> = vars.get_local_values().into();
        let nv: &BitshiftView<ExtensionTarget<D>> = vars.get_next_values().into();
        let lv: &Bitshift<ExtensionTarget<D>> = &lv.executed;
        let nv: &Bitshift<ExtensionTarget<D>> = &nv.executed;

        yield_constr.constraint_first_row(builder, lv.amount);

        let diff = builder.sub_extension(nv.amount, lv.amount);
        let one_extension = builder.one_extension();
        let diff_sub_one = builder.sub_extension(diff, one_extension);
        let diff_mul_diff_sub_one = builder.mul_extension(diff, diff_sub_one);
        yield_constr.constraint_transition(builder, diff_mul_diff_sub_one);

        let thirty_one_extension = builder.constant_extension(F::Extension::from_canonical_u8(31));
        let amount_sub_thirty_one = builder.sub_extension(lv.amount, thirty_one_extension);
        yield_constr.constraint_last_row(builder, amount_sub_thirty_one);

        let multiplier_minus_one = builder.sub_extension(lv.multiplier, one_extension);
        yield_constr.constraint_first_row(builder, multiplier_minus_one);

        let one_plus_diff = builder.add_extension(one_extension, diff);
        let either_multiplier = builder.mul_extension(one_plus_diff, lv.multiplier);
        let multiplier_difference = builder.sub_extension(nv.multiplier, either_multiplier);
        yield_constr.constraint_transition(builder, multiplier_difference);

        let two_to_thirty_one_extension =
            builder.constant_extension(F::Extension::from_canonical_u32(1 << 31));
        let multiplier_sub_two_to_thirty_one =
            builder.sub_extension(lv.multiplier, two_to_thirty_one_extension);
        yield_constr.constraint_last_row(builder, multiplier_sub_two_to_thirty_one);
    }
}

// pub enum Polynomial<V, Binary> {
//     Base(V),
//     // UnaryNode(Unary, V),
//     BinaryNode(Binary, V, V),
// }

// // pub enum Unary<V> {
// //     Neg(V),
// // }

// pub enum Binary<V> {
//     Add(V, V),
//     Sub(V, V),
//     Mul(V, V),
// }

#[cfg(test)]
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use anyhow::Result;
    use mozak_runner::instruction::{Args, Instruction, Op};
    use mozak_runner::test_utils::{execute_code, u32_extra};
    use plonky2::plonk::config::{GenericConfig, Poseidon2GoldilocksConfig};
    use proptest::{prop_assert_eq, proptest};
    use starky::stark_testing::{test_stark_circuit_constraints, test_stark_low_degree};

    use super::BitshiftStark;
    use crate::stark::mozak_stark::MozakStark;
    use crate::test_utils::ProveAndVerify;

    const D: usize = 2;
    type C = Poseidon2GoldilocksConfig;
    type F = <C as GenericConfig<D>>::F;
    type S = BitshiftStark<F, D>;

    #[test]
    fn test_degree() -> Result<()> {
        let stark = S::default();
        test_stark_low_degree(stark)
    }

    #[test]
    fn prove_sll() -> Result<()> {
        let p: u32 = 10;
        let q: u32 = 10;
        let sll = Instruction {
            op: Op::SLL,
            args: Args {
                rd: 5,
                rs1: 7,
                rs2: 8,
                ..Args::default()
            },
        };
        // We use 3 similar instructions here to ensure duplicates and padding work
        // during trace generation.
        let (program, record) = execute_code([sll, sll, sll], &[], &[(7, p), (8, q)]);
        assert_eq!(record.executed[0].aux.dst_val, p << (q & 0x1F));
        MozakStark::prove_and_verify(&program, &record)
    }

    #[test]
    fn prove_srl() -> Result<()> {
        let p: u32 = 10;
        let q: u32 = 10;
        let srl = Instruction {
            op: Op::SRL,
            args: Args {
                rd: 5,
                rs1: 7,
                rs2: 8,
                ..Args::default()
            },
        };

        // We use 3 similar instructions here to ensure duplicates and padding work
        // during trace generation.
        let (program, record) = execute_code([srl, srl, srl], &[], &[(7, p), (8, q)]);
        assert_eq!(record.executed[0].aux.dst_val, p >> (q & 0x1F));
        MozakStark::prove_and_verify(&program, &record)
    }

    proptest! {
        #[test]
        fn prove_shift_amount_proptest(p in u32_extra(), q in u32_extra()) {
            let (program, record) = execute_code(
                [Instruction {
                    op: Op::SLL,
                    args: Args {
                        rd: 5,
                        rs1: 7,
                        rs2: 8,
                        ..Args::default()
                    },
                },
                Instruction {
                    op: Op::SRL,
                    args: Args {
                        rd: 6,
                        rs1: 7,
                        imm: q,
                        ..Args::default()
                    },
                }
                ],
                &[],
                &[(7, p), (8, q)],
            );
            prop_assert_eq!(record.executed[0].aux.dst_val, p << (q & 0x1F));
            prop_assert_eq!(record.executed[1].aux.dst_val, p >> (q & 0x1F));
            BitshiftStark::prove_and_verify(&program, &record).unwrap();
        }
    }

    #[test]
    fn test_circuit() -> anyhow::Result<()> {
        let stark = S::default();
        test_stark_circuit_constraints::<F, C, S, D>(stark)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Lit(u64),
    // Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

// We can define some helpful constructors:
impl Expr {
    pub fn lit(i: u64) -> Expr { Expr::Lit(i) }

    // pub fn neg(r: Expr) -> Expr {
    //     Expr::Neg(Box::new(r))
    // }

    pub fn add(r1: Expr, r2: Expr) -> Expr { Expr::Add(Box::new(r1), Box::new(r2)) }

    // fn eval_packed_generic<FE, P, const D2: usize>(
    //     &self,
    //     vars: &Self::EvaluationFrame<FE, P, D2>,
    //     yield_constr: &mut ConstraintConsumer<P>,
    // ) where
    //     FE: FieldExtension<D2, BaseField = F>,
    //     P: PackedField<Scalar = FE>, {

    // And now we can evaluate expressions in the language:
    pub fn eval_as_packed_generic<F, FE, P, const D2: usize>(&self) -> P
    where
        F: RichField,
        FE: FieldExtension<D2, BaseField = F>,
        P: PackedField<Scalar = FE>, {
        match self {
            Expr::Lit(i) => P::ONES * P::Scalar::from_noncanonical_u64(*i),
            // Expr::Neg(r) => -r.eval_as_packed_generic::<_, _, P, D2>(),
            Expr::Add(r1, r2) =>
                r1.eval_as_packed_generic::<_, _, P, D2>()
                    + r2.eval_as_packed_generic::<_, _, P, D2>(),
            Expr::Mul(r1, r2) =>
                r1.eval_as_packed_generic::<_, _, P, D2>()
                    * r2.eval_as_packed_generic::<_, _, P, D2>(),
        }
    }

    // fn eval_ext_circuit(
    //     &self,
    //     builder: &mut CircuitBuilder<F, D>,
    //     vars: &Self::EvaluationFrameTarget,
    //     yield_constr: &mut RecursiveConstraintConsumer<F, D>,
    // ) {
    // ExtensionTarget<const D: usize>

    pub fn eval_as_builder<F, P, const D: usize>(
        &self,
        builder: &mut CircuitBuilder<F, D>,
    ) -> ExtensionTarget<D>
    where
        F: RichField + Extendable<D>, {
        match self {
            Expr::Lit(i) => builder.constant_extension(F::Extension::from_canonical_u64(*i)),
            // Expr::Neg(r) => builder.neg_extension(r.eval_as_builder(builder)),
            Expr::Add(r1, r2) => {
                let r1 = r1.eval_as_builder::<F, P, D>(builder);
                let r2 = r2.eval_as_builder::<F, P, D>(builder);
                builder.add_extension(r1, r2)
            }
            Expr::Mul(r1, r2) => {
                let r1 = r1.eval_as_builder::<F, P, D>(builder);
                let r2 = r2.eval_as_builder::<F, P, D>(builder);
                builder.mul_extension(r1, r2)
            }
        }
    }

    // We can also pretty-print expressions:
    pub fn view(&self) -> String {
        match self {
            Expr::Lit(i) => i.to_string(),
            // Expr::Neg(r) => format!("(-{})", r.view()),
            Expr::Add(r1, r2) => format!("({} + {})", r1.view(), r2.view()),
            Expr::Mul(r1, r2) => format!("({} * {})", r1.view(), r2.view()),
        }
    }

    // and do a bunch of other things.
}

// pub mod final_style {

// // ----------------------------------------------------------------------------

// // In this tutorial we'll contrast two ways of defining an embedded DSL in
// Rust. // Embedded DSLs sound complicated, but quite a few problems can be
// re-cast as // writing an embedded DSL. I often find it useful to think of API
// design as // embedded DSL design - are you coming up with an API for
// dataframe manipulation, // plotting, or some other domain-specific task? Then
// you can think about the API // as an embeddded DSL too, and the
// implementation of the API becomes the interpreter // (or compiler) of the
// DSL.

// // What makes a DSL "embedded" is simply that it's embedded in a "general
// purporse" host language // like Rust. This is both convenient and limiting.
// It's convenient because you can use the // host language's features to
// implement the DSL, and limiting because most host languages have // some limitations
// on what is expressible, e.g. one often wants to overload literals.

// // Consider the "canonical" way of defining a simple expression language as
// an embedded DSL. // For reasons that will become clear later, it's just
// slightly more // expressive (with negation) than Hutton's razor.

// // We usually start out with defining the abstract syntax tree:

// use std::marker::PhantomData;

// #[derive(Debug, Clone)]
// enum Expr {
//     Lit(i32),
//     Neg(Box<Expr>),
//     Add(Box<Expr>, Box<Expr>),
// }

// // We can define some helpful constructors:
// impl Expr {
//     fn lit(i: i32) -> Expr {
//         Expr::Lit(i)
//     }

//     fn neg(r: Expr) -> Expr {
//         Expr::Neg(Box::new(r))
//     }

//     fn add(r1: Expr, r2: Expr) -> Expr {
//         Expr::Add(Box::new(r1), Box::new(r2))
//     }

//     // And now we can evaluate expressions in the language:
//     fn eval(&self) -> i32 {
//         match self {
//             Expr::Lit(i) => *i,
//             Expr::Neg(r) => -r.eval(),
//             Expr::Add(r1, r2) => r1.eval() + r2.eval(),
//         }
//     }

//     // We can also pretty-print expressions:
//     fn view(&self) -> String {
//         match self {
//             Expr::Lit(i) => i.to_string(),
//             Expr::Neg(r) => format!("(-{})", r.view()),
//             Expr::Add(r1, r2) => format!("({} + {})", r1.view(), r2.view()),
//         }
//     }

//     // and do a bunch of other things.
// }

// // Note - this relies heavily on pattern matching. Oleg calls this the
// "initial" style, // in contrast to the "final" style we'll define now.

// // As a first approximation, let's suppose we only want to evaluate our
// language. Then // we can skip the enum entirely and instead define our
// language directly as functions // in the host language:

// type Repr = i32;

// fn lit(i: i32) -> Repr {
//     i
// }

// fn neg(r: Repr) -> Repr {
//     -r
// }

// fn add(r1: Repr, r2: Repr) -> Repr {
//     r1 + r2
// }

// // That's not flexible enough - we want to re-interpret in different ways,
// // e.g. eval and view. We'd like to essentially overload these functions -
// // exactly what traits are for:
// trait ExprSym {
//     type Repr;

//     fn lit(i: i32) -> Self::Repr;
//     fn neg(r: Self::Repr) -> Self::Repr;
//     fn add(r1: Self::Repr, r2: Self::Repr) -> Self::Repr;
// }

// // The trait definition of the syntax is isomorphic to the enum definition.
// // Now we can implement the trait for different representations. First, eval:
// struct Eval;
// impl ExprSym for Eval {
//     type Repr = i32;

//     fn lit(i: i32) -> Self::Repr {
//         i
//     }

//     fn neg(r: Self::Repr) -> Self::Repr {
//         -r
//     }

//     fn add(r1: Self::Repr, r2: Self::Repr) -> Self::Repr {
//         r1 + r2
//     }
// }
// // trick to make rust infer the type, without explicit type arguments.
// // Rust can't infer the type of the trait from the repr, so this provides
// // a link back from the implemented repr type (e.g. i32) to the interpreter
// // type (e.g. Eval).
// trait HasExprSym {
//     type ES: ExprSym;
// }

// impl HasExprSym for i32 {
//     type ES = Eval;
// }

// fn exprsym_eval(e: i32) -> i32 {
//     e
// }

// // And here is view:
// struct View;
// impl ExprSym for View {
//     type Repr = String;

//     fn lit(i: i32) -> Self::Repr {
//         i.to_string()
//     }

//     fn neg(r: Self::Repr) -> Self::Repr {
//         format!("(-{r})")
//     }

//     fn add(r1: Self::Repr, r2: Self::Repr) -> Self::Repr {
//         format!("({r1} + {r2})")
//     }
// }

// impl HasExprSym for String {
//     type ES = View;
// }

// fn exprsym_view(e: String) -> String {
//     e
// }

// // You probably have questions now. In particular, why would you use this
// weird final style over the familiar intial style? // And are the two really
// equivalent? For example, we seem to be losing pattern matching in the final
// style.

// // We'll tackle some of these questions in turn.

// // First, why would you use this final style over the initial style? One
// reason could be extensibility. We've already seen that // both the initial
// and final style are easily extensible with new interpreters. In the initial
// style, we just write a new function // and pattern match. In the final style,
// we add a new trait implementation.

// // The final style is additionally easily extensible with new syntax. In the
// initial style, we'd have to add a new enum variant:

// enum ExprUgh {
//     Lit(i32),
//     Neg(Box<Expr>),
//     Add(Box<Expr>, Box<Expr>),
//     Mul(Box<Expr>, Box<Expr>),
// }

// // and rewrite all interpreters (eval, view, ...). Ugh.

// // In the final style, we just add extend the trait:

// trait MulExprSym: ExprSym {
//     fn mul(r1: Self::Repr, r2: Self::Repr) -> Self::Repr;
// }

// // and add a new implementation:
// impl MulExprSym for Eval {
//     fn mul(r1: Self::Repr, r2: Self::Repr) -> Self::Repr {
//         r1 * r2
//     }
// }

// // Thus the final style effectively solves the expression problem.

// // A second advantage may be that we got rid of the Boxes in the enum - so
// potentially final style is faster? // TODO: benchmark

// // Now let's turn our attention to the what seems like final style's major
// limitation: the lack of pattern matching.

// // For an example where pattern matching is convenient, let's consider the
// case where we want to push down negation // to literals, getting rid of
// double negation. You can think of this as an example of an optimization pass.

// impl Expr {
//     fn push_neg(self) -> Expr {
//         match &self {
//             Expr::Lit(_) => self,
//             Expr::Neg(content) => match content.as_ref() {
//                 Expr::Lit(_) => self,
//                 Expr::Neg(c) => c.clone().push_neg(),
//                 Expr::Add(r1, r2) => Expr::add(
//                     Expr::Neg(r1.clone()).push_neg(),
//                     Expr::Neg(r2.clone()).push_neg(),
//                 ),
//             },
//             Expr::Add(r1, r2) => Expr::add(r1.clone().push_neg(),
// r2.clone().push_neg()),         }
//     }
// }

// // The result is a new expression which again we can interpret in many ways,
// e.g. eval and view.

// // Now let's see how we can do the same thing in the final style. In the
// inital style, it's clear that // the transformation depends on context - in
// particular we need to push down negation if an expression // occurs as part
// of another negation.

// // In the final stye, all we can really do is write a new interpreter - as a
// new implementation of the ExprSym trait. // The trait is parametrized by the
// associated type Repr, and we can achieve the same effect by making the
// context // explicit:
// enum Ctx {
//     Pos,
//     Neg,
// }

// struct CtxFun<TRepr>(Box<dyn Fn(&Ctx) -> TRepr>);

// impl<TRepr> CtxFun<TRepr> {
//     fn new(f: impl Fn(&Ctx) -> TRepr + 'static) -> Self {
//         CtxFun(Box::new(f))
//     }
// }

// // PhantomData here to get around "unconstrained type parameter T" in trait
// impl. struct PushNeg<T>(PhantomData<T>);
// impl<T: ExprSym + 'static> ExprSym for PushNeg<T> {
//     type Repr = CtxFun<T::Repr>;

//     fn lit(i: i32) -> Self::Repr {
//         CtxFun::new(move |ctx| match ctx {
//             Ctx::Pos => T::lit(i),
//             Ctx::Neg => T::neg(T::lit(i)),
//         })
//     }

//     fn neg(r: Self::Repr) -> Self::Repr {
//         CtxFun::new(move |ctx| match ctx {
//             Ctx::Pos => r.0(&Ctx::Neg),
//             Ctx::Neg => r.0(&Ctx::Pos),
//         })
//     }

//     fn add(r1: Self::Repr, r2: Self::Repr) -> Self::Repr {
//         CtxFun::new(move |ctx| T::add(r1.0(ctx), r2.0(ctx)))
//     }
// }

// fn exprsym_push_neg0<S: ExprSym>(e: &CtxFun<S::Repr>) -> S::Repr {
//     e.0(&Ctx::Pos)
// }

// // What's this business with the phantom data?
// // (Should introduce the PushNeg without PhantomData first)
// // It is only needed to implement HasExprSym.
// struct CtxFunPh<TRepr, T>(Box<dyn Fn(&Ctx) -> TRepr>, PhantomData<T>);

// impl<TRepr, T> CtxFunPh<TRepr, T> {
//     fn new(f: impl Fn(&Ctx) -> TRepr + 'static) -> Self {
//         CtxFunPh(Box::new(f), PhantomData)
//     }
// }

// // PhantomData here to get around "unconstrained type parameter T" in trait
// impl. struct PushNegPh<T>(PhantomData<T>);
// impl<T: ExprSym + 'static> ExprSym for PushNegPh<T> {
//     type Repr = CtxFunPh<T::Repr, T>;

//     fn lit(i: i32) -> Self::Repr {
//         CtxFunPh::new(move |ctx| match ctx {
//             Ctx::Pos => T::lit(i),
//             Ctx::Neg => T::neg(T::lit(i)),
//         })
//     }

//     fn neg(r: Self::Repr) -> Self::Repr {
//         CtxFunPh::new(move |ctx| match ctx {
//             Ctx::Pos => r.0(&Ctx::Neg),
//             Ctx::Neg => r.0(&Ctx::Pos),
//         })
//     }

//     fn add(r1: Self::Repr, r2: Self::Repr) -> Self::Repr {
//         CtxFunPh::new(move |ctx| T::add(r1.0(ctx), r2.0(ctx)))
//     }
// }

// // Here I'd love to write just CtxFun<T::Repr>, but then the compiler
// complains // T is not constrained. So we pass on the T into CtxFun as
// phantomdata. impl<T: ExprSym + 'static> HasExprSym for CtxFunPh<T::Repr, T> {
//     type ES = PushNegPh<T>;
// }

// fn exprsym_push_neg<S: ExprSym<Repr = T>, T: HasExprSym<ES = S>>(e:
// &CtxFunPh<T, S>) -> T {     e.0(&Ctx::Pos)
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     fn ti1() -> Expr {
//         Expr::add(
//             Expr::lit(8),
//             Expr::neg(Expr::add(Expr::lit(1), Expr::lit(2))),
//         )
//     }

//     fn tf1_pre<E: ExprSym>() -> E::Repr {
//         E::add(E::lit(8), E::neg(E::add(E::lit(1), E::lit(2))))
//     }

//     fn tf1<E: ExprSym<Repr = T>, T: HasExprSym<ES = E>>() -> T {
//         E::add(E::lit(8), E::neg(E::add(E::lit(1), E::lit(2))))
//     }

//     #[test]
//     fn eval_equal() {
//         // exprsym_eval(tf1_pre());
//         let initial_style = Expr::eval(&ti1());
//         let final_style = exprsym_eval(tf1());

//         assert_eq!(initial_style, final_style);
//         dbg!(final_style);
//     }
//     #[test]
//     fn view_equal() {
//         let initial_style = Expr::view(&ti1());
//         let final_style = exprsym_view(tf1());

//         assert_eq!(initial_style, final_style);
//         dbg!(final_style);
//     }

//     fn tfm1<E: MulExprSym<Repr = T>, T: HasExprSym<ES = E>>() -> T {
//         E::add(E::lit(7), E::neg(E::mul(E::lit(1), E::lit(2))))
//     }

//     #[test]
//     fn mul_extensibility() {
//         let final_style = exprsym_eval(tfm1());
//         assert_eq!(5, final_style);

//         // Type safety without pattern match exhaustiveness checking:
//         // error[E0277]: the trait bound `View: MulExprSym` is not satisfied
//         // let final_style = exprsym_view(tfm1());
//         // because we have indeed not implement MulExprSym for View.
//     }

//     #[test]
//     fn push_neg_equal() {
//         let initial_style = ti1().push_neg();
//         let final_style = exprsym_push_neg(&tf1());

//         assert_eq!(Expr::view(&initial_style), exprsym_view(final_style));
//         dbg!(Expr::view(&initial_style));

//         let r = tf1_pre::<PushNeg<View>>();
//         dbg!(r.0(&Ctx::Pos));
//     }
// }
// }
