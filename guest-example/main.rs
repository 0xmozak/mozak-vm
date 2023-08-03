#![no_std]
#![no_main]
#![feature(lang_items)]

use core::arch::asm;
use core::assert;
use core::assert_eq;

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
    let res = fibonacci(8);
    assert!(res == 13);
    assert_eq!(res, 13);
    exit(res);
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

// TODO: we can remove this once https://github.com/rust-lang/rust/issues/85736 is fixed.
#[no_mangle]
pub fn __atomic_load_4(arg: *const usize, _ordering: usize) -> usize {
    unsafe { *arg }
}
