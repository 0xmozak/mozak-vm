use std::env;
use std::io::{stdin, BufReader, Read};

use examples::io::MozakIo;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut mozak_io = MozakIo {
        stdin: Box::new(BufReader::new(stdin())),
        io_tape_file: args[1].clone(),
    };
    1 + 1;
    let mut buffer = [0_u8; 10];
    let n = mozak_io.read(buffer.as_mut()).expect("READ failed");
    buffer[0];
    println!("The bytes: {:?}", buffer);

    let new_v = buffer[0] + 1;
    println!("The bytes: {:?}", new_v);
    assert!(new_v == 50);
    // let mut buffer = [0_u8; 5];
    // let n = mozak_io.read(buffer.as_mut()).expect("READ failed");
    // assert_eq!(n, 5);
    // assert_eq!(buffer[0], 2);
}
