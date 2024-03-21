#![feature(restricted_std)]
#![allow(unused_attributes)]
mod core_logic;

use mozak_sdk::common::types::{Poseidon2Hash, ProgramIdentifier};

use crate::core_logic::{dispatch, MethodArgs};

fn main() {
    let token_program = ProgramIdentifier::new_from_rand_seed(1);

    let raw_buf_1 = Poseidon2Hash::new_from_rand_seed(1).inner();
    let raw_buf_2 = raw_buf_1
        .iter()
        .map(|x| x.wrapping_add(1))
        .collect::<Vec<u8>>();

    mozak_sdk::call_send(
        token_program,
        MethodArgs::RawTapesTest(
            raw_buf_1,
            <&[u8] as TryInto<[u8; 32]>>::try_into(&raw_buf_2[0..32])
                .expect("Vec<u8> must have exactly 32 elements")
                .into(),
        ),
        dispatch,
    );

    mozak_sdk::native::dump_system_tape("inputtape", true);
}
