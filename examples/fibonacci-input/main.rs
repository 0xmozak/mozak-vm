#![no_main]
#![feature(restricted_std)]

use std::io;
use std::io::{stdin, BufReader, Read};

pub struct MozakIo<'a> {
    pub stdin: Box<dyn Read + 'a>,
    #[cfg(not(target_os = "zkvm"))]
    pub io_tape_file: String,
}

impl<'a> Read for MozakIo<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(target_os = "zkvm")]
        {
            mozak_system::system::syscall_ioread(buf.as_mut_ptr(), buf.len());
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

fn fibonacci(n: u32) -> (u32, u32) {
    if n < 2 {
        return (0, n);
    }
    let (mut curr, mut last) = (1_u64, 0_u64);
    for _i in 0..(n - 2) {
        (curr, last) = (curr + last, curr);
    }
    ((curr >> 32) as u32, curr as u32)
}

pub fn main() {
    let mut mozak_io = MozakIo {
        stdin: Box::new(BufReader::new(stdin())),
    };
    let mut buffer = [0_u8; 1];
    let n = mozak_io.read(buffer.as_mut()).expect("READ failed");
    assert!(n == 1);
    let input = u32::from(buffer[0] as char);
    let (high, _low) = fibonacci(input);
    guest::env::write(&high.to_le_bytes());
}

guest::entry!(main);
