# Circuits Sub-crate

The **Circuits** sub-crate holds the constraints that enforce the correctness of the MozakVM trace.

These constraints are based on [Plonky2](https://github.com/mir-protocol/plonky2), in particular
its [Starky](https://github.com/mir-protocol/plonky2/tree/main/starky) variation with an addition of Cross Table
Lookups (CTL).

## Constraints

Constraints are defined over STARK tables, which are then connected between each other using Cross Table Subset Lookups
and Cross Table Permutation Arguments.

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
constraints of degree `2` by introducing new intermediate columns. However, due to how Cross Table Permutation Argument
is implemented, it requires at least degree `3` constraints to work.

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

## Arguments

Arguments are used to reason about relationships between two sequences of values. This can be in the form of a
Permutation Argument or a Subset Argument. In our case, each sequence of values is represented by STARK table
column or a set of columns, and sequences can be from the same or from different tables.

Below, we will cover each of the arguments we use and briefly explain who it works.

### Permutation (Multi-set) Arguments

The simplest argument we will consider is the Permutation Arguments. It checks that two columns (or two set of columns)
have the same values, up to a permutation. Meaning we could reorder values in the column (or rows of a set of columns)
and get the other column (or a set of columns). Each table can support multiple separate Permutation Arguments, and
columns can participate in multiple Permutation Arguments.

The Permutation Argument is so efficient compared to the Subset Argument, that sometimes we use it despite the Subset
Arguments being a natural choice.

You can refer to the documentation in the `stark/permutation.rs` for more details on how it is implemented.

### Subset Arguments (Lookups)

Subset Argument (commonly referred to as Lookup) checks if a set of values (tuple of values) of a
*looking* column (group of columns), is subset of a set of values (tuple of values) of a *looked*
column (group of columns). The converse is not required to be true, as the looked column (group of columns)
might have more unique values than the looking column (group of columns). Nevertheless, both looking and looked columns
must have the same number of rows.

The Subset Argument under the hood use two Permutation Arguments, adds twice the number of new columns as number of
looked columns, and creates degree one constraint between the newly added groups of columns.

To give some intuition on how it works, lets assume we want to do lookups between a _Looking Column_ and the _Looked
Column_. For sets of columns it works identically. First, we add two more columns - _Permuted Looking Column_ and
_Permuted Looked Column_. We populate the _Permuted Looking Column_ with rows of *Looking Column*, grouped by the
values. We then populate the rows of the _Permuted Looked Column_ with values from the _Looked Column_ in the following
way:

- If the same row is present in the _Looking Column_, we place it at the same row index as this rows first occurrence in
  the _Permuted Looking Column_.
- Otherwise, we leave it until the end to fill in all the blank index positions.

Now, to make sure we have the Subset Lookup working, it is enough for us to check:

1. The permutation between _Column 1_ and _Permuted Column 1_
2. The permutation between _Column 2_ and _Permuted Column 2_
3. For each row:
    - Either value in _Permuted Column 1_ equals the value in _Permuted Column 2_.
    - Or value in _Permuted Column 1_ is the same as value in _Permuted Column 1_ one row above.

_Below is the illustration of the subset lookup between looking Column 1 and looked Column 2, with the 2 introduced
helper columns._

| Column 1 | Permuted Column 1 | Permuted Column 2 | Column 2 |
|----------|-------------------|-------------------|----------|
| 1        | 1                 | 1                 | 1        |
| 2        | 1                 | 4                 | 2        |
| 3        | 1                 | 5                 | 3        |
| 1        | 2                 | 2                 | 4        |
| 1        | 3                 | 3                 | 5        |
| 3        | 3                 | 6                 | 6        |

You can refer to the documentation in the `lookup.rs` for more details on how it is implemented.

Also, one can refer to several sources we have used as an inspiration for our codebase:

- [ZCash Halo2 lookup docs](https://zcash.github.io/halo2/design/proving-system/lookup.html)
- [ZK Meetup Seoul ECC X ZKS Deep dive on Halo2](https://www.youtube.com/watch?v=YlTt12s7vGE&t=5237s)

### Cross Table Permutation (Multi-Multi-Set) Arguments

The Subset Arguments and Permutation Arguments described above only work on columns from the same table. However,
sometimes we may want to make use of multiple tables to partition our computations into manageable chunks. Nevertheless,
we have to still link tables to for example make sure they refer to the consistent data, hence we need arguments that
work Cross Table.

In Cross Table Permutation Argument, which we actually refer to as **Cross Table Lookup (CTL)** in our code base, we
define a single _looked_ table and multiple _looking_ tables. The looked and looking tables can also be formed
synthetically, by just grouping a sub-set of columns from the already defined tables. With the tables defined, the Cross
Table Permutation Argument asserts that a [multi-set](https://en.wikipedia.org/wiki/Multiset) union of all rows from the
_looking_ tables is a permutation of rows of the _looked_ table. We also have each _looking_ table define a filter
column that is used to filter out rows that we do not want to participate in the permutation.

To break down the above, let us consider an example. We have a _looked_ table with columns `{x, y, z, allow_to_lookup}`
and a several _looking_ tables which contain columns `{x, y, z, look_up}`. Consider also that
both `allow_to_lookup` and `look_up` columns are boolean. By applying a Cross Table Permutation Argument, we make sure
that the multi-set of `(x, y, z)` values from the _looked_ table, where `allow_to_lookup`is `true`, is a permutation of
a multi-set of `(x, y, z)` values from the _looking_ tables, where `look_up`is `true`. This construction implies
that if a single _looking_ table or several _looking_ tables look up the same row multiple times, then the _looked_
table must have this row multiple times, as each row can only be looked up once by any _looking_ table.

The [technique](https://www.notion.so/0xmozak/Cross-Table-Lookup-bbe98d9471114c36a278f0c491f203e5?pvs=4#80f9047bc40f48f29c8ba852bf94c570)
used to enable the Cross Table Permutation Arguments is very similar to the technique behind standard
[Permutation Arguments](https://hackmd.io/@arielg/ByFgSDA7D) and just requires us to use commitments to multiple STARK
tables, instead of a single one.

You can refer to the documentation in the `cross_table_lookup.rs` for more details on how it is implemented.

### Cross Table Subset Arguments (Cross Table Subset Lookups)

Similar to Subset Arguments, Cross Table Subset Arguments are used to check that row values of _looking_ table are
present in the set of row of the _looked_ table. The converse is not required to be true, and the _looked_ table might
have more unique values than the _looking_ table. The only big difference between Cross Table Subset Arguments and
standard Subset Arguments is that _looking_ and _looked_ tables in the Cross Table Subset Arguments do not need to have
the same number of rows.

We should also point out that we often refer to Cross Table Subset Arguments as Cross Table Subset Lookups.

To make the Cross Table Subset Arguments work, similar to Subset Arguments, we introduce a new table, that will be used
to combine rows from the looking and looked tables. We first populate the table with rows of the _looking_ table, with a
new additional column used to indicate if a row is from the _looking_ table or from the _looked_ table. We then add to
the new table rows from the _looked_ table, until we reach the size of the second table. We also add another column to
indicate if a row is present in the _looked_ table.

We then require that a Cross Table Permutation Argument holds between the new table and the looking table,
for rows where `is_from_looking_table` indicator is turned on, and between the new table and the second table,
for rows where `is_from_looked_table` indicator is turned on. There is a very helpful
diagram [here](https://www.notion.so/0xmozak/Cross-Table-Lookup-bbe98d9471114c36a278f0c491f203e5?pvs=4#80f9047bc40f48f29c8ba852bf94c570)
explaining how it works.

Finally, like with Subset Lookups, we require that for each row in the new table:

- Either the row is not from the looking table.
- Or the row value is the same as next row value (excluding indicator columns)
- Or the is from the looked table is true.

This summarises the types of arguments used in the codebase.

## Table Value Insertion

After we have defined the constraints, we need to fill in the tables with values that actually fulfill the constraints.
This is done in the `generation` module based on the trace of the program and the code of the program.

## Stark Cryptography

As the **Plonky2** API is quite limiting, especially when it comes to the CTL, we had to actually use all the primitives
directly and not their abstractions. This is why in the `stark` module you will find the STARK protocol implementation.
We suggest to pay attention to the `stark/mozak_stark.rs`, `stark/prover.rs` and `stark/verifier.rs` as this is where
the final lookup and regular are enforced over the tables.
Other files have much more cryptography in them, and it is advised to first become familiar with the STARK protocol.