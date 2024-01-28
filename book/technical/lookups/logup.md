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

This relation is checked by using the [sumcheck protocol].

<!-- TODO: ctl -->

[LogUp]: https://eprint.iacr.org/2022/1530.pdf
[sumcheck protocol]: https://dl.acm.org/doi/pdf/10.1145/146585.146605
