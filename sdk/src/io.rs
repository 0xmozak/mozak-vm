use std::io::{self, stdin, BufReader, Read};

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
        #[cfg(target_os = "zkvm")]
        unsafe {
            core::arch::asm!(
               "ecall",
               in ("a0") 2_usize,
               in ("a1") buf.as_ptr(),
               in ("a2") buf.len(),
            );
            Ok(buf.len())
        }
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
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        // TODO: implement
        Ok(0)
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
