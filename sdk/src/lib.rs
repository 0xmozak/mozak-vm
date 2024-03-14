#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![allow(clippy::missing_panics_doc)]
#![feature(trait_alias)]
#![feature(raw_ref_op)]
#![feature(stmt_expr_attributes)]
#![deny(warnings)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", feature(restricted_std))]
// #![cfg_attr(target_os = "mozakvm", feature(restricted_std))]

extern crate alloc as rust_alloc;

pub mod core;

#[cfg(feature = "std")]
pub mod common;

#[cfg(all(feature = "std", target_os = "mozakvm"))]
pub(crate) mod mozakvm;

#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub(crate) mod native;

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

// /// Send one message from mailbox targetted to some third-party
// /// resulting in such messages finding itself in their mailbox
// /// Panics on call-tape non-abidance.
// #[allow(clippy::similar_names)]
// pub fn call_send<A, R>(
//     caller_prog: ProgramIdentifier,
//     callee_prog: ProgramIdentifier,
//     call_args: A,
//     dispatch_native: impl Fn(A) -> R,
//     dispatch_mozakvm: impl Fn() -> R,
// ) -> R
// where
//     A: crate::traits::CallArgument + PartialEq,
//     R: crate::traits::CallReturn,
//     <A as Archive>::Archived: Deserialize<A, rkyv::Infallible>,
//     <R as Archive>::Archived: Deserialize<R, rkyv::Infallible>, {
//     unsafe {
//         SYSTEM_TAPES.call_tape.to_mailbox(
//             caller_prog,
//             callee_prog,
//             call_args,
//             dispatch_native,
//             dispatch_mozakvm,
//         )
//     }
// }

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
