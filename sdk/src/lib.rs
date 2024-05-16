#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![allow(clippy::missing_panics_doc)]
#![feature(trait_alias)]
#![feature(raw_ref_op)]
#![feature(stmt_expr_attributes)]
#![deny(warnings)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", feature(restricted_std))]

#[cfg(feature = "std")]
use rkyv::rancor::{Panic, Strategy};
#[cfg(feature = "std")]
use rkyv::Deserialize;

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

pub enum InputTapeType {
    PublicTape,
    PrivateTape,
}

#[cfg(all(feature = "std", target_os = "mozakvm"))]
pub use crate::mozakvm::helpers::poseidon2_hash_no_pad;
#[cfg(all(feature = "std", target_os = "mozakvm"))]
pub use crate::mozakvm::helpers::poseidon2_hash_with_pad;
/// Provides the length of tape available to read
#[cfg(all(feature = "std", target_os = "mozakvm"))]
pub use crate::mozakvm::inputtape::input_tape_len;
/// Reads utmost given number of raw bytes from an input tape
#[cfg(all(feature = "std", feature = "stdread", target_os = "mozakvm"))]
pub use crate::mozakvm::inputtape::read;
/// Manually add a `ProgramIdentifier` onto `IdentityStack`. Useful
/// when one want to escape automatic management of `IdentityStack`
/// via cross-program-calls sends (ideally temporarily).
/// CAUTION: Manual function for `IdentityStack`, misuse may lead
/// to system tape generation failure.
#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub use crate::native::identity::add_identity;
/// Manually remove a `ProgramIdentifier` from `IdentityStack`.
/// Useful when one want to escape automatic management of `IdentityStack`
/// via cross-program-calls sends (ideally temporarily).
/// CAUTION: Manual function for `IdentityStack`, misuse may lead
/// to system tape generation failure.
#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub use crate::native::identity::rm_identity;
/// Writes raw bytes to an input tape. Infallible
#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub use crate::native::inputtape::write;
