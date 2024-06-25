#[cfg(target_os = "mozakvm")]
mod alloc;
#[cfg(target_os = "mozakvm")]
pub mod debug_macros;
pub mod ecall;
pub mod env;
pub mod reg_abi;

pub mod constants {
    /// The size of a `Poseidon2Hash` digest in bytes.
    pub const DIGEST_BYTES: usize = 32;

    /// `RATE` of `Poseidon2Permutation` we use
    #[allow(dead_code)]
    pub const RATE: usize = 8;
}

#[cfg(feature = "std")]
pub fn always_abort() {
    std::panic::always_abort();
}

#[cfg(feature = "std")]
#[macro_export]
macro_rules! entry {
    ($path:path) => {
        // Type check the given path
        #[cfg(target_os = "mozakvm")]
        const MOZAK_ENTRY: fn() = $path;

        #[cfg(target_os = "mozakvm")]
        mod mozak_generated_main {
            #[no_mangle]
            fn bespoke_entrypoint() {
                $crate::core::always_abort();
                super::MOZAK_ENTRY();
                {
                    mozak_sdk::common::system::ensure_clean_shutdown();
                }
            }
        }
    };
}

#[cfg(not(feature = "std"))]
#[macro_export]
macro_rules! entry {
    ($path:path) => {
        // Type check the given path
        #[cfg(target_os = "mozakvm")]
        const MOZAK_ENTRY: fn() = $path;

        #[cfg(target_os = "mozakvm")]
        mod mozak_generated_main {
            #[no_mangle]
            fn bespoke_entrypoint() {
                super::MOZAK_ENTRY(); }
        }
    };
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
        ecall::panic(msg.as_str());
        unreachable!();
    }
}
