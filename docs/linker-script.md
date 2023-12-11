# Linker script
Linker script used while generating mozak-vm targetted program ELFs modifies the memory layout.
This document intends to clarify this in more detail.

## Without linker script
We take an example of `empty` within `examples`. Without a linker script, we build the system using
the command 
```sh
cargo +nightly build --release --bin empty
```

### Exploring via `objdump`
`objdump` utility (differently built for riscV) can be fetched via and is useful for exploring the
generated ELF
- MacOS: https://github.com/riscv-software-src/homebrew-riscv
- Others: https://github.com/riscv-software-src/riscv-tools
