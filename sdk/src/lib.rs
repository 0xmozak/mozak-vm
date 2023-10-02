#![cfg_attr(not(feature = "std"), no_std)]

pub use id::Id;
pub use object::{data, program, Object, ObjectContent};
pub use transition::{Transition, TransitionInput};

pub mod id;
pub mod object;
mod transition;
