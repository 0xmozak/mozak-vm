extern crate alloc;
// use alloc::vec::Vec;
use mozak_sdk::coretypes::{ProgramIdentifier, StateObject};
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, PartialEq, Default)]
#[archive(compare(PartialEq))]
#[archive_attr(derive(Debug))]
#[cfg_attr(not(target_os = "zkvm"), derive(Debug))]
pub struct MetadataObject {
    /// Token addresses are program addresses for the two token types
    /// held by the AMM
    pub token_programs: [ProgramIdentifier; 2],

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
/// returned. It is assumed that only the last element of both `objects_presented` and 
/// `objects_requested` will ever be subject to such rebalancing. This gets returned in the 
/// order `(to_user, presented_change, requested_change)`
pub fn swap_tokens<'a>(
    metadata_object: MetadataObject,
    amount_in: Unsigned256,
    objects_presented: Vec<StateObject<'a>>,
    objects_requested: Vec<StateObject<'a>>
) -> (
    Vec<StateObject<'a>>,  // Objects given to the user from the AMM
    Option<StateObject<'a>>,  // Residual change from `objects_presented`
    Option<StateObject<'a>>   // Residual change from `objects_requested`
) {
    let idx_in = if objects_presented.is_empty() {
        panic!("no objects presented for swap");
    } else {
        (objects_presented[0].constraint_owner != metadata_object.token_programs[0]) as usize
    };

    let idx_out = 1 - idx_in;

    let current_price = metadata_object.reserves[idx_out] / metadata_object.reserves[idx_in];
    let amount_out = current_price * amount_in;

    if amount_out > metadata_object.reserves[idx_out] {
        panic!("cannot process swap, insufficient funds");
    }

    metadata_object.token_programs[idx_in].ensure_owners(objects_presented.iter());
    metadata_object.token_programs[idx_out].ensure_owners(objects_requested.iter());

    let (total_presented, last_presented) = extract_amounts(metadata_object[idx_in], objects_presented);
    let (total_requested, last_requested) = extract_amounts(metadata_object[idx_out], objects_requested);

    if total_presented < amount_in && (total_presented - last_presented) > amount_in {
        panic!("invalid token objects presented for transaction");
    }
    if total_requested < amount_out && (total_requested - last_requested) > amount_out {
        panic!("invalid token objects requested for transaction");
    }

    if last_presented > 0 {
        // Split last element
    }
    if last_requested > 0 {
        // Split last element
    }

    // send the elements to amm
    // send the elements to user

    return (vec![], vec![], vec![]);
}
