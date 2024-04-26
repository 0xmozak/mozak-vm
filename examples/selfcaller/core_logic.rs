#![feature(restricted_std)]
#![allow(unused_attributes)]
extern crate alloc;

use mozak_sdk::common::types::ProgramIdentifier;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[cfg_attr(not(target_os = "mozakvm"), derive(Debug))]
pub enum MethodArgs {
    SelfCall(ProgramIdentifier, u8),
}

#[derive(Archive, Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub enum MethodReturns {
    SelfCall(u8),
}

// TODO: Remove later
impl Default for MethodReturns {
    fn default() -> Self { Self::SelfCall(0) }
}

#[allow(clippy::unit_arg)]
pub fn dispatch(args: MethodArgs) -> MethodReturns {
    match args {
        MethodArgs::SelfCall(self_id, counter) => self_call(self_id, counter),
    }
}

pub fn self_call(self_id: ProgramIdentifier, counter: u8) -> MethodReturns {
    if counter == 0 {
        return MethodReturns::SelfCall(0);
    }
    mozak_sdk::call_send(
        self_id,
        MethodArgs::SelfCall(self_id, counter - 1),
        dispatch,
    );

    MethodReturns::SelfCall(counter)
}
