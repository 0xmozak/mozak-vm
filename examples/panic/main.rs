#![cfg_attr(target_os = "mozakvm", no_main)]
#![cfg_attr(not(feature = "std"), no_std)]

// mod handlers {
//     use core::panic::PanicInfo;

//     // use crate::core::ecall;

//     #[panic_handler]
//     fn panic_fault(_panic_info: &PanicInfo) -> ! {
//         unsafe {
//             core::arch::asm!("unimp", options(noreturn, nomem, nostack),);
//         }
//         // let msg = rust_alloc::format!("{panic_info}");
//         // ecall::panic(msg.as_ptr(), msg.len());

//     }
// }

pub fn main() {
    // unsafe {
    //     core::arch::asm!("unimp", options(noreturn, nomem, nostack),);
    // }
    // use std::panic;
    // panic::set_hook(Box::new(|_| unsafe {
    //     core::arch::asm!("unimp", options(noreturn, nomem, nostack));
    // }));
    panic!();
    // panic!("Mozak VM panics 😱");
}

mozak_sdk::entry!(main);
