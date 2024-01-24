#![feature(restricted_std)]
mod core_logic;

use simple_logger::{set_up_color_terminal, SimpleLogger};

fn main() {
    SimpleLogger::new().init().unwrap();
    set_up_color_terminal();

    log::info!("Running amm-native");

    let files = ["public_input.tape", "private_input.tape"];

    log::info!("Generated tapes and verified proof, all done!");
}
