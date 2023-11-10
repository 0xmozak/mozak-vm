#![no_main]
#![feature(restricted_std)]

use std::io;
use std::io::{stdin, BufReader};

pub struct MozakIo;

impl MozakIo {
    fn read_private(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(target_os = "zkvm")]
        {
            mozak_system::system::syscall_ioread_private(buf.as_mut_ptr(), buf.len());
            Ok(buf.len())
        }
    }

    fn read_public(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(target_os = "zkvm")]
        {
            mozak_system::system::syscall_ioread_public(buf.as_mut_ptr(), buf.len());
            Ok(buf.len())
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
    let mut mozak_io = MozakIo {};
    // read from private iotape, the input
    let mut buffer = [0_u8; 4];
    let n = mozak_io.read_private(buffer.as_mut()).expect("READ failed");
    // assert!(n <= 4);
    let input = u32::from_le_bytes(buffer);

    // read from public iotape, the output
    let mut buffer = [0_u8; 4];
    let n = mozak_io.read_public(buffer.as_mut()).expect("READ failed");
    // assert!(n <= 4);
    let out = u32::from_le_bytes(buffer);

    let (high, low) = fibonacci(input);
    // assert!(low == out);
    guest::env::write(&high.to_le_bytes());
}

guest::entry!(main);
