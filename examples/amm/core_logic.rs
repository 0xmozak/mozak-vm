extern crate alloc;
// use alloc::vec::Vec;
use mozak_sdk::prog::ProgramIdentifier;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, PartialEq, Default)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct MetadataObject {
    /// Token addresses are program addresses for the two token types
    /// held by the AMM
    pub token_addresses: [ProgramIdentifier; 2],

    /// Reserves of tokens on both sides
    pub reserves: [Unsigned256; 2],
}
