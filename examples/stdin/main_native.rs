use std::env;
use std::io::{stdin, BufReader, Read};

use guest::stdin::{MozakIoPrivate, MozakIoPublic};

fn main() {
    let args: Vec<String> = env::args().collect();
    // Private IO
    let mut mozak_io_private = MozakIoPrivate {
        stdin: Box::new(BufReader::new(stdin())),
        io_tape_file: args[1].clone(),
    };
    let mut buffer = [0; 1];
    let n = mozak_io_private
        .read(&mut buffer)
        .expect("Private READ failed");
    println!("The private bytes: {:?}", &buffer[..n]);
    let mut buffer = [0; 5];
    let n = mozak_io_private
        .read(&mut buffer)
        .expect("Private READ failed");
    println!("The private bytes: {:?}", &buffer[..n]);
    let mut buffer = [0; 5];
    let n = mozak_io_private
        .read(&mut buffer)
        .expect("Private READ failed");
    println!("The private bytes: {:?}", &buffer[..n]);

    // Public IO
    let mut mozak_io_public = MozakIoPublic {
        stdin: Box::new(BufReader::new(stdin())),
        io_tape_file: args[1].clone(),
    };
    let mut buffer = [0; 1];
    let n = mozak_io_public
        .read(&mut buffer)
        .expect("Public READ failed");
    println!("The public bytes: {:?}", &buffer[..n]);
    let mut buffer = [0; 5];
    let n = mozak_io_public
        .read(&mut buffer)
        .expect("Public READ failed");
    println!("The public bytes: {:?}", &buffer[..n]);
    let mut buffer = [0; 5];
    let n = mozak_io_public
        .read(&mut buffer)
        .expect("Public READ failed");
    println!("The public bytes: {:?}", &buffer[..n]);
}
