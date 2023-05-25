#![deny(clippy::pedantic)]
#![deny(clippy::cargo)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![allow(clippy::if_not_else)]
// Some of the below might be better to deny here and allow on a case-by-case basis in the code.
// This is just a first cut.

// However, pendantic lint can be _very_, well, pedantic.  So feel free to add exceptions locally or
// even globally for the whole crate.
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::needless_pass_by_value)]

pub mod decode;
pub mod elf;
pub mod instruction;
pub mod state;
pub mod vm;

extern crate alloc;
