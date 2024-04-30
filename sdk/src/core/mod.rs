#[cfg(target_os = "mozakvm")]
mod alloc;
pub mod ecall;
pub mod env;
pub mod reg_abi;

#[macro_export]
#[cfg(target_os = "mozakvm")]
macro_rules! entry {
    ($path:path) => {
        #[no_mangle]
        fn bespoke_entrypoint() {
            $path();
            $crate::core::maybe_clean_shutdown();
        }
    };
}

/// Runs clean shutdown logic only if `std`
/// feature enabled
#[cfg(target_os = "mozakvm")]
pub fn maybe_clean_shutdown() {
    #[cfg(feature = "std")]
    crate::common::system::ensure_clean_shutdown();
}

#[cfg(target_os = "mozakvm")]
#[no_mangle]
#[allow(clippy::semicolon_if_nothing_returned)]
unsafe extern "C" fn __start() {
    env::init();
    {
        extern "C" {
            fn bespoke_entrypoint();
        }
        bespoke_entrypoint()
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
#[cfg(target_os = "mozakvm")]
static STACK_TOP: u32 = 0xFFFF_0000;

// Entry point; sets up stack pointer and passes to __start.
#[cfg(target_os = "mozakvm")]
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

#[cfg(all(not(feature = "std"), target_os = "mozakvm"))]
mod handlers {
    use core::panic::PanicInfo;

    use crate::core::ecall;

    #[panic_handler]
    fn panic_fault(panic_info: &PanicInfo) -> ! {
        let msg = rust_alloc::format!("{panic_info}");
        ecall::panic(msg.as_ptr(), msg.len());
        unreachable!();
    }
}
