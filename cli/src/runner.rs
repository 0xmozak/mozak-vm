//! Utility functions that helps the CLI to interact with the
//! [Mozak runner crate](mozak_runner).
use std::io::Read;

use anyhow::Result;
use clio::Input;
use log::debug;
use mozak_runner::elf::Program;
use mozak_sdk::common::types::SystemTape;

pub fn load_program(mut elf: Input) -> Result<Program> {
    let mut elf_bytes = Vec::new();
    let bytes_read = elf.read_to_end(&mut elf_bytes)?;
    debug!("Read {bytes_read} of ELF data.");
    Program::mozak_load_program(&elf_bytes)
}

/// Deserializes a serde JSON serialized system tape binary file into a
/// [`SystemTape`].
///
/// # Errors
///
/// Errors if reading from the binary file fails.
pub fn deserialize_system_tape(mut bin: Input) -> Result<SystemTape> {
    let mut sys_tapes_bytes = Vec::new();
    let bytes_read = bin.read_to_end(&mut sys_tapes_bytes)?;
    debug!("Read {bytes_read} of system tape data.");
    let deserialized: SystemTape = serde_json::from_slice(&sys_tapes_bytes)?;
    Ok(deserialized)
}
