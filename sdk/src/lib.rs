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

extern crate alloc as rust_alloc;

pub mod core;

#[cfg(feature = "std")]
pub mod common;

#[cfg(all(feature = "std", target_os = "mozakvm"))]
pub mod mozakvm;

#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub mod native;

// ----------- Exported methods -----------------------

// pub enum IOTape {
//     Private,
//     Public,
// }

// /// Emit an event from `mozak_vm` to provide receipts of
// /// `reads` and state updates including `create` and `delete`.
// /// Panics on event-tape non-abidance.
// pub fn event_emit(id: ProgramIdentifier, event: Event) {
//     unsafe { SYSTEM_TAPES.event_tape.emit_event(id, event) }
// }

// /// Receive one message from mailbox targetted to us and its index
// /// "consume" such message. Subsequent reads will never
// /// return the same message. Panics on call-tape non-abidance.
// #[must_use]
// pub fn call_receive() -> Option<(CPCMessage, usize)> {
//     unsafe { SYSTEM_TAPES.call_tape.from_mailbox() }
// }

/// Send one message from mailbox targetted to some third-party
/// resulting in such messages finding itself in their mailbox
/// Panics on call-tape non-abidance.
#[allow(clippy::similar_names)]
pub fn call_send<A, R>(
    recepient_program: crate::common::types::ProgramIdentifier,
    argument: A,
    resolver: impl Fn(A) -> R,
) -> R
where
    A: crate::common::traits::CallArgument + PartialEq,
    R: crate::common::traits::CallReturn,
    <A as rkyv::Archive>::Archived: rkyv::Deserialize<A, rkyv::Infallible>,
    <R as rkyv::Archive>::Archived: rkyv::Deserialize<R, rkyv::Infallible>, {
    use crate::common::traits::Call;
    unsafe {
        crate::common::system::SYSTEM_TAPE
            .call_tape
            .send(recepient_program, argument, resolver)
    }
}

// /// Get raw pointer to access iotape (unsafe) without copy into
// /// buffer. Subsequent calls will provide pointers `num` away
// /// (consumed) from pointer provided in this call for best
// /// effort safety. `io_read` and `io_read_into` would also affect
// /// subsequent returns.
// /// Unsafe return values, use wisely!!
// #[must_use]
// pub fn io_raw_read(_from: &IOTape, _num: usize) -> *const u8 {
// unimplemented!() }

// /// Get a buffer filled with num elements from choice of `IOTape`
// /// in process "consuming" such bytes.
// #[must_use]
// pub fn io_read(_from: &IOTape, _num: usize) -> Vec<u8> { unimplemented!() }

// /// Fills a provided buffer with num elements from choice of `IOTape`
// /// in process "consuming" such bytes.
// pub fn io_read_into(_from: &IOTape, _buf: &mut [u8]) { unimplemented!() }
