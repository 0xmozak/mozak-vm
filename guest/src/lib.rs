#![no_std]

extern crate alloc as rust_alloc;

mod alloc;
pub mod env;

#[macro_export]
macro_rules! entry {
    ($path:path) => {
        // Type check the given path
        const MOZAK_ENTRY: fn() = $path;

        mod mozak_generated_main {
            #[no_mangle]
            fn main() { super::MOZAK_ENTRY() }
        }
    };
}

#[no_mangle]
unsafe extern "C" fn __start() {
    env::init();
    {
        extern "C" {
            fn main();
        }
        main()
    }
    env::finalize();
}

// The stack grows downwards (towards lower addresses) and the stack pointer
// shall be aligned to a 128-bit boundary upon procedure entry. The first
// argument passed on the stack is located at offset zero of the stack pointer
// on function entry; following arguments are stored at correspondingly higher
// addresses.
//
// For more details:
// https://github.com/riscv-non-isa/riscv-elf-psabi-doc/blob/master/riscv-cc.adoc
static STACK_TOP: u32 = 0xFFFF_FFFF;

// Entry point; sets up stack pointer and passes to __start.
core::arch::global_asm!(
r#"
.section .text._start;
.global _start;
_start:
    la sp, {0}
    lw sp, 0(sp)
    jal ra, __start;
"#,
    sym STACK_TOP
);

#[cfg(all(not(feature = "std"), target_os = "zkvm"))]
mod handlers {
    use core::arch::asm;
    use core::panic::PanicInfo;

    #[panic_handler]
    fn panic_fault(panic_info: &PanicInfo) -> ! {
        let msg = rust_alloc::format!("{}", panic_info);
        unsafe {
            asm!("ecall", in ("a0") 1, in ("a1") msg.len(), in ("a2") msg.as_ptr());
        }
        unreachable!();
    }
}
