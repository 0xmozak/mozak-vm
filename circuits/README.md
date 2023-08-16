# Circuits Sub-crate

The **Circuits** sub-crate contains the constraints to enforce the correctness of the RISC-V VM trace.

The constraints are based on [Plonky2](https://github.com/mir-protocol/plonky2), in particular
its [Starky](https://github.com/mir-protocol/plonky2/tree/main/starky) variation with an addition of Cross Table
Lookups (CTL).

## Constraints

The constraints are defined over STARK tables, which are then connected between each other using Cross Table Lookups.

### Tables Structure

Each Table is defined in its own module and contains definition of used columns, in `columns.rs`, as well as definition
of constraints to be applied over these columns, in `stark.rs`. To read more about Starks, you can refer
to [this page](https://www.notion.so/0xmozak/eb420963310e407dafce95d267f5a55e?v=a6a42a89bb744309999d9a0ff16ce25f&p=0d7f4fe31e214c3aa9b719908ac56e57&pm=s).
In order to make tables columns easier to interpret, we make use the `View` wrappers, that allow to access each column
of a table by name and not just by index. This makes development less error-prone.

### Constraints Degree

We try to limit all constraints degrees to `3`, as the higher the degree, the longer is the proving time. In general,
one can reason of the proving time as _**O(num_rows * num_cols * num_constraints * constraints_degree)**_, thus we are
constantly balancing these values.

### Padding Rows

Due to how STARKs work, we need to always pad tables to a size which is a multiple of two (likely, for FRI to work).
Furthermore, if we use several STARK tables and connect them using CTL, they all need to be of the same length.
Hence, a lot of times you will see padding rows added to the table, as well as some selector that tells if the row is a
padding row or not.

### Constraints Declaration

Constraints assert that certain multi-variable polynomial of degree less than maximum constraints degrees evaluate to
zero for each row of the table. This polynomial can also depend on column values from the following or presiding rows of
the table.

#### Example

An example of such constraint can be making sure a value of column `x` is 0 or 1 when the padding row selector is 0.
The following will achieve this:

```ignore
    yield_constr.constraint((1 - lv.x) * (lv.x) * (1 - lv.is_padding));
```

As some constraints, such the one above, are used very frequently, we have introduced aliases for then. One of them
is `is_binary`, which asserts that the value of a column is indeed 0 or 1.

You will also find that for more complex
constraints, such as that CPU table row transition happened correctly, we partition them into smaller constraint
functions, good example of that being the `div` sub-module of the `cpu` module, which defines constraints for when the
row is a division operation.

### Selectors

Above was a great example of how selectors work. In particular, when we need to turn on or off a constraint based on
some logic, we make user of selectors.
If you multiply a constraint by the selector, if the selector is 0, then the constraint no longer needs to hold, as a
multiple of zero is always zero.

One then can apply some constraint logic on the selectors. For example in the CPU we enable constraints for particular
opcodes using the opcode selectors. Yet to make sure each row has at least one opcode active, we enforce that at least
one of all opcode selectors is non-zero.

You might notice that not all opcodes constraints have selectors. This is because for some operations there is no harm
to be enabled everywhere, as they do not collude with anything else.

### Helper Columns

Sometimes computing a value is harder than verification. A good example of that is public key cryptography, where
extracting `a`, such that `b=g^a` is much harder than verifying that `a` is indeed correct as `b==g^a` holds. Such
asymmetry is especially prevalent in the constraint system, where we do not need to actually proved we calculated `a`
correctly, rather that `a` we provided fulfills the `b==g^a`.
In this case, `a` can be placed into the helper column, it will serve as a hint to the constraint system and make it
simpler to calculate the values.

Another example of value useful to be placed into a helper column is inverse, that can be used to prove that a value is
not zero. Calculating that in constraints is very tough, but we do not need to, all we need is to show that there is a
value that multiplied by the other value in question, equals 1.

Though we must point out that sometimes adding a helper column is unjustified. If the helper value is just a linear or
low-degree combination of other columns, it is cheaper to just keep it as a low degree combination, possibly adding an
alias for easier use.

### Cross Table Lookups (CTL)

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

Lookups are rather simpler than CTL and work as CTL happens on a single table. For that, we select two columns and say
that the values of the columns must be the same up to a row position permutation. A good example of a lookup is in the
RangeCheck table.

There we need to show that lo (hi) limb is a u16. For that, we first create a column with all u16
values, and then
create another permuted a column of lo (hi) values in question to match the position of values in teh u16 column.
Finally, we make sure that values in the permuted column are equal to the values in the original column with all u16
values in each row.

## Table Population

After we have defined the constraints, we need to fill in the tables with values that actually fulfill the constraints.
This is done in the `generation` module based on the trace of the program and the code of the program.

## Stark Cryptography

As the **Plonky2** API is quite limiting, especially when it comes to the CTL, we had to actually use all the primitives
directly and not their abstractions. This is why in the `stark` module you will find the STARK protocol implementation.
We suggest to pay attention to the `stark/mozak_stark.rs`, `stark/prover.rs` and `stark/verifier.rs` as this is where
the final lookup and regular are enforced over the tables.
Other files have much more cryptography in them, and it is advised to first become familiar with the STARK protocol.