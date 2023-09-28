use std::io;
use std::io::Read;

pub struct MozakIo<'a> {
    pub stdin: Box<dyn Read + 'a>,
    #[cfg(not(target_os = "zkvm"))]
    pub io_tape_file: String,
}

impl<'a> Read for MozakIo<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(target_os = "zkvm")]
        unsafe {
            let mut len: usize;
            core::arch::asm!(
            "ecall",
            inout ("a0") 2_usize => len,
            in ("a1") buf.as_ptr(),
            in ("a2") buf.len(),
            );
            Ok(len)
        }
        #[cfg(not(target_os = "zkvm"))]
        {
            let n_bytes = self.stdin.read(buf).expect("read should not fail");
            // open I/O log file in append mode.
            use std::io::Write;
            let mut io_tape = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(self.io_tape_file.as_str())
                .expect("cannot open tape");
            io_tape.write(buf).expect("write failed");
            Ok(n_bytes)
        }
    }
}

impl MozakIo<'_> {
    /// Function that reads up all the bytes from the input stream.
    /// The maximum number of bytes that can be read is 2^32 - 1.
    /// The function first reads the first 4 bytes to determine the number of
    /// bytes to read. Then it reads the remaining bytes.
    /// The function returns the bytes read.
    pub(crate) fn read_all(&mut self) -> io::Result<Vec<u8>> {
        // Read the first 4 bytes to determine the number of bytes to read.
        let len: usize;
        let mut buf = [0_u8; 4];
        self.read(&mut buf)?;

        // Convert the first 4 bytes to usize.
        // We do not use `from_le_bytes` because in some native environments
        // the usize is 8 bytes.
        len = usize::try_from(u32::from_be_bytes(buf)).expect("Could not convert to usize");

        // Read the remaining bytes.
        let mut buf = vec![0_u8; len];
        self.read(&mut buf)?;

        Ok(buf)
    }

    #[cfg(target_os = "zkvm")]
    pub(crate) fn new<'a>() -> MozakIo<'a> {
        MozakIo {
            stdin: Box::new(io::stdin()),
        }
    }

    #[cfg(not(target_os = "zkvm"))]
    pub(crate) fn new<'a>(io_tape_file: String) -> MozakIo<'a> {
        MozakIo {
            stdin: Box::new(io::stdin()),
            io_tape_file,
        }
    }
}

impl Default for MozakIo<'_> {
    #[cfg(target_os = "zkvm")]
    fn default() -> Self { Self::new() }

    #[cfg(not(target_os = "zkvm"))]
    fn default() -> Self {
        const DEFAULT_IO_TAPE_FILE: &str = "iotape";

        Self::new(DEFAULT_IO_TAPE_FILE.to_string())
    }
}
