#![no_std]
#![no_main]
#![feature(lang_items)]

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
        exit(R_STATIC_B as u64);
    }
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
fn panic_handler(_: &core::panic::PanicInfo) -> ! { loop {} }

// TODO: we can remove this once https://github.com/rust-lang/rust/issues/85736 is fixed.
#[no_mangle]
pub fn __atomic_load_4(arg: *const usize, _ordering: usize) -> usize { unsafe { *arg } }
