#![cfg_attr(target_os = "zkvm", no_main)]
#![cfg_attr(feature = "std", feature(restricted_std))]

use std::io::{stdin, BufReader, Read};

use guest::stdin::{MozakIoPrivate, MozakIoPublic};

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
    let mut mozak_io_private = MozakIoPrivate {
        stdin: Box::new(BufReader::new(stdin())),
    };
    // read from private iotape, the input
    let mut buffer = [0_u8; 4];
    let n = mozak_io_private.read(buffer.as_mut()).expect("READ failed");
    assert!(n <= 4);
    let input = u32::from_le_bytes(buffer);

    // read from public iotape, the output
    let mut mozak_io_public = MozakIoPublic {
        stdin: Box::new(BufReader::new(stdin())),
    };
    let mut buffer = [0_u8; 4];
    let n = mozak_io_public.read(buffer.as_mut()).expect("READ failed");
    assert!(n <= 4);
    let out = u32::from_le_bytes(buffer);

    let ans = fibonacci(input);
    assert!(ans == out);
}

guest::entry!(main);
