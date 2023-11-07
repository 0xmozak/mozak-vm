use std::io;
use std::io::Read;

pub struct MozakIo<'a> {
    pub stdin: Box<dyn Read + 'a>,
    #[cfg(not(target_os = "zkvm"))]
    pub io_tape_file: String,
}

impl<'a> Read for MozakIo<'a> {
    /// TODO(Roman): how to add read-public ?
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(target_os = "zkvm")]
        {
            mozak_system::system::syscall_ioread_private(buf.as_mut_ptr(), buf.len());
            Ok(buf.len())
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
