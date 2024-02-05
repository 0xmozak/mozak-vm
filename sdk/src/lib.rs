#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![feature(trait_alias)]
#![cfg_attr(target_os = "zkvm", feature(restricted_std))]
pub mod coretypes;
pub mod io;
pub mod sys;
