use super::helpers::owned_buffer;
use super::linker_symbols::{_mozak_private_io_tape, _mozak_public_io_tape};
use crate::mozakvm::helpers::get_owned_buffer;

#[derive(Default)]
pub struct RandomAccessPreinitMemTape {
    pub tape: Box<[u8]>,
    pub read_offset: usize,
}

/// Implementing `std::io::Read` allows seekability later as
/// the original buffer remains owned by the Tape and only
/// copies of relevant data asked is returned back to the caller.
/// This suffers from spent cpu cycles in `memcpy`.
#[cfg(feature = "readtrait")]
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

#[cfg(feature = "readtrait")]
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

/// Not implementing `std::io::Read` allows for consumption of
/// data slices from the Tape, albeit linearly. This still leaves
/// room for seekability, but any seek is only allowed on currently
/// owned data elements (a.k.a. ahead from current `read_offset`).
/// When that happens, slice uptil that point will be thrown away.
#[cfg(not(feature = "readtrait"))]
impl RandomAccessPreinitMemTape {
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

#[derive(Default)]
pub struct RandomAccessEcallTape {
    pub ecall_id: u32,
    pub read_offset: usize,
}

#[cfg(feature = "rawio")]
type FreeformTape = RandomAccessPreinitMemTape;
#[cfg(not(feature = "rawio"))]
type FreeformTape = RandomAccessEcallTape;

pub struct PrivateInputTape(FreeformTape);
pub struct PublicInputTape(FreeformTape);

impl Default for PrivateInputTape {
    fn default() -> Self {
        #[cfg(feature = "rawio")]
        {
            Self(FreeformTape {
                tape: get_owned_buffer!(_mozak_private_io_tape),
                read_offset: 0,
            })
        }
        #[cfg(not(feature = "rawio"))]
        {
            unimplemented!()
        }
    }
}

impl Default for PublicInputTape {
    fn default() -> Self {
        #[cfg(feature = "rawio")]
        {
            Self(FreeformTape {
                tape: get_owned_buffer!(_mozak_public_io_tape),
                read_offset: 0,
            })
        }
        #[cfg(not(feature = "rawio"))]
        {
            unimplemented!()
        }
    }
}

#[cfg(feature = "readtrait")]
impl std::io::Read for PrivateInputTape {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.0.read(buf) }
}

#[cfg(feature = "readtrait")]
impl std::io::Read for PublicInputTape {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> { self.0.read(buf) }
}

// use std::io::{self, stdin, BufReader, Read};

// use rkyv::ser::serializers::AllocSerializer;
// use rkyv::{Archive, Deserialize, Serialize};

// use crate::core::{ioread_private, ioread_public};

// pub trait Extractor {
//     /// Extract one byte from the tape
//     fn get_u8(&mut self) -> u8;

//     /// Extract multiple bytes from the tape
//     fn get_buf(&mut self, buf: &mut [u8], count: usize);
// }

// /// `MozakPublicInput` is the "public" (visible to both the zk prover and the
// /// verifier) input tape. It intends to be as close to `std`'s implementation
// /// as possible to provide similar interface for end user.
// pub struct MozakPublicInput<'a> {
//     pub stdin: Box<dyn Read + 'a>,
// }

// impl<'a> Read for MozakPublicInput<'a> {
//     /// Relies on internal `ecall` (ecall #2) to the host `mozak-vm`
//     /// This may be different in behavior to what may be expected in `native`
//     /// since the host VM can return the `read_amount` as a value in two
//     /// different ways:
//     /// 1. Return the `read_amount` in register "a0" (the tenth register)
// using     /// ```
//     /// let mut len: usize = 0;
//     /// core::arch::asm!(
//     ///     "ecall",
//     ///     inout ("a0") 2_usize => len,
//     ///     in ("a1") buf.as_ptr(),
//     ///     in ("a2") buf.len(),
//     /// );
//     /// Ok(len)
//     /// ```
//     /// 2. Return the `read_amount` as the first four bytes in `buf`, leading
// to     ///    actual data to be encapsulated in `buf[4..buf.len()]`
//     ///
//     /// Due to internal concerns, one should not rely on returned
// `read_amount`     /// to base program judgements. Instead, the user should
// assume, the     /// expected bytes to be read will always succeed and would
// never lead     /// to partial read i.e. `buf` will always return filled to
// the end.     ///
//     /// Return of value here is merely for closer future compatibility with
//     /// `std`.
//     ///
//     /// TODO: reuse impl from `mozak-vm/guest`
//     fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
//         ioread_public(buf.as_mut_ptr(), buf.len());
//         Ok(buf.len())
//     }
// }

// impl<'a> Extractor for MozakPublicInput<'a> {
//     fn get_buf(&mut self, buf: &mut [u8], count: usize) {
//         let bytes_read = self.read(buf[0..count].as_mut()).expect(
//             "READ
// failed",
//         );
//         assert!(bytes_read == 1);
//     }

//     fn get_u8(&mut self) -> u8 {
//         let mut buf = [0_u8; 1];
//         let bytes_read = self.read(buf.as_mut()).expect("READ failed");
//         assert!(bytes_read == 1);
//         buf[0]
//     }
// }

// impl MozakPublicInput<'_> {
//     pub fn get_function_id(&mut self) -> u8 { self.get_u8() }
// }

// /// `MozakPrivateInput` is the "private" (visible to only the zk prover and
// not /// the verifier) input-output tape. It intends to be as close to `std`'s
// /// implementation as possible to provide similar interface for end user.
// pub struct MozakPrivateInput<'a> {
//     /// Relies on internal `ecall` (ecall #3) to the host `mozak-vm`
//     /// This may be different in behavior to what may be expected in `native`
//     /// since the host VM can return the `read_amount` as a value in two
//     /// different ways:
//     /// 1. Return the `read_amount` in register "a0" (the tenth register)
// using     /// ```
//     /// let mut len: usize = 0;
//     /// core::arch::asm!(
//     ///     "ecall",
//     ///     inout ("a0") 2_usize => len,
//     ///     in ("a1") buf.as_ptr(),
//     ///     in ("a2") buf.len(),
//     /// );
//     /// Ok(len)
//     /// ```
//     /// 2. Return the `read_amount` as the first four bytes in `buf`, leading
// to     ///    actual data to be encapsulated in `buf[4..buf.len()]`
//     ///
//     /// Due to internal concerns, one should not rely on returned
// `read_amount`     /// to base program judgements. Instead, the user should
// assume, the     /// expected bytes to be read will always succeed and would
// never lead     /// to partial read i.e. `buf` will always return filled to
// the end.     ///
//     /// Return of value here is merely for closer future compatibility with
//     /// `std`.
//     ///
//     /// TODO: reuse impl from `mozak-vm/guest`
//     pub stdin: Box<dyn Read + 'a>,
// }

// impl<'a> Read for MozakPrivateInput<'a> {
//     fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
//         syscall_ioread_private(buf.as_mut_ptr(), buf.len());
//         Ok(buf.len())
//     }
// }

// impl<'a> Extractor for MozakPrivateInput<'a> {
//     fn get_buf(&mut self, buf: &mut [u8], count: usize) {
//         let bytes_read = self.read(buf[0..count].as_mut()).expect(
//             "READ
// failed",
//         );
//         assert!(bytes_read == 1);
//     }

//     fn get_u8(&mut self) -> u8 {
//         let mut buf = [0_u8; 1];
//         let bytes_read = self.read(buf.as_mut()).expect("READ failed");
//         assert!(bytes_read == 1);
//         buf[0]
//     }
// }

// /// Provides access to the global input tapes, both public and private to
// /// the guest program. Use this as an entry point to access inputs
// /// # Examples
// /// ```rust
// /// let (mut public_tape, mut private_tape) = get_tapes();
// /// ```
// #[must_use]
// pub fn get_tapes<'a>() -> (MozakPublicInput<'a>, MozakPrivateInput<'a>) {
//     (
//         MozakPublicInput {
//             stdin: Box::new(BufReader::new(stdin())),
//         },
//         MozakPrivateInput {
//             stdin: Box::new(BufReader::new(stdin())),
//         },
//     )
// }

// /// Native API that provides open files for `IOTape` access in either
// /// read-only or write-only mode. Infallible, may panic internally
// #[must_use]
// #[allow(clippy::match_bool)]
// #[allow(clippy::missing_panics_doc)]
// pub fn get_tapes_native(is_read: bool, files: [&str; 2]) ->
// Vec<std::fs::File> {     let mut new_oo = std::fs::OpenOptions::new();

//     let open_options = match is_read {
//         true => new_oo.read(true).write(false),
//         false => new_oo.append(true).create(true),
//     };
//     files
//         .iter()
//         .map(|x| open_options.open(x).expect("cannot open tape"))
//         .collect()
// }

// /// Writes to a tape a single `u8` value signifying
// /// function ID selector. Infallible, may panic internally
// #[allow(clippy::missing_panics_doc)]
// pub fn to_tape_function_id<T>(public_tape: &mut T, id: u8)
// where
//     T: std::io::Write, {
//     public_tape
//         .write_all(&[id])
//         .expect("failure while writing function ID");
// }

// /// Reads from a tape a single `u8` value signifying
// /// function ID selector. Infallible, may panic internally
// #[allow(clippy::missing_panics_doc)]
// pub fn from_tape_function_id<T>(public_tape: &mut T) -> u8
// where
//     T: std::io::Read, {
//     let mut function_id_buffer = [0u8; 1];
//     public_tape
//         .read_exact(&mut function_id_buffer)
//         .expect("failure while reading function ID");
//     function_id_buffer[0]
// }

// /// Writes to a tape a non-length prefixed raw buffer.
// /// Infallible, may panic internally
// #[allow(clippy::missing_panics_doc)]
// pub fn to_tape_rawbuf<T>(tape: &mut T, buf: &[u8])
// where
//     T: std::io::Write, {
//     tape.write_all(buf)
//         .expect("failure while writing raw buffer");
// }

// /// Reads from a tape a non-length prefixed raw buffer.
// /// Infallible, may panic internally
// #[allow(clippy::missing_panics_doc)]
// pub fn from_tape_rawbuf<T, const N: usize>(tape: &mut T) -> [u8; N]
// where
//     T: std::io::Read, {
//     let mut buf = [0u8; N];
//     tape.read_exact(&mut buf)
//         .expect("failure while reading raw buffer");
//     buf
// }

// /// Serializes and writes to tape an `rkyv` serializable
// /// data structure with 32-bit LE length prefix.
// /// Infallible, may panic internally
// #[allow(clippy::missing_panics_doc)]
// pub fn to_tape_serialized<F, T, const N: usize>(tape: &mut F, object: &T)
// where
//     F: std::io::Write,
//     T: Serialize<AllocSerializer<N>>, {
//     let serialized_obj = rkyv::to_bytes::<_, N>(object).unwrap();
//     let serialized_obj_len =
// u32::try_from(serialized_obj.len()).unwrap().to_le_bytes();
//     tape.write_all(&serialized_obj_len)
//         .expect("failure while writing serialized obj len prefix");
//     tape.write_all(&serialized_obj)
//         .expect("failure while writing serialized obj");
// }

// /// Reads and deserializes from tape an `rkyv` deserializable
// /// data structure with 32-bit LE length prefix.
// /// Infallible, may panic internally
// #[allow(clippy::missing_panics_doc)]
// pub fn from_tape_deserialized<F, T, const N: usize>(tape: &mut F) -> T
// where
//     F: std::io::Read,
//     T: Archive,
//     T::Archived: Deserialize<T, rkyv::Infallible>, {
//     let mut length_prefix = [0u8; 4];
//     tape.read_exact(&mut length_prefix)
//         .expect("read failed for length prefix");
//     let length_prefix = u32::from_le_bytes(length_prefix);

//     let mut obj_buf = vec![0; length_prefix as usize];

//     tape.read_exact(&mut obj_buf[0..(length_prefix as usize)])
//         .expect("read failed for obj");

//     let archived = unsafe { rkyv::archived_root::<T>(&obj_buf) };
//     archived.deserialize(&mut rkyv::Infallible).unwrap()
// }
