#!/bin/bash
cargo run --release \
          --target ../../../buildtarget/riscv32im-mozak-mozakvm-elf.json \
          -Zbuild-std=alloc,core,compiler_builtins,std,panic_abort,proc_macro \
          -Zbuild-std-features=compiler-builtins-mem \
