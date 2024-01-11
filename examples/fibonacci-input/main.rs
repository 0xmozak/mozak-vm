#![cfg_attr(target_os = "zkvm", no_main)]
#![feature(restricted_std)]

#[cfg(not(target_os = "zkvm"))]
use std::env;
use std::io::{stdin, BufReader, Read};

use guest::stdin::{MozakIo, MozakIoPrivate, MozakIoPublic};

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
    #[cfg(not(target_os = "zkvm"))]
    let args: Vec<String> = env::args().collect();
    let mut mozak_io_private = MozakIoPrivate(MozakIo {
        stdin: Box::new(BufReader::new(stdin())),
        #[cfg(not(target_os = "zkvm"))]
        io_tape_file: args[1].clone(),
    });
    // read from private iotape, the input
    let mut buffer = [0_u8; 4];
    let n = mozak_io_private.read(buffer.as_mut()).expect("READ failed");
    assert!(n <= 4);
    let input = u32::from_le_bytes(buffer);

    // read from public iotape, the output
    let mut mozak_io_public = MozakIoPublic(MozakIo {
        stdin: Box::new(BufReader::new(stdin())),
        #[cfg(not(target_os = "zkvm"))]
        io_tape_file: args[2].clone(),
    });
    let mut buffer = [0_u8; 4];
    let n = mozak_io_public.read(buffer.as_mut()).expect("READ failed");
    assert!(n <= 4);
    let out = u32::from_le_bytes(buffer);

    let ans = fibonacci(input);
    assert!(ans == out);
    guest::env::write(&out.to_le_bytes());
}

guest::entry!(main);
