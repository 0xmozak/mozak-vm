#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![allow(clippy::missing_panics_doc)]
#![feature(trait_alias)]
#![cfg_attr(target_os = "mozakvm", feature(restricted_std))]
pub mod coretypes;
pub mod hash;
pub mod io;
pub mod sys;
