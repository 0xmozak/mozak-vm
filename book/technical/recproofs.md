# Recproofs

Mozak uses [recproofs] to efficiently produce proofs of the states, which are commited through a Merkle Tree.

Recproofs enable

1. Nice batching property. \\( O(nlogn) \\) time to compute the proof in parallel.
2. Updatable in \\( O(nlogn) \\) time if one of the leafs changes.

whereas a naive vector commitment requires \\( O(n) \\) time to update the proofs.

## Mozak State Commitment
In Mozak, the state is committed through a binary merkle tree where each leaf has a binary index and a value.

## Canonical Hash
At a high level, recproofs achieves batch opening of leafs whose indices are in the index set \\( I \\) by keeping a "canonical hash" of the set \\( I \\) in addition to the Merkle hash at each branch and leaf. We call this "canonical hash" from the paper "summary_hash" in our codebase.

When the verifier verify the proof through the merkle authentication path recursively, it verifiers both the canonical hash and the merkle hash are hashed correctly. At the leaf node of the tree, the verifier checks that the canonical hash of the leaf is equal to its merkle hash.

We refer the reader to the [recproofs] paper for a graphical visualization and pseudocode  of the scheme. Our implementation of recproof is in [this folder]. To checkout how the state interacts with the circuit, checkout [state.rs]


[recproofs]: https://uploads-ssl.webflow.com/6460ebf2b6ff254688bebf1c/64e4dd54d9198fde8d58ef44_main.pdf
[this folder]: ../../circuits/src/recproof
[state.rs]: ../../node/src/block_proposer/state.rs
