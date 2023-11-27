extern crate alloc;
use alloc::vec::Vec;

use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq, Default)]
#[archive(
    // This will generate a PartialEq impl between our unarchived and archived
    // types:
    compare(PartialEq),
)]
// // Derives can be passed through to the generated type:
#[archive_attr(derive(Debug))]
pub struct TestData {
    pub indices_to_prove: Vec<u32>,
    pub leaves_hashes: Vec<[u8; 32]>,
    pub proof_bytes: Vec<u8>,
}
