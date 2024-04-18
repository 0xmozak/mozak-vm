#![feature(restricted_std)]
#![allow(unused_attributes)]

mod core_logic;

use mozak_sdk::common::types::ProgramIdentifier;

use crate::core_logic::{dispatch, MethodArgs};

fn main() {
    let self_caller_program = ProgramIdentifier::new_from_rand_seed(5);

    mozak_sdk::call_send(
        self_caller_program,
        MethodArgs::SelfCall(self_caller_program, 5),
        dispatch,
    );

    mozak_sdk::native::dump_proving_files("selfcaller", self_caller_program);
}
