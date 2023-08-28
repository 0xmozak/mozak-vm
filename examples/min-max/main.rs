#![no_main]
#![feature(restricted_std)]

use core::arch::asm;
use core::assert;

#[no_mangle]
pub fn _start() -> ! {
    let min = std::cmp::min(100_u32, 1000_u32);
    let max = std::cmp::max(100_u32, 1000_u32);
    assert!(min < max);
    exit(min, max);
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
