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

Due to how STARKs work, we need to always pad tables to a size which is a multiple of two (due to the requirements of
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

### Lookups and Permutation Arguments

Lookup is technique that allows anyone to check if a value of one column is present in another column. This can be
done between columns of the same table or between columns of different tables (Cross Table Lookups).
Furthermore, when you do such checks, sometime you do not care about the multiplicity of the value in the second column,
and sometimes you do. Sometimes you are okay that some values from the second column are not present in the first
column, and sometimes you need the columns to have identical values. Bellow, we will cover each of the techniques and
briefly explain
who it works.

Lastly, we have said in the beginning that lookups allow us to check if value from one column is present in the other.
However, most
of the time we actually check that an ordered tuple of values from one column combination is present in the other column
combination. This is a lot more versatile, and we use it excessively.

#### Permutation (Multi-set) Arguments

The simplest variant of lookups are permutation arguments. It checks that two columns (or two set of columns) have the
same values, up to a permutation. Meaning we could reorder values in the column (or rows of a set of columns) and
get the other column (or a set of columns). Each table can support multiple separate permutation arguments, and columns
can participate in multiple permutation arguments.

Finally, permutation argument is computationally cheapest from the ones listed bellow.

You can refer to the documentation in the `stark/permutation.rs` for more details on how it is implemented.

#### Subset Lookups

This is what we refer to as `lookup` in our code. It checks if we create a set from values (tuple of values) of one *
*looking** column (set of columns), then it will be a subset of a set of values (tuple of values) of another **looked**
column (set of columns).
The opposite is not required to be true, as the looked column (set of columns)
might have more unique values than the looking set of columns. Nevertheless, both sets of columns must have the same
number of
rows.

The subset lookups under the hood use two permutation lookups, adds two more columns, and one degree constraints between
two newly added columns.

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
    - Either value in _Permuted Column 1_ equals the value in _Permuted Column 2_
    - Or value in _Permuted Column 1_ is the same as value in _Permuted Column 1_ one row above.

You can refer to the documentation in the `lookup.rs` for more details on how it is implemented.

_Bellow is the illustration of the subset lookup between looking Column 1 and looked Column 2, with the 2 introduced
helper columns._

| Column 1 | Permuted Column 1 | Permuted Column 2 | Column 2 |
|----------|-------------------|-------------------|----------|
| 1        | 1                 | 1                 | 1        |
| 2        | 1                 | 4                 | 2        |
| 3        | 1                 | 5                 | 3        |
| 1        | 2                 | 2                 | 4        |
| 1        | 3                 | 3                 | 5        |
| 3        | 3                 | 6                 | 6        |

#### Cross Table Permutation (Multi-Multi-Set) Arguments

The subset lookups and permutation arguments described above only work on columns of the same table. However,
sometimes we may want to partition our table into multiple tables, and then check that the values of one table
are present in the values of another table columns. This is what we call Cross Table.

In Cross Table Permutation Arguments, which we actually refer to as **Cross Table Lookups** in our code base, we define
a single _looked table_ and multiple _looking tables_. The looked and looking tables can also be formed synthetically,
by just grouping a sub-set of columns from the already defined tables. With the tables defined, we want that
a [multi-set](https://en.wikipedia.org/wiki/Multiset) union of all the rows from the looking tables to be a permutation
of the rows of the looked table. We also allow each looking table to define a filter column that will be used to filter
out rows that we do not want to participate in the permutation.

To break down the above, we can have a looked table with columns `{x, y, z, allow_to_lookup}`and a several looking
tables which include columns `{x, y, z, look_up}`. By applying a cross table lookup, we make sure that the multi-set
of `(x, y, z)` values from the looked table, where `allow_to_lookup` is `1`, is a permutation of the multi-set
of `(x, y, z)` values from the looking table, where `is_looked_up` is `1`. This construction also implies if a single
looking table or several looking table look up the same row, then the looked table must have this row multiple times, as
each row can only be looked up once by any looking table.

In particular, we use the Permutation Cross Table Arguments to connect the STARK tables. It allows us to
off-load the constraints and extra columns for some logic into a separate tables, which possibly reduces the amount of
rows in the original tables.

The technique used to enable the Cross Table Permutation Arguments is very similar to the technique behind standard
[Permutation Arguments](https://hackmd.io/@arielg/ByFgSDA7D) and just requires us to use commitments to multiple STARK
tables, instead of a single one.

You can refer to the documentation in the `cross_table_lookup.rs` for more details on how it is implemented.

#### Cross Table Subset Lookups

As with Permutation Arguments being a building block for Subset Lookups, Cross Table Permutation Arguments are used to
construct Cross Table Subset Lookups.

Cross Table Subset Lookups allow us to check that the row values of looking table are present in
the set of row of the looked table. The reverse is not required to be true, and the looked table might have more unique
values than the looking table. Key difference between Cross Table Subset Lookups and Subset Lookups is that looking and
looked tables do not even need to have the same number of rows.

To make Cross Table Subset Lookups work, we introduce a new table, that will be used to combine columns from the looking
and looked tables, and then pad them to equal length. We first populate the table with rows of the
looking table, with a new additional column of `1`s, which will be used to indicate if a row is from the looking table
or from the looked table. We then populate the new table with columns from the looked table, which have not been
included yet in the new table, until we reach the size of the second table. We also indicate if a row was present in the
looked table with a binary column.

We then require that Multi-Multi-Set Cross Table Lookups holds between the new table and the looking table,
where `is_from_looking_table` indicator is turned on, and between the new table and the second table,
when `is_from_looked_table` is turned on. There is a very helpful
diagram [here](https://www.notion.so/0xmozak/Cross-Table-Lookup-bbe98d9471114c36a278f0c491f203e5?pvs=4#80f9047bc40f48f29c8ba852bf94c570)
explaining how it works.

Finally, like with Subset Lookups, we require that in the new table for each row:

- Either the row is not from the looking table.
- Or the row value is the same as next row value (excluding indicator columns)
- Or the is from the looked table is true.

This summarises the types of lookups and permutation arguments used in the codebase.

## Table Value Insertion

After we have defined the constraints, we need to fill in the tables with values that actually fulfill the constraints.
This is done in the `generation` module based on the trace of the program and the code of the program.

## Stark Cryptography

As the **Plonky2** API is quite limiting, especially when it comes to the CTL, we had to actually use all the primitives
directly and not their abstractions. This is why in the `stark` module you will find the STARK protocol implementation.
We suggest to pay attention to the `stark/mozak_stark.rs`, `stark/prover.rs` and `stark/verifier.rs` as this is where
the final lookup and regular are enforced over the tables.
Other files have much more cryptography in them, and it is advised to first become familiar with the STARK protocol.