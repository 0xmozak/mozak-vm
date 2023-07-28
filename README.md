Example cargo project wich can generate ELF compatible with MozakVM.

How to build:
`cargo +nightly build`

To dump assembly files:
`RUSTFLAGS="--emit asm" cargo +nightly build`
 
 Now check in ./target/riscv32im-risc0-zkvm-elf/debug/deps for assembly files (.s extension)
