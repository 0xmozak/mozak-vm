#![cfg_attr(not(feature = "std"), no_std)]

pub use id::Id;
pub use object::{data, program, Object, ObjectContent};
pub use transition::Transition;

pub mod id;

mod object;
mod transition;
