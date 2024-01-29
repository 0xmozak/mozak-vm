extern crate alloc;

use mozak_sdk::coretypes::{Address, ProgramIdentifier, StateObject};
use mozak_sdk::cpc::cross_program_call;
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
    pub reserves: [u64; 2],
}

/// `swap_tokens` swaps `objects_presented`, a homogenous (all objects of the
/// same token type) amounting to a cumulative sum of `amount_in` (in
/// denomination of the concerned input token type) for a dynamically calculated
/// `amount_out` of the other token. Swapping operates on a set of objects
/// `objects_requested` economically held by the AMM amounting to a cumulative
/// sum greater than or equal to the dynamic `amount_out`. If
/// `objects_presented` or `objects_requested` are greater than `amounts_in` or
/// `amount_out` respectively, rebalanced objects are presented back with change
/// amounts or new "change" objects are created and returned. It is assumed that
/// only the last element of both `objects_presented` and `objects_requested`
/// will ever be subject to such rebalancing. This gets returned in the
/// order `(presented_change, requested_change)`
pub fn swap_tokens<'a>(
    metadata_object: MetadataObject,
    amount_in: u64,
    user_wallet: ProgramIdentifier,
    objects_presented: Vec<StateObject<'a>>,   
    objects_requested: Vec<StateObject<'a>>,
    available_state_addresses: [Address; 2],
    self_prog_id: ProgramIdentifier,
) -> (
    Option<StateObject<'a>>, // Residual change from `objects_presented`
    Option<StateObject<'a>>, // Residual change from `objects_requested`
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

    metadata_object.token_programs[idx_in]
        .ensure_constraint_owner_similarity(objects_presented.iter());
    metadata_object.token_programs[idx_out]
        .ensure_constraint_owner_similarity(objects_requested.iter());

    let (total_presented, last_presented) = extract_amounts(&objects_presented);
    let (total_requested, last_requested) = extract_amounts(&objects_requested);

    if total_presented < amount_in && (total_presented - last_presented) > amount_in {
        panic!("invalid token objects presented for transaction");
    }
    if total_requested < amount_out && (total_requested - last_requested) > amount_out {
        panic!("invalid token objects requested for transaction");
    }

    let mut residual_presented: Option<StateObject<'a>> = None;
    if last_presented > 0 {
        let remaining = total_presented - amount_in;
        let calldata: Vec<u8> = available_state_addresses[0]
            .get_raw()
            .iter()
            .chain(remaining.to_le_bytes().iter())
            .cloned()
            .collect();

        residual_presented = Some(cross_program_call::<StateObject>(
            metadata_object.token_programs[idx_in],
            stablecoin::Methods::Split as u8,
            calldata,
        ));
    }
    let mut residual_requested: Option<StateObject<'a>> = None;
    if last_requested > 0 {
        let remaining = total_requested - amount_out;
        let calldata: Vec<u8> = available_state_addresses[0]
            .get_raw()
            .iter()
            .chain(remaining.to_le_bytes().iter())
            .cloned()
            .collect();

        residual_requested = Some(cross_program_call::<StateObject>(
            metadata_object.token_programs[idx_out],
            stablecoin::Methods::Split as u8,
            calldata,
        ));
    }

    objects_presented.iter().for_each(|x| {
        let calldata: Vec<u8> = x
            .address
            .get_raw()
            .iter()
            .chain(self_prog_id
        .to_le_bytes().iter())
            .cloned()
            .collect();
        cross_program_call::<()>(
            x.constraint_owner,
            stablecoin::Methods::Transfer as u8,
            calldata,
        );
    });

    objects_requested.iter().for_each(|x| {
        let calldata: Vec<u8> = x
            .address
            .get_raw()
            .iter()
            .chain(user_wallet.to_le_bytes().iter())
            .cloned()
            .collect();
        cross_program_call::<()>(
            x.constraint_owner,
            stablecoin::Methods::Transfer as u8,
            calldata,
        );
    });

    (residual_presented, residual_requested)
}

#[must_use]
fn extract_amounts(objects: &Vec<StateObject<'_>>) -> (u64, u64) {
    let mut total_amount = 0;
    let mut last_amount = 0;
    for obj in objects {
        last_amount = cross_program_call(
            obj.constraint_owner,
            stablecoin::Methods::GetAmount as u8,
            obj.data.to_vec(),
        );
        total_amount += last_amount;
    }
    (total_amount, last_amount)
}
