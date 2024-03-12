#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![allow(clippy::missing_panics_doc)]
#![feature(trait_alias)]
#![deny(warnings)]
#![cfg_attr(target_os = "mozakvm", feature(restricted_std))]
pub mod coretypes;
pub mod io;
#[cfg(not(target_os = "mozakvm"))]
pub(crate) mod native_helpers;
pub mod sys;
