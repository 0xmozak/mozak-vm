#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![allow(clippy::missing_panics_doc)]
#![feature(trait_alias)]
#![feature(raw_ref_op)]
#![feature(stmt_expr_attributes)]
#![feature(slice_ptr_len)]
#![deny(warnings)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", feature(restricted_std))]

#[cfg(feature = "std")]
use rkyv::rancor::{Panic, Strategy};
#[cfg(feature = "std")]
use rkyv::Deserialize;
#[cfg(feature = "std")]
use rkyv::ser::AllocSerializer;

extern crate alloc as rust_alloc;

pub mod core;

#[cfg(feature = "std")]
pub mod common;

#[cfg(all(feature = "std", target_os = "mozakvm"))]
pub mod mozakvm;

#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub mod native;

/// Emit an event from `mozak_vm` to provide receipts of
/// `reads` and state updates including `create` and `delete`.
/// Panics on event-tape non-abidance.
#[cfg(feature = "std")]
pub fn event_emit(event: crate::common::types::Event) {
    use crate::common::traits::EventEmit;
    unsafe {
        crate::common::system::SYSTEM_TAPE.event_tape.emit(event);
    }
}

/// Receive one message from mailbox targetted to us and its index
/// "consume" such message. Subsequent reads will never
/// return the same message. Panics on call-tape non-abidance.
#[cfg(feature = "std")]
#[must_use]
pub fn call_receive<A, R>() -> Option<(crate::common::types::ProgramIdentifier, A, R)>
where
    A: crate::common::traits::CallArgument + PartialEq,
    R: crate::common::traits::CallReturn,
    <A as rkyv::Archive>::Archived: Deserialize<A, Strategy<(), Panic>>,
    <R as rkyv::Archive>::Archived: Deserialize<R, Strategy<(), Panic>>, {
    use crate::common::traits::Call;
    unsafe { crate::common::system::SYSTEM_TAPE.call_tape.receive() }
}

/// Send one message from mailbox targetted to some third-party
/// resulting in such messages finding itself in their mailbox
/// Panics on call-tape non-abidance.
#[cfg(feature = "std")]
#[allow(clippy::similar_names)]
pub fn call_send<A, R>(
    recipient_program: crate::common::types::ProgramIdentifier,
    argument: A,
    resolver: impl Fn(A) -> R,
) -> R
where
    A: crate::common::traits::CallArgument + PartialEq,
    R: crate::common::traits::CallReturn,
    <A as rkyv::Archive>::Archived: Deserialize<A, Strategy<(), Panic>>,
    <R as rkyv::Archive>::Archived: Deserialize<R, Strategy<(), Panic>>, {
    use crate::common::traits::Call;
    unsafe {
        crate::common::system::SYSTEM_TAPE
            .call_tape
            .send(recipient_program, argument, resolver)
    }
}
