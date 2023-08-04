#![no_std]
#![no_main]
#![feature(lang_items)]

use core::arch::asm;
use core::{assert, assert_eq};

fn fibonacci(n: u32) -> (u32, u32) {
    if n == 0 {
        return (0, 0);
    }
    if n == 1 {
        return (0, 1);
    }
    let mut sum = 0_u64;
    let mut last = 0;
    let mut curr = 1;
    for _i in 0..(n - 2) {
        sum = last + curr;
        last = curr;
        curr = sum;
    }
    ((sum >> 32) as u32, sum as u32)
}

#[no_mangle]
pub fn _start() -> ! {
    let (high, low) = fibonacci(40);
    assert!(low == 63245986);
    assert_eq!(high, 0);
    exit(low, high);
}

/// Exit syscall
///
/// As per RISC-V Calling Convention a0/a1 (which are actually X10/X11) can be
/// used as function argument/result.
#[no_mangle]
#[inline(never)]
pub fn exit(a0: u32, a1: u32) -> ! {
    unsafe {
        asm!(
            "add a0, zero, {a0}",
            "add a1, zero, {a1}",
            "li a7, 93",
            "ecall",
            a0 = in(reg) a0,
            a1 = in(reg) a1,
        );
    }
    loop {}
}

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! { loop {} }

// TODO: we can remove this once https://github.com/rust-lang/rust/issues/85736 is fixed.
#[no_mangle]
pub fn __atomic_load_4(arg: *const usize, _ordering: usize) -> usize { unsafe { *arg } }
