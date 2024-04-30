use super::helpers::owned_buffer;
use super::linker_symbols::{_mozak_private_io_tape, _mozak_public_io_tape};
use crate::mozakvm::helpers::get_owned_buffer;

#[derive(Default, Clone)]
pub struct RandomAccessPreinitMemTape {
    pub tape: Box<[u8]>,
    pub read_offset: usize,
}

/// Implementing `std::io::Read` allows seekability later as
/// the original buffer remains owned by the Tape and only
/// copies of relevant data asked is returned back to the caller.
/// This suffers from spent cpu cycles in `memcpy`.
#[cfg(feature = "stdread")]
impl std::io::Read for RandomAccessPreinitMemTape {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let (mut read_bytes, remaining_buf) = (buf.len(), self.tape.len() - self.read_offset);
        // In case we don't have enough bytes to read
        if read_bytes > remaining_buf {
            read_bytes = remaining_buf;
        }

        buf[..read_bytes]
            .clone_from_slice(&self.tape[self.read_offset..(self.read_offset + read_bytes)]);

        self.read_offset += read_bytes;

        Ok(read_bytes)
    }
}

#[cfg(feature = "stdread")]
impl std::io::Seek for RandomAccessPreinitMemTape {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match pos {
            std::io::SeekFrom::Start(x) =>
                if x >= self.tape.len().try_into().unwrap() {
                    self.read_offset = self.tape.len() - 1;
                } else {
                    self.read_offset = usize::try_from(x).unwrap();
                },
            std::io::SeekFrom::End(x) =>
                if x >= self.tape.len().try_into().unwrap() {
                    self.read_offset = 0;
                } else {
                    self.read_offset = self.tape.len() - usize::try_from(x).unwrap() - 1;
                },
            std::io::SeekFrom::Current(x) => {
                if x + i64::try_from(self.read_offset).unwrap()
                    >= self.tape.len().try_into().unwrap()
                {
                    self.read_offset = self.tape.len() - 1;
                } else {
                    self.read_offset += usize::try_from(x).unwrap();
                }
            }
        }
        Ok(self.read_offset as u64)
    }
}

impl RandomAccessPreinitMemTape {
    /// Returns the "safe" readable length of the tape,
    /// reading outside which may result in accessing
    /// garbage values. `len` reduces after elements are
    /// accessed in case the tape  is "consuming" i.e.
    /// tape is accessed without feature `stdread` enabled.
    /// If `stdread` is enabled, `std::io::Seek` is
    /// implemented for the tape and length of tape never
    /// reduces.
    #[allow(dead_code)]
    pub fn len(&self) -> usize { self.tape.len() }

    /// Provides the read offset in tape which can be
    /// changed via `std::io::Seek` implementation on
    /// a tape with feature `stdread`. In case this feature
    /// is not enabled, this function always returns `0`
    /// and the tape elements when accessed are "consumed".
    #[allow(dead_code)]
    pub fn read_ptr(&self) -> usize { self.read_offset }
}

/// Not implementing `std::io::Read` allows for consumption of
/// data slices from the Tape, albeit linearly. This still leaves
/// room for seekability (in principle), but any seek is only
/// allowed on currently owned data elements
/// (a.k.a. ahead from current `read_offset` ).
/// When that happens, slice uptil that point will be thrown away.
#[cfg(not(feature = "stdread"))]
impl RandomAccessPreinitMemTape {
    /// consumes the underlying buffer upto a maximum length
    /// `max_readlen` and returns an owned slice.
    fn read(&mut self, max_readlen: usize) -> Box<[u8]> {
        let (mut read_bytes, remaining_buf) = (buf.len(), self.tape.len());
        // In case we don't have enough bytes to read
        if read_bytes > remaining_buf {
            read_bytes = remaining_buf;
        }
        self.read_offset += read_bytes;

        let read_ptr = self.tape.as_ptr();

        self.tape = unsafe {
            let mem_slice = slice_from_raw_parts::<u8>(
                read_ptr.add(read_bytes),
                (self.tape.len() - read_bytes),
            );
            Box::from_raw(mem_slice as *mut [u8])
        };
        unsafe {
            let mem_slice = slice_from_raw_parts::<u8>(read_ptr, read_bytes);
            Box::from_raw(mem_slice as *mut [u8])
        }
    }
}

#[derive(Default, Clone)]
#[allow(warnings)]
pub struct RandomAccessEcallTape {
    pub ecall_id: u32,
    pub read_offset: usize,
}

#[cfg(feature = "preinitmem_inputtape")]
type FreeformTape = RandomAccessPreinitMemTape;
#[cfg(not(feature = "preinitmem_inputtape"))]
type FreeformTape = RandomAccessEcallTape;

#[derive(Clone)]
pub struct PrivateInputTape(FreeformTape);

#[derive(Clone)]
pub struct PublicInputTape(FreeformTape);

impl Default for PrivateInputTape {
    fn default() -> Self {
        #[cfg(feature = "preinitmem_inputtape")]
        {
            Self(FreeformTape {
                tape: get_owned_buffer!(_mozak_private_io_tape),
                read_offset: 0,
            })
        }
        #[cfg(not(feature = "preinitmem_inputtape"))]
        {
            // TODO: Implement this when we want to revert back to
            // ecall based systems. Unimplemented for now.
            unimplemented!()
        }
    }
}

impl Default for PublicInputTape {
    fn default() -> Self {
        #[cfg(feature = "preinitmem_inputtape")]
        {
            Self(FreeformTape {
                tape: get_owned_buffer!(_mozak_public_io_tape),
                read_offset: 0,
            })
        }
        #[cfg(not(feature = "preinitmem_inputtape"))]
        {
            // TODO: Implement this when we want to revert back to
            // ecall based systems. Unimplemented for now.
            unimplemented!()
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
