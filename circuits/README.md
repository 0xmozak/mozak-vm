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

### Lookups

Lookups is an idea that you can somehow check if a value from one column is present in the another column. This can be
done between columns of the same table or between columns of different tables (Cross Table Lookups).
Furthermore, when you do such checks, sometime you do not care about the multiplicity of the value in the second column,
and sometimes you do. Sometimes you are okay that some values from the second column are not present in the first
column, and sometimes you need the columns to have identical values. Bellow, we will cover all the cases and explain
which tooling allows us to do it.

Finally, we have said in the beginning that we want to check if one column values is present in the other. However, most
of the time you actually want to check that values of one column combination are present in the other column
combination. This gives a lot more flexibility to the protocol.

#### Permutation (Multi-set) Lookups

The simplest lookup is a permutation lookup. It is a lookup between two sets of columns of the same table, where the
values of the first set of columns should be equal to the values of the second set of columns, but in a different order.
The order is given by some permutation.

You can refer to the documentation in the `stark/permutation.rs` for more details on how it is implemented.

##### Example

Inside the `RangeCheck` table, we have a permutation lookup between the `limb_lo`, `limb_hi` columns and
the `limb_lo_permuted`, `limb_hi_permuted` columns. In that particular case, `limb_lo` and `limb_hi` permutations are
separate, though we could have linked them together, making sure that (lo, hi) row pairs are permuted together
into `(lo_permuted, hi_permuted)` pairs.

#### Subset Lookups

This is what we call `lookup` in our code. It checks that the values of rows of the first set of columns are present in
the values of rows of the second set of columns. The opposite is not required to be true, as the second set of columns
might have more unique values than the first set of columns. Though both sets of columns must have the same number of
rows.

Subset lookups work by first permuting the row values of the first set of columns, so that all duplicate values are
adjacent to each other. We check the permutation correctness using Permutation Lookups.

| Column 1 | Permuted Column 1 |
|----------|-------------------|
| 1        | 1                 |
| 2        | 1                 |
| 3        | 1                 |
| 1        | 2                 |
| 1        | 3                 |
| 3        | 3                 |  

Then we permute the row values of the second set of columns, so if a row is present in the first set of columns, we
place it at the same row index as this rows first occurrence of the row in the first set of **permuted** columns.
We check the permutation correctness using Permutation Lookups.

| Column 1 | Sorter Column 1 | Permuted Column 2 | Column 2 |
|----------|-----------------|-------------------|----------|
| 1        | 1               | 1                 | 1        |
| 2        | 1               | 4                 | 2        |
| 3        | 1               | 5                 | 3        |
| 1        | 2               | 2                 | 4        |
| 1        | 3               | 3                 | 5        |
| 3        | 3               | 6                 | 6        |

Finally, we compare the values of the first **permuted** set of columns and second **permuted** set of columns. Each row
of
the first permuted set can either be equal to the preceding row, or to the corresponding row of second permuted set.
This
gives us a guarantee that each unique row present in the first set of columns is present in the second set of columns.

##### Example

We use the subset lookup in the `RangeCheck` table to make sure that the `lo` and `hi` limbs are indeed u16 values.
For that we create the above-mentioned `limb_lo_permuted` and `limb_hi_permuted` columns, as well as
the `fixed_range_check_u16_permuted_lo` and `fixed_range_check_u16_permuted_hi` columns, which contain all possible u16
values.

Then we create a subset lookup between the `limb_lo_permuted` and `fixed_range_check_u16_permuted_lo` columns, which
achieves the goal of making sure that all `lo` limbs are u16 values. We do the same for the `hi` limbs.

#### Multi-Multi-Set Cross Table Lookups

With the lookup technique described above, we can only check that the values between columns of the same table. However,
sometimes we may want to partition our table into multiple tables, and then check that the values of one table columns
are present in the values of another table columns. This is what we call Cross Table Lookups (CTL).
The example where this comes in useful is again the `RangeCheck`.

If we did not split the `RangeCheck` table away from the main table, then the length of the table would be `2^16`, even
for a very small program and made the proving time much longer then necessary. By splitting it, we can the main table be
of arbitrary size, and the `RangeCheck` table be of size `2^16`, which is much more manageable due to a much smaller
amount of columns.

Cross Table Lookups work by defining a **looked table**, in which we will look up values from multiple
**looking tables**. These tables are formed by taking sub-sets of columns from the already defined tables.
In particular, we want that a multi-set-union of all the rows from the looking tables to be permutation of the rows of
the looked table. We also allow each looking table to define a filter column that will be used to filter out rows that
we do not want to look up in the looked table.

So, to break down the above, we can have a table with columns `{x, y, z}` (looked table) and a several table with
columns `{x, y, z, is_looked_up}` (looking table). By applying a cross table lookup, we make sure that the multi-set of
`(x, y, z)` values from the looked table is a permutation of the multi-set of `(x, y, z)` values from the looking table,
where `is_looked_up` is `1`. It implies that if several looking table look up the same row, then the looked table must
have the same row multiple times, as each row can only be looked up once by any looking table.

##### Example

We use the Multi-set CTL across all the STARK tables. In particular, we use it to connect the Xor table with the CPU, to
off-load the constraints and extra columns of the `xor` operation to the Xor table. By using the CTL we can avoid adding
the extra columns to the CPU table, and instead just add them to the Xor table, where the amount of rows is much
smaller.

The CTL, as explained, connects the set of columns. First column set is in the CPU, and has `xor_input_a`, `xor_input_b`
and `xor_output` values. Second column set is `xor_input_a`, `xor_input_b` and `xor_output`, but now in the Xor table.
Additionally, to that, both sets haas a filter column. The CPU has a `is_xor_used` filter, which tells if the Xor
operation has been used in a row, and if therefor we need to check the execution status. At the same time, Xor table
has a `is_executed` column, which tells if the row is a padding row, that pads the table size to the power of two, or if
it is an actual Xor computation row.

#### Subset Cross Table Lookups

As with Permutation Lookups being a building block for Subset Lookups, Multi-Multi-Set Cross Table Lookups allow us to
build Subset Cross Table Lookups.

Subset Cross Table Lookups allow us to check that the values of one table columns are present in the values of another
table columns. The opposite is not required to be true, as the second table might have more unique values than the first
table. Though both tables do not even need to have the same number of rows.

The idea is to introduce a new table. Part of the table will be populated with selected rows of the first table, with a
binary column that identifies if the row is from the first table or not. The rest will be populated with rows from the
second table, until the size of the new table is the same as the size of the second table. Note that we require that the
first table is smaller than the second. The rest is similar to how Subset Lookups work.

We require that Multi-Multi-Set Cross Table Lookups holds between the new table and the first table,
when `is_from_first_table` is turned on, and between the new table and the second table, when `is_from_second_table` is
turned on. There is a very helpful
diagram [here](https://www.notion.so/0xmozak/Cross-Table-Lookup-bbe98d9471114c36a278f0c491f203e5?pvs=4#80f9047bc40f48f29c8ba852bf94c570).

##### Example

We use the Subset CTL to make sure that the ELF instructions executed by the CPU indeed were part of the program. For
that, we create a new `Program` table, which contains all the instructions of the program. Then we create a Subset CTL
between the executed instructions of the CPU and the instructions of the Program table, to make sure that
all the executed instructions are indeed part of the program.

## Table Value Insertion

After we have defined the constraints, we need to fill in the tables with values that actually fulfill the constraints.
This is done in the `generation` module based on the trace of the program and the code of the program.

## Stark Cryptography

As the **Plonky2** API is quite limiting, especially when it comes to the CTL, we had to actually use all the primitives
directly and not their abstractions. This is why in the `stark` module you will find the STARK protocol implementation.
We suggest to pay attention to the `stark/mozak_stark.rs`, `stark/prover.rs` and `stark/verifier.rs` as this is where
the final lookup and regular are enforced over the tables.
Other files have much more cryptography in them, and it is advised to first become familiar with the STARK protocol.