# Circuits Sub-crate

The **Circuits** sub-crate holds the constraints that enforce the correctness of the MozakVM trace.

These constraints are based on [Plonky2](https://github.com/mir-protocol/plonky2), in particular
its [Starky](https://github.com/mir-protocol/plonky2/tree/main/starky) variation with an addition of Cross Table
Lookups (CTL).

## Constraints

Constraints are defined over STARK tables, which are then connected between each other using Cross Table Lookups.

### Tables Structure

Each Table is defined in its own module and in

- `columns.rs` contains declaration of used columns.
- `stark.rs` contains definition of constraints to be applied over these columns.

To read more about Starks proving system, you can refer
to [this internal page](https://www.notion.so/0xmozak/eb420963310e407dafce95d267f5a55e?v=a6a42a89bb744309999d9a0ff16ce25f&p=0d7f4fe31e214c3aa9b719908ac56e57&pm=s).

Note that in order to make tables columns easier to interpret, we make use the `View` wrappers, that allow to access
each column of a table by name and not just by index, as in vanilla _Plonk2_. This makes development less error-prone
and code comprehension easier.

### Constraints Degree

We try to limit all constraints degrees to `3`, as the higher the degree, the longer is the proving time. In general,
one can reason of the proving time as _**O(num_rows * num_cols * num_constraints * constraints_degree)**_, thus we are
constantly balancing these values.

We could go lower than `3` to use `2` for example, as one can always express any constraint of degree `n` with a set of
constraints of degree `2` by introducing new intermediate columns. However, due to how Cross Table Lookups (CTL) are
implemented, they require at least degree `3` constraints to work.

### Padding Rows

Due to how STARKs work, we need to always pad tables to a size which is a power of two (due to the requirements of
FFT/IFFT).

```rust
// utils.rs
pub fn pad_trace_with_default<Row: Default + Clone>(trace: Vec<Row>) -> Vec<Row> {
    let len = trace.len().next_power_of_two();
    pad_trace_with_default_to_len(trace, len)
}
```

You will also see STARK tables, such as `rangecheck` and `bitshift` are fulfilling double purpose, they are used for
CTL, and at the same time define the range of values, such as values from `0` to `2^16` for `rangecheck` and values
from `0` to `32` for `bitshift`.

There, one need to specify which values are allowed to be looked up from other tables, and which are there just to form
the range and build constraints to form a table. For that we use filter columns, such as `is_executed`.

### Constraints Declaration

Constraints assert that certain multi-variate polynomial of degree less or equal to the maximum constraints degrees
evaluate to
zero for each row of the table. This polynomial can also depend on column values from the following or presiding rows of
the table.

#### Example

Let us see an example of a constraint. In particular, we will show a constraint that checks that a value of column `x`
is 0 or 1 when the `is_looked_up` row selector is non-zero.
The following will achieve this:

```ignore
    yield_constr.constraint((1 - lv.x) * (lv.x) * (lv.is_looked_up));
```

To explain why, when `is_looked_up` is `0`, the constraint is trivially satisfied, as `0 * (1 - lv.x) * (lv.x) = 0`.
When `is_looked_up` is `1`, the constraint is satisfied if `lv.x` is `0` or `1`, as `1 * (1 - lv.x) * (lv.x) = 0`.

As some constraints, such the one above, are very frequent, we have introduced aliases for then. One of them
is `is_binary`, which asserts that the value of a given column is indeed `0` or `1`.

You will also find that the more complex constraints are split into smaller ones, to make them easier to read. A good
example is the CPU STARK table, where instead of a single `stark.rs` file, we have multiple modules, for each OPCODE,
such as `add`, `sub`, `mul`, etc. All of these constraints are then applied to each row of the table, however, using
selectors, we can turn on or off the constraints for each row, depending on the opcode.

### Selectors

The above `CPU` example shows the idea behind selectors. We need them to turn on or off a constraint
based on some logic.

It works the following way: if you multiply any constraint by the selector, if the selector is `0`, then the constraint
no longer needs to hold, as a multiple of zero is always zero. If the selector is not `0`, then the rest of the
constraint needs to hold (evaluate to zero).

Note that to add more complex behaviour, it is sometimes necessary to apply logic to selectors, such as making some
selectors exclusive with each other. This is what we do in the CPU table for example, where to make sure each row has at
least one opcode active, we enforce that one of all opcode selectors is non-zero (this is done by checking that the sum
of all selectors is `1`).

You might also notice that not all opcodes constraints have selectors. This is because for some operations there is no
harm for their constraints to be enabled everywhere, as they do not collude with anything else.

### Helper Columns

Sometimes we need to add additional information to the table, to make constraints simpler. One example of that we have
covered above, where we need to convert a constraint of degree `n` to a combination of smaller constraints. Another case
where we might need that is for _hints_. As one might recall from Computer Theory, there are problems such that
verifying that their answer is correct is much simpler than computing the answer. A great example of such problems
is the [NP](https://en.wikipedia.org/wiki/NP_(complexity)) class, and in particular problems such
as [3-coloring](http://www.cs.toronto.edu/~lalla/373s16/notes/3col.pdf)
or [CDH](https://en.wikipedia.org/wiki/Computational_Diffie–Hellman_assumption)).

For such cases, it might be easier to add a new column with an answer (hint), and then just verify its correctness, then
computing it directly in some form of constraints. This generally adds a very interesting twist on the problem at hand,
as some of the operations can be handed off to a computer and then verified, which adds a new level of optimisation.

A more practice example of such optimisation is proving that a number is not `0`.

One way would be to range check that a
number is in some range that does not include `0`. However, it can be done much simpler.
We can prove that an inverse of a number exists. To do so, we need to first compute it. This can be done based on the
field size and some algorithms, yet proving that we did all the steps correctly would be very computationally intensive.

So the solution is to off-load the computation to a powerful machine, and then add a column that contains the inverse of
the number. Then we can just verify that the number multiplied by the inverse is `1`.

We must point out though that adding a helper column is not always unjustified. If the helper (hint) value is just a
linear or low-degree combination of other columns, it is cheaper to just keep it as a low degree combination, possibly
adding an alias for easier use.

### Cross Table Lookups (CTL)

**This section is WIP, and its content will change with future PRs.**

On a very high level, CTL work by taking two tables with the same amount of rows, selecting a subset of columns from
both tables and saying that the rows of the selected sub-set tables form a
permutation. It should be clear that for this to work, both subsets of columns should be of the same size.

#### Example

Let us take some table `CPU` with columns `{x, y, sum, prod, xor, custom_func, custom_func_selector}` and
table `ComplexFunc` with
columns `{x, y, custom_func, is_looked_up}`. Here, table `ComplexFunc` just lists all evaluations of `custom_func`,
which
would be too hard to constraint otherwise.

We can not make sure that the `CPU` table correctly calculated the value of `custom_func(x, y)` by adding constraints on
it, as the function can not be efficiently expressed with constraints, unlike `sum` or `prod`. Therefor we can just look
up the triple `x,y,custom_func` in the `ComplexFunc` table, and if it is present there, then the evaluation
in the CPU is correct.

However, as the order in which the CPU has `complex_func` can be different from the order it appears in
the `ComplexFunc` table, we need to not check row-by-row, but form some sort of permutation between rows. And as not all
evaluations of `complex_func` will be executed in the CPU, we will need a sub-set inclusion of `CPU` columns in
the `ComplexFunc` columns, for that we use the `is_looked_up` and `custom_func_selector` selector columns.

Finally, as CPU might invoke a single evaluation of `complex_func` multiple times, we need to add as many copies of that
to the `ComplexFunc` table as might get invoked. This again can be simplified by using sub-set inclusion arguments, but
is the current limitation.

To answer the question "Why should I trust the content of the `ComplexFunc` table?" - it can be committed
upfront and then checked by the user or some independent validator once to indeed contain the correct evaluations
of `custom_func`.

### Lookups

**This section is WIP, and its content will change with future PRs.**

Lookups are rather simpler than CTL and work as CTL happens on a single table. For that, we select two columns and say
that the values of the columns must be the same up to a row position permutation. A good example of a lookup is in the
RangeCheck table.

There we need to show that lo (hi) limb is a u16. For that, we first create a column with all u16
values, and then
create another permuted a column of lo (hi) values in question to match the position of values in teh u16 column.
Finally, we make sure that values in the permuted column are equal to the values in the original column with all u16
values in each row.

## Table Value Insertion

After we have defined the constraints, we need to fill in the tables with values that actually fulfill the constraints.
This is done in the `generation` module based on the trace of the program and the code of the program.

## Stark Cryptography

As the **Plonky2** API is quite limiting, especially when it comes to the CTL, we had to actually use all the primitives
directly and not their abstractions. This is why in the `stark` module you will find the STARK protocol implementation.
We suggest to pay attention to the `stark/mozak_stark.rs`, `stark/prover.rs` and `stark/verifier.rs` as this is where
the final lookup and regular are enforced over the tables.
Other files have much more cryptography in them, and it is advised to first become familiar with the STARK protocol.