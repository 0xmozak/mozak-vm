#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![cfg_attr(target_os = "zkvm", feature(restricted_std))]
pub mod coretypes;
pub mod io;
pub mod sys;
