#![feature(restricted_std)]
#![allow(unused_attributes)]
mod core_logic;

use mozak_sdk::common::types::{Poseidon2Hash, ProgramIdentifier};

use crate::core_logic::{dispatch, MethodArgs};

fn main() {
    let token_program = ProgramIdentifier::new_from_rand_seed(3);

    let buf1 = Poseidon2Hash::new_from_rand_seed(2).inner();
    let buf2 = buf1.iter().map(|x| x.wrapping_add(1)).collect::<Vec<u8>>();

    mozak_sdk::add_identity(token_program); // Manual override for `IdentityStack`
    let _ = mozak_sdk::write(&mozak_sdk::InputTapeType::PublicTape, &buf1);
    let _ = mozak_sdk::write(&mozak_sdk::InputTapeType::PrivateTape, &buf2[..]);
    mozak_sdk::rm_identity(); // Manual override for `IdentityStack`

    mozak_sdk::call_send(token_program, MethodArgs::RawTapesTest, dispatch);

    mozak_sdk::native::dump_proving_files("inputtape");
}
