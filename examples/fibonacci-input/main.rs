#![no_main]
#![feature(restricted_std)]

use std::io;
use std::io::Read;
use std::io::{stdin, BufReader};

pub struct MozakIo<'a> {
    pub stdin: Box<dyn Read + 'a>,
    #[cfg(not(target_os = "zkvm"))]
    pub io_tape_file: String,
}

use core::{assert, assert_eq};

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


pub fn main(){
    let mut mozak_io = MozakIo {
        stdin: Box::new(BufReader::new(stdin())),
    };
    // allow only numbers with atmost 5 digits
    let mut buffer = [0_u8; 3];
    let bytes_read = mozak_io.read(buffer.as_mut()).expect("READ failed");
    println!("{:?}", buffer);
    let n: u32 = std::str::from_utf8(&buffer[..bytes_read-1]).unwrap().to_string().trim().parse().unwrap();
    println!("{}", n);
    let (high, low) = fibonacci(2);
    assert!(low == 63245986);
    assert_eq!(high, 0);
    guest::env::write(&high.to_le_bytes());
}

guest::entry!(main);
