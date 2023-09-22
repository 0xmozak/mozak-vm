use std::io;
use std::io::Read;

pub struct MozakIo<'a> {
    pub stdin: Box<dyn Read + 'a>,
}

impl<'a> Read for MozakIo<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(target_os = "zkvm")]
        unsafe {
            let mut len: usize;
            core::arch::asm!(
               "ecall",
               inout ("a0") 2_usize => len,
               in ("a1") &buf,
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
                .open("iotape.txt")
                .expect("cannot open tape");
            io_tape.write(buf).expect("write failed");
            Ok(n_bytes)
        }
    }
}
