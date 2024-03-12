#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![allow(clippy::missing_panics_doc)]
#![feature(trait_alias)]
#![deny(warnings)]
#![cfg_attr(target_os = "mozakvm", feature(restricted_std))]

// ----------- TARGET AGNOSTIC / FOR NATIVE ----------
pub(crate) mod call_tape_native;
pub mod coretypes;
pub(crate) mod event_tape;
pub mod io;
pub(crate) mod native_helpers;
pub mod sys;
pub(crate) mod traits;

// ----------- ONLY FOR MOZAKVM ----------------------
#[cfg(target_os = "mozakvm")]
pub(crate) mod call_tape_vm;
