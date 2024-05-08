use core::ops::{Deref, DerefMut};

use crate::core::ecall;

#[derive(Default, Clone)]
pub struct RandomAccessEcallTape {
    pub ecall_id: u32,
    pub read_offset: usize,
    /// This holds the max readable bytes from the tape
    /// TODO: Populate this via `SIZE_HINT` ecall
    pub size_hint: usize,
    /// `internal_buf` contains already read bytes
    /// via ecalls but which can be referenced again
    /// due to access to `std::io::Seek`.
    #[cfg(feature = "stdread")]
    pub internal_buf: Vec<u8>,
}

/// Implementing `std::io::Read` allows seekability later as
/// the original buffer remains owned by the Tape and only
/// copies of relevant data asked is returned back to the caller.
/// This suffers from spent cpu cycles in `memcpy`.
#[cfg(feature = "stdread")]
impl std::io::Read for RandomAccessEcallTape {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // While we want the whole buffer to be filled, it may
        // not be possible due to us reaching the end of tape
        // at times. Hence `required_bytes` encodes the desired
        // request, while `serviced_bytes` encode the actual
        // serviced len.
        let required_bytes = buf.len();
        let mut serviced_bytes = required_bytes;
        if (self.read_offset + required_bytes) > self.size_hint {
            serviced_bytes = self.size_hint - self.read_offset;
        }

        // In cases where `Seek` was used, we may be reading from
        // `internal_buf` as we may have previously "consumed" those
        // bytes already from the `ECALL` mechanism.
        let mut populatable_from_internal_buf = self.internal_buf.len() - self.read_offset;
        if serviced_bytes < populatable_from_internal_buf {
            populatable_from_internal_buf = serviced_bytes;
        }

        // These are the actual bytes we get from doing an `ECALL`
        let remaining_from_ecall = serviced_bytes - populatable_from_internal_buf;

        // Populate partial buf from `internal_buf`
        buf[..populatable_from_internal_buf].clone_from_slice(
            &self.internal_buf
                [self.read_offset..(self.read_offset + populatable_from_internal_buf)],
        );

        // Get new elements from  `ecall`
        let old_len = self.internal_buf.len();
        self.internal_buf.resize(old_len + remaining_from_ecall, 0);

        // TODO: maybe move out this function into `ecall.rs` somehow?
        unsafe {
            core::arch::asm!(
                "ecall",
                in ("a0") self.ecall_id,
                in ("a1") self.internal_buf.as_mut_ptr().add(old_len),
                in ("a2") remaining_from_ecall,
            );
        };

        // Populate partial buf from newly fetched bytes via `ECALL`
        buf[populatable_from_internal_buf..].clone_from_slice(&self.internal_buf[old_len..]);
        self.read_offset += serviced_bytes;

        Ok(serviced_bytes)
    }
}

#[cfg(feature = "stdread")]
impl std::io::Seek for RandomAccessEcallTape {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match pos {
            std::io::SeekFrom::Start(x) =>
                if x >= self.size_hint.try_into().unwrap() {
                    self.read_offset = self.size_hint - 1;
                } else {
                    self.read_offset = usize::try_from(x).unwrap();
                },
            std::io::SeekFrom::End(x) =>
                if x >= self.size_hint.try_into().unwrap() {
                    self.read_offset = 0;
                } else {
                    self.read_offset = self.size_hint - usize::try_from(x).unwrap() - 1;
                },
            std::io::SeekFrom::Current(x) => {
                if x + i64::try_from(self.read_offset).unwrap()
                    >= self.size_hint.try_into().unwrap()
                {
                    self.read_offset = self.size_hint - 1;
                } else {
                    self.read_offset += usize::try_from(x).unwrap();
                }
            }
        }
        Ok(self.read_offset as u64)
    }
}

#[derive(Clone)]
pub struct PrivateInputTape(RandomAccessEcallTape);

#[derive(Clone)]
pub struct PublicInputTape(RandomAccessEcallTape);

impl Default for PrivateInputTape {
    fn default() -> Self {
        Self(RandomAccessEcallTape {
            ecall_id: ecall::PRIVATE_TAPE,
            read_offset: 0,
            size_hint: 0,
            internal_buf: vec![],
        })
    }
}

impl PrivateInputTape {
    /// Creates a new `PrivateInputTape` with a given `size_hint`.
    pub fn with_size_hint(size_hint: usize) -> Self {
        Self(RandomAccessEcallTape {
            ecall_id: ecall::PRIVATE_TAPE,
            read_offset: 0,
            size_hint,
            internal_buf: vec![],
        })
    }
}

impl Default for PublicInputTape {
    fn default() -> Self {
        Self(RandomAccessEcallTape {
            ecall_id: ecall::PUBLIC_TAPE,
            read_offset: 0,
            size_hint: 0,
            internal_buf: vec![],
        })
    }
}

impl PublicInputTape {
    /// Creates a new `PublicInputTape` with a given `size_hint`.
    pub fn with_size_hint(size_hint: usize) -> Self {
        Self(RandomAccessEcallTape {
            ecall_id: ecall::PUBLIC_TAPE,
            read_offset: 0,
            size_hint,
            internal_buf: vec![],
        })
    }
}

impl Deref for PrivateInputTape {
    type Target = RandomAccessEcallTape;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl DerefMut for PrivateInputTape {
    fn deref_mut(&mut self) -> &mut RandomAccessEcallTape { &mut self.0 }
}

impl Deref for PublicInputTape {
    type Target = RandomAccessEcallTape;

    fn deref(&self) -> &Self::Target { &self.0 }
}

impl DerefMut for PublicInputTape {
    fn deref_mut(&mut self) -> &mut RandomAccessEcallTape { &mut self.0 }
}

impl RandomAccessEcallTape {
    pub(crate) fn len(&self) -> usize { self.size_hint }
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
