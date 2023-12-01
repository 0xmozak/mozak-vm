use std::io::{self, stdin, BufReader, Read};

use mozak_system::system::{syscall_ioread_private, syscall_ioread_public};
use rkyv::ser::serializers::AllocSerializer;
use rkyv::{Archive, Deserialize, Serialize};

pub trait Extractor {
    /// Extract one byte from the tape
    fn get_u8(&mut self) -> u8;

    /// Extract multiple bytes from the tape
    fn get_buf(&mut self, buf: &mut [u8], count: usize);
}

/// `MozakPublicInput` is the "public" (visible to both the zk prover and the
/// verifier) input tape. It intends to be as close to `std`'s implementation
/// as possible to provide similar interface for end user.
pub struct MozakPublicInput<'a> {
    pub stdin: Box<dyn Read + 'a>,
}

impl<'a> Read for MozakPublicInput<'a> {
    /// Relies on internal `ecall` (ecall #2) to the host `mozak-vm`
    /// This may be different in behavior to what may be expected in `native`
    /// since the host VM can return the `read_amount` as a value in two
    /// different ways:
    /// 1. Return the `read_amount` in register "a0" (the tenth register) using
    /// ```
    /// let mut len: usize = 0;
    /// core::arch::asm!(
    ///     "ecall",
    ///     inout ("a0") 2_usize => len,
    ///     in ("a1") buf.as_ptr(),
    ///     in ("a2") buf.len(),
    /// );
    /// Ok(len)
    /// ```
    /// 2. Return the `read_amount` as the first four bytes in `buf`, leading to
    ///    actual data to be encapsulated in `buf[4..buf.len()]`
    ///
    /// Due to internal concerns, one should not rely on returned `read_amount`
    /// to base program judgements. Instead, the user should assume, the
    /// expected bytes to be read will always succeed and would never lead
    /// to partial read i.e. `buf` will always return filled to the end.
    ///
    /// Return of value here is merely for closer future compatibility with
    /// `std`.
    ///
    /// TODO: reuse impl from `mozak-vm/guest`
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        syscall_ioread_public(buf.as_mut_ptr(), buf.len());
        Ok(buf.len())
    }
}

impl<'a> Extractor for MozakPublicInput<'a> {
    fn get_buf(&mut self, buf: &mut [u8], count: usize) {
        let bytes_read = self.read(buf[0..count].as_mut()).expect("READ failed");
        assert!(bytes_read == 1);
    }

    fn get_u8(&mut self) -> u8 {
        let mut buf = [0_u8; 1];
        let bytes_read = self.read(buf.as_mut()).expect("READ failed");
        assert!(bytes_read == 1);
        buf[0]
    }
}

impl MozakPublicInput<'_> {
    pub fn get_function_id(&mut self) -> u8 { self.get_u8() }
}

/// `MozakPrivateInput` is the "private" (visible to only the zk prover and not
/// the verifier) input-output tape. It intends to be as close to `std`'s
/// implementation as possible to provide similar interface for end user.
pub struct MozakPrivateInput<'a> {
    /// Relies on internal `ecall` (ecall #3) to the host `mozak-vm`
    /// This may be different in behavior to what may be expected in `native`
    /// since the host VM can return the `read_amount` as a value in two
    /// different ways:
    /// 1. Return the `read_amount` in register "a0" (the tenth register) using
    /// ```
    /// let mut len: usize = 0;
    /// core::arch::asm!(
    ///     "ecall",
    ///     inout ("a0") 2_usize => len,
    ///     in ("a1") buf.as_ptr(),
    ///     in ("a2") buf.len(),
    /// );
    /// Ok(len)
    /// ```
    /// 2. Return the `read_amount` as the first four bytes in `buf`, leading to
    ///    actual data to be encapsulated in `buf[4..buf.len()]`
    ///
    /// Due to internal concerns, one should not rely on returned `read_amount`
    /// to base program judgements. Instead, the user should assume, the
    /// expected bytes to be read will always succeed and would never lead
    /// to partial read i.e. `buf` will always return filled to the end.
    ///
    /// Return of value here is merely for closer future compatibility with
    /// `std`.
    ///
    /// TODO: reuse impl from `mozak-vm/guest`
    pub stdin: Box<dyn Read + 'a>,
}

impl<'a> Read for MozakPrivateInput<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        syscall_ioread_private(buf.as_mut_ptr(), buf.len());
        Ok(buf.len())
    }
}

impl<'a> Extractor for MozakPrivateInput<'a> {
    fn get_buf(&mut self, buf: &mut [u8], count: usize) {
        let bytes_read = self.read(buf[0..count].as_mut()).expect("READ failed");
        assert!(bytes_read == 1);
    }

    fn get_u8(&mut self) -> u8 {
        let mut buf = [0_u8; 1];
        let bytes_read = self.read(buf.as_mut()).expect("READ failed");
        assert!(bytes_read == 1);
        buf[0]
    }
}

/// Provides access to the global input tapes, both public and private to
/// the guest program. Use this as an entry point to access inputs
/// # Examples
/// ```rust
/// let (mut public_tape, mut private_tape) = get_tapes();
/// ```
#[must_use]
pub fn get_tapes<'a>() -> (MozakPublicInput<'a>, MozakPrivateInput<'a>) {
    (
        MozakPublicInput {
            stdin: Box::new(BufReader::new(stdin())),
        },
        MozakPrivateInput {
            stdin: Box::new(BufReader::new(stdin())),
        },
    )
}

/// Native API that provides open files for IOTape access in either
/// read-only or write-only mode.
// #[allow(dead_code)]
pub fn get_tapes_native(is_read: bool, files: [&str; 2]) -> Vec<std::fs::File> {
    let mut new_oo = std::fs::OpenOptions::new();

    let open_options = match is_read {
        true => new_oo.read(true).write(false),
        false => new_oo.append(true).create(true),
    };
    files
        .iter()
        .map(|x| open_options.open(x).expect("cannot open tape"))
        .collect()
}

/// Native API that writes to a tape a single u8 value signifying
/// function ID selector.
pub fn to_tape_function_id<T>(public_tape: &mut T, id: u8)
where
    T: std::io::Write, {
    public_tape
        .write(&[id])
        .expect("failure while writing function ID");
}

/// Native API that reads from a tape a single u8 value signifying
/// function ID selector.
pub fn from_tape_function_id<T>(public_tape: &mut T) -> u8
where
    T: std::io::Read, {
    let mut function_id_buffer = [0u8; 1];
    public_tape
        .read(&mut function_id_buffer)
        .expect("failure while reading function ID");
    function_id_buffer[0]
}

/// Native API that writes to a tape a non-length prefixed raw buffer
pub fn to_tape_rawbuf<T>(tape: &mut T, buf: &[u8])
where
    T: std::io::Write, {
    tape.write(buf).expect("failure while writing raw buffer");
}

/// Native API that reads from a tape a non-length prefixed raw buffer
pub fn from_tape_rawbuf<T, const N: usize>(tape: &mut T) -> [u8; N]
where
    T: std::io::Read, {
    let mut buf = [0u8; N];
    tape.read(&mut buf)
        .expect("failure while reading raw buffer");
    buf
}

/// Native API that serializes and writes to tape an `rkyv` serializable
/// data structure with 32-bit length prefix.
pub fn to_tape_serialized<F, T, const N: usize>(tape: &mut F, object: &T)
where
    F: std::io::Write,
    T: Serialize<AllocSerializer<N>>, {
    let serialized_obj = rkyv::to_bytes::<_, N>(object).unwrap();
    let serialized_obj_len = (serialized_obj.len() as u32).to_le_bytes();
    tape.write(&serialized_obj_len)
        .expect("failure while writing serialized obj len prefix");
    tape.write(&serialized_obj)
        .expect("failure while writing serialized obj");
}

/// Native API that reads and deserializes from tape an `rkyv` deserializable
/// data structure with 32-bit length prefix.
pub fn from_tape_deserialized<F, T, const N: usize>(tape: &mut F) -> T
where
    F: std::io::Read,
    T: Archive,
    T::Archived: Deserialize<T, rkyv::Infallible>, {
    let mut length_prefix = [0u8; 4];
    tape.read(&mut length_prefix)
        .expect("read failed for length prefix");
    let length_prefix = u32::from_le_bytes(length_prefix);

    let mut obj_buf = Vec::with_capacity(length_prefix as usize);
    obj_buf.resize(length_prefix as usize, 0);

    tape.read(&mut obj_buf[0..(length_prefix as usize)])
        .expect("read failed for obj");

    let archived = unsafe { rkyv::archived_root::<T>(&obj_buf) };
    archived.deserialize(&mut rkyv::Infallible).unwrap()
}
