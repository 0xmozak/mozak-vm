#![feature(restricted_std)]
extern crate alloc;

use mozak_sdk::common::types::{Event, EventType, ProgramIdentifier, StateObject};

/// Checks if each element of input tape is one off from the other
pub fn raw_tapes_test(tape_1: [u8; 32], tape_2: [u8; 32]) {
    tape_1
        .iter()
        .zip(tape_2.iter())
        .for_each(|(x, y)| assert!(x.wrapping_add(1) == *y));
}
