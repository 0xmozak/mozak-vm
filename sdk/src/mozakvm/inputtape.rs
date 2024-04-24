use super::helpers::owned_buffer;
use super::linker_symbols::{_mozak_private_io_tape, _mozak_public_io_tape};
use crate::core::ecall;
use crate::mozakvm::helpers::get_owned_buffer;

#[derive(Default, Clone)]
pub struct RandomAccessTape {
    pub ecall_id: u32,
    pub read_offset: usize,
}

type FreeformTape = RandomAccessTape;

#[derive(Clone)]
pub struct PrivateInputTape(FreeformTape);

#[derive(Clone)]
pub struct PublicInputTape(FreeformTape);

impl Default for PrivateInputTape {
    fn default() -> Self {
        {
            Self(FreeformTape {
                ecall_id: ecall::IO_READ_PRIVATE,
                read_offset: 0,
            })
        }
    }
}

impl Default for PublicInputTape {
    fn default() -> Self {
        {
            Self(FreeformTape {
                ecall_id: ecall::IO_READ_PUBLIC,
                read_offset: 0,
            })
        }
    }
}

#[cfg(feature = "stdread")]
impl std::io::Read for PrivateInputTape {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.0.read(buf) }
}

#[cfg(feature = "stdread")]
impl std::io::Read for PublicInputTape {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.0.read(buf) }
}

impl PrivateInputTape {
    pub fn len(&self) -> usize { self.0.len() }

    pub fn read_ptr(&self) -> usize { self.0.read_ptr() }
}

impl PublicInputTape {
    pub fn len(&self) -> usize { self.0.len() }

    pub fn read_ptr(&self) -> usize { self.0.read_ptr() }
}

/// Provides the length of tape available to read
#[cfg(all(feature = "std", target_os = "mozakvm"))]
#[must_use]
pub fn input_tape_len(kind: &crate::InputTapeType) -> usize {
    match kind {
        crate::InputTapeType::PublicTape => unsafe {
            crate::common::system::SYSTEM_TAPE.public_input_tape.len()
        },
        crate::InputTapeType::PrivateTape => unsafe {
            crate::common::system::SYSTEM_TAPE.private_input_tape.len()
        },
    }
}

/// Reads utmost given number of raw bytes from an input tape
#[allow(clippy::missing_errors_doc)]
#[cfg(all(feature = "std", feature = "stdread", target_os = "mozakvm"))]
pub fn read(kind: &crate::InputTapeType, buf: &mut [u8]) -> std::io::Result<usize> {
    use std::io::Read;
    match kind {
        crate::InputTapeType::PublicTape => unsafe {
            crate::common::system::SYSTEM_TAPE
                .public_input_tape
                .read(buf)
        },
        crate::InputTapeType::PrivateTape => unsafe {
            crate::common::system::SYSTEM_TAPE
                .private_input_tape
                .read(buf)
        },
    }
}
