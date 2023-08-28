#![no_main]
#![feature(restricted_std)]

use core::arch::asm;
use core::assert;

const R_CONST_A: u32 = 41;
static mut R_STATIC_B: u32 = 51;

#[no_mangle]
pub fn _start() -> ! {
    unsafe {
        assert!(R_CONST_A > 41);
        assert!(R_STATIC_B > 0);
        R_STATIC_B = 56;
        exit(R_STATIC_B, 0);
    }
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

// #[panic_handler]
// fn panic_handler(_: &core::panic::PanicInfo) -> ! { loop {} }

// // TODO: we can remove this once https://github.com/rust-lang/rust/issues/85736 is fixed.
// #[no_mangle]
// pub fn __atomic_load_4(arg: *const usize, _ordering: usize) -> usize { unsafe
// { *arg } }
