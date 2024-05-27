#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![allow(clippy::missing_panics_doc)]
#![feature(trait_alias)]
#![deny(warnings)]
#![allow(unexpected_cfgs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", feature(restricted_std))]

extern crate alloc as rust_alloc;

pub mod core;

#[cfg(feature = "std")]
pub mod common;

#[cfg(feature = "std")]
pub use crate::common::system::{call_receive, call_send, event_emit};

#[cfg(all(feature = "std", target_os = "mozakvm"))]
pub mod mozakvm;

#[cfg(all(feature = "std", not(target_os = "mozakvm")))]
pub mod native;

/// Provides the length of tape available to read
#[cfg(all(feature = "std", target_os = "mozakvm"))]
pub use crate::mozakvm::inputtape::input_tape_len;
/// Reads utmost given number of raw bytes from an input tape
#[cfg(all(feature = "std", feature = "stdread", target_os = "mozakvm"))]
pub use crate::mozakvm::inputtape::read;
#[cfg(all(feature = "std", target_os = "mozakvm"))]
pub use crate::mozakvm::poseidon::poseidon2_hash_no_pad;
#[cfg(all(feature = "std", target_os = "mozakvm"))]
pub use crate::mozakvm::poseidon::poseidon2_hash_with_pad;
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

pub enum InputTapeType {
    PublicTape,
    PrivateTape,
}
