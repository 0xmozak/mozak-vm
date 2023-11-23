extern crate alloc;
use alloc::vec::Vec;

use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive(
    // This will generate a PartialEq impl between our unarchived and archived
    // types:
    compare(PartialEq),
)]
// // Derives can be passed through to the generated type:
#[archive_attr(derive(Debug))]
pub struct TestData {
    pub trustedroot: [u8; 32],
    pub merkleproof: Vec<u8>,
}
