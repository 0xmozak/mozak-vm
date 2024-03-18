extern crate alloc;

use rkyv::{Archive, Deserialize, Serialize};
use rs_merkle::algorithms::Sha256;
use rs_merkle::MerkleProof;

#[derive(Archive, Deserialize, Serialize, PartialEq, Default)]
#[archive(
    // This will generate a PartialEq impl between our unarchived and archived
    // types:
    compare(PartialEq),
)]
// // Derives can be passed through to the generated type:
#[archive_attr(derive(Debug))]
pub struct ProofData {
    /// Indices in merkle tree to be proven
    pub indices_to_prove: Vec<u32>,

    /// Hashes of the leaves at the indices
    /// that we intend to prove
    pub leaf_hashes: Vec<[u8; 32]>,

    /// Serialized (non-rkyv) bytes that merkle-prove
    /// multiple `leaf_hashes` against a trust `merkle_root`
    pub proof_bytes: Vec<u8>,

    /// Number of leaves in the merkle tree
    pub leaves_len: u32,
}

#[cfg(not(target_os = "mozakvm"))]
impl std::fmt::Debug for ProofData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProofData")
            .field("indices_to_prove", &self.indices_to_prove)
            .field(
                "leaf_hashes",
                &self
                    .leaf_hashes
                    .iter()
                    .map(|x| hex::encode(x))
                    .collect::<Vec<String>>(),
            )
            .field("proof_bytes", &hex::encode(&self.proof_bytes))
            .field("leaves_len", &self.leaves_len)
            .finish()
    }
}

pub type MerkleRootType = [u8; 32];

pub fn verify_merkle_proof(merkle_root: MerkleRootType, proof_data: ProofData) {
    let proof = MerkleProof::<Sha256>::try_from(proof_data.proof_bytes).unwrap();
    let indices: Vec<usize> = proof_data
        .indices_to_prove
        .iter()
        .map(|&x| x as usize)
        .collect();
    let verified = proof.verify(
        merkle_root,
        &indices[..],
        proof_data.leaf_hashes.as_slice(),
        proof_data.leaves_len as usize,
    );
    assert!(verified, "merkle proof verification failed");

    guest::env::write(&[verified.into(); 1]);
}
