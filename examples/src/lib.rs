#![cfg_attr(target_os = "zkvm", feature(restricted_std))]
#![cfg_attr(target_os = "zkvm", no_main)]

use std::ops::Deref;

use mozak_node_sdk::TransitionInput;
use postcard::from_bytes;

use crate::io::MozakIo;

pub mod io;

pub fn read_input(mut reader: MozakIo) -> TransitionInput {
    // Read all input bytes from the tape to a buffer.
    let serialized_bytes = reader.read_all().expect("Could not read input bytes");

    // Deserialize the buffer into TransitionInput.

    let input = from_bytes(serialized_bytes.deref()).expect("Could not deserialize input");

    input
}

/// A trait that represents a transition function.
/// It abstracts away the logic of reading the input and writing the output.
/// It also provides a default implementation of the run method.
/// The run method is the entry point of the transition program.
pub trait Transition {
    /// The entry point of the transition program.
    /// It reads the input from `std` stream, validates it and writes the
    /// output. If executed in native mode, it also writes the read input to
    /// the `iotape` file. This is done to later rerun the transition
    /// program in zkvm, where `iotape` would act as the input.
    fn run() {
        let reader = MozakIo::default();

        let transition_input = read_input(reader);
        let valid = Self::validate(transition_input);

        assert!(valid, "Transition is not valid");

        #[cfg(target_os = "zkvm")]
        guest::env::write(&(valid as u32).to_le_bytes());
    }

    /// Validates the transition and returns if it is valid or not.
    fn validate(transition_input: TransitionInput) -> bool;
}

/// This macro sets up the entry point for a `zkvm` target, and provides a
/// default `main` function for non-`zkvm` targets. It takes a single argument:
/// the name of a struct which implements a `run` method to be used as the entry
/// point for `zkvm`
#[macro_export]
macro_rules! setup_main {
    ($struct_name:ident) => {
        #[cfg(target_os = "zkvm")]
        guest::entry!($struct_name::run);

        #[cfg(not(target_os = "zkvm"))]
        fn main() {}
    };
}
