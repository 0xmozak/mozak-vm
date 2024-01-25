# Cross Table Lookups

Lookup arguments are often used in zero knowledge virtual machines to optimize performance. Simply speaking,
a lookup argument checks whether or not a set of values \\( S\\) is in a table \\( T \\).

In Mozak-VM, we designed and implemented a Cross Table Lookup (CTL) arugment that is based on [LogUp].

## Lookup Constraints

Lookup constraints are defined over STARK tables, which are then connected between each other using Cross Table Subset Lookups
and Cross Table Permutation Arguments.

## Types of Lookups

### Permutation (Multi-set) Arguments

The simplest argument we will consider is the Permutation Arguments. It checks that two columns (or two set of columns)
have the same values, up to a permutation. Meaning we could reorder values in the column (or rows of a set of columns)
and get the other column (or a set of columns). Each table can support multiple separate Permutation Arguments, and
columns can participate in multiple Permutation Arguments.

The Permutation Argument is so efficient compared to the Subset Argument, that sometimes we use it despite the Subset
Arguments being a natural choice.

This argument is implemented [permutation.rs].

### Cross Table Permutation (Multi-Multi-Set) Arguments

The Subset Arguments described above only work on columns from the one table. However,
sometimes we may want to make use of multiple tables to partition our computations into manageable chunks.
Nevertheless, we have to still link tables to for example make sure they refer to the consistent data,
hence we need arguments that work Cross Table.

In Cross Table Permutation Argument, we define a single _looked_ table and multiple _looking_ tables. The looked and looking tables can also be formed
synthetically, by just grouping a sub-set of columns from the already defined tables. With the tables defined, the Cross
Table Permutation Argument asserts that a [multi-set](https://en.wikipedia.org/wiki/Multiset) union of all rows from the
_looking_ tables is a permutation of rows of the _looked_ table. We also have each _looking_ table define a filter
column that is used to filter out rows that we do not want to participate in the permutation.

To break down the above, let us consider an example. We have a _looked_ table with columns `{x, y, z, allow_to_lookup}`
and a several _looking_ tables which contain columns `{x, y, z, look_up}`. Consider also that
both `allow_to_lookup` and `look_up` columns are boolean. By applying a Cross Table Permutation Argument, we make sure
that the multi-set of `(x, y, z)` values from the _looked_ table, where `allow_to_lookup` is `true`, is a permutation of
a multi-set of `(x, y, z)` values from the _looking_ tables, where `look_up` is `true`. This construction implies
that if a single _looking_ table or several _looking_ tables look up the same row multiple times, then the _looked_
table must have this row multiple times, as each row can only be looked up once by any _looking_ table.

This argument is implemented [cross_table_lookup.rs].

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
for rows where `is_from_looked_table` indicator is turned on.

Finally, like with Subset Lookups, we require that for each row in the new table:

- Either the row is not from the looking table.
- Or the row value is the same as next row value (excluding indicator columns)
- Or the is from the looked table is true.

[LogUp]: https://eprint.iacr.org/2022/1530.pdf
[permutation.rs]: ../../circuits/src/stark/permutation.rs
[cross_table_lookup.rs]: ../../circuits/src/cross_table_lookup.rs
