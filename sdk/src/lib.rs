#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![allow(clippy::missing_panics_doc)]
#![feature(trait_alias)]
#![deny(warnings)]
#![cfg_attr(target_os = "mozakvm", feature(restricted_std))]

// TODO(Matthias): Remove these once the big sdk refactor is in.
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::default_trait_access)]
#![allow(clippy::deref_addrof)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::implicit_clone)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::needless_return)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::single_char_pattern)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::unnecessary_join)]

pub mod coretypes;
pub mod io;
#[cfg(not(target_os = "mozakvm"))]
pub(crate) mod native_helpers;
pub mod sys;
