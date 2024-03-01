# Writing Starky Constraints

Starky constraints are implemented by implementing the [Stark trait].

Most of the time, these three functions below are implemented.

```rust
/// Evaluate constraints at a vector of points.
///
/// The points are elements of a field `FE`, a degree `D2` extension of `F`. This lets us
/// evaluate constraints over a larger domain if desired. This can also be called with `FE = F`
/// and `D2 = 1`, in which case we are using the trivial extension, i.e. just evaluating
/// constraints over `F`.
fn eval_packed_generic<FE, P, const D2: usize>(
    &self,
    vars: &Self::EvaluationFrame<FE, P, D2>,
    yield_constr: &mut ConstraintConsumer<P>,
) where
    FE: FieldExtension<D2, BaseField = F>,
    P: PackedField<Scalar = FE>;

/// Evaluate constraints at a vector of points from the degree `D` extension field. This is like
/// `eval_ext`, except in the context of a recursive circuit.
/// Note: constraints must be added through`yield_constr.constraint(builder, constraint)` in the
/// same order as they are given in `eval_packed_generic`.
fn eval_ext_circuit(
    &self,
    builder: &mut CircuitBuilder<F, D>,
    vars: &Self::EvaluationFrameTarget,
    yield_constr: &mut RecursiveConstraintConsumer<F, D>,
);

/// The maximum constraint degree.
fn constraint_degree(&self) -> usize;
```

Recall that in STARK, we constrain how the rows are initialised and how rows transition from the current one to the next.

We will be using `vars` to get the current and next row of the evaluation frame, and use `yield_constr` to write the constraints. The following are helper functions of `yield_constr` that are used to write Starky constraints, check out the full API [here]

```rust
/// Add one constraint valid on all rows except the last.
///
/// Leaves degree unchanged.
#[track_caller]
pub fn constraint_transition(&mut self, constraint: P)

/// Add one constraint on all rows.
#[track_caller]
pub fn constraint(&mut self, constraint: P)

/// Add one constraint, but first multiply it by a filter such that it will only apply to the
/// first row of the trace.
///
/// Increases degree by 1.
#[track_caller]
pub fn constraint_first_row(&mut self, constraint: P)

/// Add one constraint, but first multiply it by a filter such that it will only apply to the
/// last row of the trace.
///
/// Increases degree by 1.
#[track_caller]
pub fn constraint_last_row(&mut self, constraint: P)
```

## Bitshift Table Example

We will see how the bitshift table in our code base is constrained.

The code below generates constraints for a RISC-V bitshift table. When a bitshift instruction is executed, the instruction and corresponding registers are constrained by referring to this table.

The table has an `amount` and a `multiplier` field. Specifically, \\( multiplier = 1 << amount \\). `local_values` stands for the values of the current row, and `next_values` stands for the values of the next row.
The constraint logic is written with comments in the code.

```rust
{{#include ../../circuits/src/bitshift/stark.rs:40:82}}
```

We use Starky for the constraint in the execution circuit and Plonky2 for the recursion circuit. The following function implements the same constraint but in the Plonky2 recursive circuit.

```rust
{{#include ../../circuits/src/bitshift/stark.rs:86:122}}
```

It is clear that the constraints here are same as whats written in `fn eval_packed_generic` but with plonkish arithemtization, used in recursive circuit.

Finally, we have

```rust
fn constraint_degree(&self) -> usize { 3 }
```

This is due to our use of Cross Table Lookups, which require a minimal constraint degree of 3.

If you are interested in more examples, there is also a Fibonacci STARK example in the [plonky2 codebase].

<!-- Add this once we have a discord/telegram/other platform: If you have questions about the constraints and how they are writtern, feel free to reach out to us at ... -->

[Stark trait]: https://github.com/0xPolygonZero/plonky2/blob/main/starky/src/stark.rs#L20-L225
[here]: https://github.com/0xPolygonZero/plonky2/blob/main/starky/src/constraint_consumer.rs
[plonky2 codebase]: https://github.com/0xPolygonZero/plonky2/blob/main/starky/src/fibonacci_stark.rs
