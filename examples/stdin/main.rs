#![cfg_attr(target_os = "zkvm", no_main)]
#![feature(restricted_std)]

#[cfg(not(target_os = "zkvm"))]
use std::env;
use std::io::{stdin, BufReader, Read};

use guest::stdin::{MozakIo, MozakIoPrivate, MozakIoPublic};

pub fn main() {
    #[cfg(not(target_os = "zkvm"))]
    let args: Vec<String> = env::args().collect();
    // Private IO
    let mut mozak_io_private = MozakIoPrivate(MozakIo {
        stdin: Box::new(BufReader::new(stdin())),
        #[cfg(not(target_os = "zkvm"))]
        io_tape_file: args[1].clone(),
    });
    let mut buffer = [0_u8; 1];
    let n = mozak_io_private
        .read(buffer.as_mut())
        .expect("Private READ failed");
    #[cfg(not(target_os = "zkvm"))]
    println!("The private bytes: {:?}", &buffer[..n]);
    assert!(n == 1);
    let mut buffer = [0_u8; 5];
    let n = mozak_io_private
        .read(buffer.as_mut())
        .expect("Private READ failed");
    #[cfg(not(target_os = "zkvm"))]
    println!("The private bytes: {:?}", &buffer[..n]);
    assert!(n == 5);
    let mut buffer = [0_u8; 5];
    let n = mozak_io_private
        .read(buffer.as_mut())
        .expect("Private READ failed");
    #[cfg(not(target_os = "zkvm"))]
    println!("The private bytes: {:?}", &buffer[..n]);
    assert!(n == 5);
    guest::env::write(&n.to_be_bytes());

    // Public IO
    let mut mozak_io_public = MozakIoPublic(MozakIo {
        stdin: Box::new(BufReader::new(stdin())),
        #[cfg(not(target_os = "zkvm"))]
        io_tape_file: args[2].clone(),
    });
    let mut buffer = [0_u8; 1];
    let n = mozak_io_public
        .read(buffer.as_mut())
        .expect("Public READ failed");
    #[cfg(not(target_os = "zkvm"))]
    println!("The public bytes: {:?}", &buffer[..n]);
    assert!(n == 1);
    let mut buffer = [0_u8; 5];
    let n = mozak_io_public
        .read(buffer.as_mut())
        .expect("Public READ failed");
    #[cfg(not(target_os = "zkvm"))]
    println!("The public bytes: {:?}", &buffer[..n]);
    assert!(n == 5);
    let mut buffer = [0_u8; 5];
    let n = mozak_io_public
        .read(buffer.as_mut())
        .expect("Public READ failed");
    #[cfg(not(target_os = "zkvm"))]
    println!("The public bytes: {:?}", &buffer[..n]);
    assert!(n == 5);
    guest::env::write(&n.to_be_bytes());
}

guest::entry!(main);
