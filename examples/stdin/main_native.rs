// TODO: Unify core_logic with this file. See https://github.com/0xmozak/mozak-vm/issues/664
mod core_logic;

use std::io::{stdin, BufReader, Read};

use crate::core_logic::MozakIo;

fn main() {
    let mut mozak_io = MozakIo {
        stdin: Box::new(BufReader::new(stdin())),
    };
    let mut buffer = [0; 1];
    let n = mozak_io.read(&mut buffer).expect("READ failed");
    println!("The bytes: {:?}", &buffer[..n]);
    let mut buffer = [0; 5];
    let n = mozak_io.read(&mut buffer).expect("READ failed");
    println!("The bytes: {:?}", &buffer[..n]);
    let mut buffer = [0; 5];
    let n = mozak_io.read(&mut buffer).expect("READ failed");
    println!("The bytes: {:?}", &buffer[..n]);
}
