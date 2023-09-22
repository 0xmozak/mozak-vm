#![no_main]
#![no_std]

pub fn main() {
    panic!("Mozak VM panics 😱");
}

guest::entry!(main);
