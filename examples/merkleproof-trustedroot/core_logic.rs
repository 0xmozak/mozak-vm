extern crate alloc;
use alloc::vec::Vec;
use rkyv::with::{Inline, With};
use std::fs::File;
use std::io::{Read, Write};

use rkyv::ser::serializers::AllocSerializer;
use rkyv::{Archive, Deserialize, Serialize, Fallible, Infallible};
use rs_merkle::algorithms::Sha256;
use rs_merkle::MerkleProof;

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Default)]
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

pub type MerkleRootType = [u8; 32];

// TODO: shift to SDK
pub fn to_tape_function_id(public_tape: &mut File, id: u8) {
    public_tape
        .write(&[id])
        .expect("failure while writing function ID");
}

// TODO: shift to SDK
pub fn from_tape_function_id(public_tape: &mut File) -> u8 {
    let mut function_id_buffer = [0u8; 1];
    public_tape
        .read(&mut function_id_buffer)
        .expect("failure while reading function ID");
    function_id_buffer[0]
}

// TODO: shift to SDK
pub fn to_tape_rawbuf(tape: &mut File, buf: &[u8]) {
    tape.write(buf).expect("failure while writing raw buffer");
}

// TODO: shift to SDK
pub fn from_tape_rawbuf<const N: usize>(tape: &mut File) -> [u8; N] {
    let mut buf = [0u8; N];
    tape.read(&mut buf).expect("failure while reading raw buffer");
    buf
}

// TODO: shift to SDK
pub fn to_tape_serialized<T, const N: usize>(tape: &mut File, object: &T)
where
    T: Serialize<AllocSerializer<N>>, {
    let serialized_obj = rkyv::to_bytes::<_, N>(object).unwrap();
    let serialized_obj_len = (serialized_obj.len() as u32).to_le_bytes();
    tape.write(&serialized_obj_len)
        .expect("failure while writing serialized obj len prefix");
    tape.write(&serialized_obj)
        .expect("failure while writing serialized obj");
}

// TODO: shift to SDK
pub fn from_tape_serialized<T, const N: usize>(tape: &mut File) -> T
where
    T: Archive, 
    T::Archived: Deserialize<T, dyn Fallible<Error = rkyv::Infallible>>,
{
    let mut length_prefix = [0u8; 4];
    tape.read(&mut length_prefix)
        .expect("read failed for length prefix");
    let length_prefix = u32::from_le_bytes(length_prefix);

    let mut obj_buf = Vec::with_capacity(length_prefix as usize);
    obj_buf.resize(length_prefix as usize, 0);

    tape.read(&mut obj_buf[0..(length_prefix as usize)])
        .expect("read failed for obj");

    let archived = unsafe { rkyv::archived_root::<T>(&obj_buf) };
    // let a = Infallible::deserialize(&self, deserializer)
    let t: T = archived.deserialize(&mut rkyv::Infallible).unwrap();
    t
}

pub fn verify_merkle_proof(merkle_root: MerkleRootType, proof_data: ProofData) {
    let proof = MerkleProof::<Sha256>::try_from(proof_data.proof_bytes).unwrap();
    let indices: Vec<usize> = proof_data
        .indices_to_prove
        .iter()
        .map(|&x| x as usize)
        .collect();
    assert!(proof.verify(
        merkle_root,
        &indices[..],
        proof_data.leaf_hashes.as_slice(),
        proof_data.leaves_len as usize,
    ));
}
