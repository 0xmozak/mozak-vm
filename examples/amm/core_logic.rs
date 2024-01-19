extern crate alloc;
// use alloc::vec::Vec;
use mozak_sdk::coretypes::ProgramIdentifier;
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

/// `swap_tokens` swaps `objects_presented`, a homogenous (all objects of the same token type)
/// amounting to a cumulative sum of `amount_in` (in denomination of the concerned input token
/// type) for a dynamically calculated `amount_out` of the other token. Swapping operates on
/// a set of objects `objects_requested` economically held by the AMM amounting to a cumulative
/// sum greater than or equal to the dynamic `amount_out`. If `objects_presented` or
/// `objects_requested` are greater than `amounts_in` or `amount_out` respectively, rebalanced
/// objects are presented back with change amounts or new "change" objects are created and 
/// returned. This gets returned in the order `(to_user, presented_change, requested_change)`
pub fn swap_tokens(
    metadata_object: MetadataObject,
    amount_in: Unsigned256,
    objects_presented: Vec<StorageObject>,
    objects_requested: Vec<StorageObject>
) -> (
    Vec<StorageObject>,  // Objects given to the user from the AMM
    Vec<StorageObject>,  // Residual change from `objects_presented`
    Vec<StorageObject>   // Residual change from `objects_requested`
) {
    
}
