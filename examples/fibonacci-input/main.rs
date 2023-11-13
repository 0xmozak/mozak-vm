#![no_main]
#![feature(restricted_std)]

use std::io;

pub struct MozakIo {}

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

fn fibonacci(n: u32) -> u32 {
    if n < 2 {
        return n;
    }
    let (mut curr, mut last) = (1_u32, 0_u32);
    for _i in 0..(n - 2) {
        (curr, last) = (curr.wrapping_add(last), curr);
    }
    curr
}

pub fn main() {
    let mut mozak_io = MozakIo {};
    // read from private iotape, the input
    let mut buffer = [0_u8; 4];
    let n = mozak_io.read_private(buffer.as_mut()).expect("READ failed");
    assert!(n <= 4);
    let input = u32::from_le_bytes(buffer);

    // read from public iotape, the output
    let mut buffer = [0_u8; 4];
    let n = mozak_io.read_public(buffer.as_mut()).expect("READ failed");
    assert!(n <= 4);
    let out = u32::from_le_bytes(buffer);

    let ans = fibonacci(input);
    assert!(ans == out);
}

guest::entry!(main);
