#![feature(restricted_std)]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use mozak_node_sdk::Object;

pub mod io;

pub fn deserialize_input() -> Vec<Object> { vec![] }

pub const TMP: &str = "tmp";
