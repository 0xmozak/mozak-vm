#![no_std]
#![no_main]
#![feature(lang_items)]

use core::arch::asm;

fn fibonacci(n: u32) -> u64 {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return 1;
    }
    let mut sum = 0;
    let mut last = 0;
    let mut curr = 1;
    for _i in 0..(n - 2) {
        sum = last + curr;
        last = curr;
        curr = sum;
    }
    sum
}

#[no_mangle]
pub fn _start() -> ! {
    exit(fibonacci(80));
}

/// Exit syscall
pub fn exit(_code: u64) -> ! {
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
