# LogUp

Given two sets \\(S\\) and \\(T\\). [LogUp] checks whether the elements in \\(S\\) is a subset of \\(T\\) by checking
the following relation

$$
\sum_{i=0}^{k} \frac{1}{(\alpha - s_i)} = \sum_{i=0}^{n} \frac{m_i}{(\alpha - t_i)}
$$

where
* k is the number of elements in the set \\(S\\).
* \\(\alpha \\) is a random challenge from the verifier to the prover.
* \\(s_i\\) is an element in the set \\(S\\).
* \\(n\\) is the number of elements in the set \\(T\\).
* \\(t_i\\) is an element in the set \\(T\\).
* \\(m_i \\) is the multiplicity of \\(t_i\\). The number of times \\(t_i\\) appeared in set \\(S\\)

### Combining tables with random challenges

The CTL argument argues that values from multiple tables are in a single table. We call "multiple tables" looking tables
and "a single table" looked table in our codebase.

To combine multiple tables, we use two random challenges \\( \beta \\) and \\( \gamma \\) from the verifier, which are generated using
the Fiat-Shamir Heuristic in the non-interative setting. Let \\(s_0, s_1, s_2 ...\\) be values from a table, they are combined as

$$
s_0*\beta^{n-1} + s_1*\beta^{n-2} + ... + s_{n-1} + \gamma
$$

Both values from the looking tables and the looked table is combined like this.

### Quotient polynomial to check the equality of the relation

In the LogUp paper, the relation above is checked with the [sumcheck protocol]. Since Mozak-VM is using LogUp in a STARK setting, the sumcheck protocol
check is replaced with a quotient polynomial check. i.e. two polynomial are the same if a random evaluation of their quotient polynomial is the same.
The quotient polynomial \\( h(x) \\) is aquired by a division of the vanishing polynomial \\( Z_H(x) \\).

$$
g(x) = h(x) * Z_H(x)
$$

The cross table lookup is implemented in [cross_table_lookup.rs].

[LogUp]: https://eprint.iacr.org/2022/1530.pdf
[sumcheck protocol]: https://dl.acm.org/doi/pdf/10.1145/146585.146605
[cross_table_lookup.rs]: ../../circuits/src/cross_table_lookup.rs
