#![no_main]
#![feature(restricted_std)]

mod core_logic;

use mozak_sdk::io::{get_tapes, Extractor};
use mozak_sdk::sys::mailbox_receive;

pub fn main() {
    // let (mut public_tape, mut _private_tape) = get_tapes();

    sdk::set_self_prog(prog_id);

    ///
    for message in mailbox_receive().unwrap() { // iterator
         // if (message.caller == self_prog_id) {
         //     // dispatch and check
         // }
    }

    // Vec<Element>
    // Element: Vec<Element> | Message

    // / under B
    // / | A | B | B->C | B->C->B | B | C |

    // #[allow(clippy::single_match)]
    // match public_tape.get_u8() {

    //     /// AMM  (Top level)
    //     ///  USDC (Responders)
    //     ///    WALLET  (Responders)
    //     ///
    //     0 => {
    //         // Single function execution
    //         // match public_tape.get_u8() {
    //         //     unimplemented!()
    //         // }
    //     }
    //     _ => {
    //         // Multi-function execution based on recepient
    //         // for calls in global_transcript_calls(program_id) {
    //         //     // Do those calls
    //         // }
    //     }
    // }
}

// We define `main()` to be the program's entry point.
guest::entry!(main);
