#![no_std]
#![no_main]
#![feature(lang_items)]

use core::arch::asm;

#[no_mangle]
pub fn _start() -> ! {
    let a = 10;
    let b = a * 10;
    exit(0)
}

/// Exit syscall
pub fn exit(_code: i8) -> ! {
    unsafe {
        // a0 is _code
        asm!("li a7, 93");
        asm!("ecall");
    }
    loop {}
}

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
