BSS-Tester tests for availability of `bss` section in final ELF when compiled with linker script

# To run

To build for Mozak-VM:

```sh
# inside examples directory
# [overseer/0-0]
cargo build --release --bin bss-tester
```

After this, test whether we have a `bss` or `sbss` sections using:
```sh
# inside examples directory
# TODO: add overseer, fails on x86_64 due to
# ./.tools/riscv64-unknown-elf-objdump_x86_64: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.33' not found (required by ./.tools/riscv64-unknown-elf-objdump_x86_64)
# ./.tools/riscv64-unknown-elf-objdump_x86_64: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.34' not found (required by ./.tools/riscv64-unknown-elf-objdump_x86_64)
set -e
./.tools/riscv64-unknown-elf-objdump_$(uname -m) -h target/riscv32im-mozak-mozakvm-elf/release/bss-tester | grep bss 
```
