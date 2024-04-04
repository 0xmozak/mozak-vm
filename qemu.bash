#!/bin/bash
set -euxo pipefail

# qemu-system-riscv32 -nographic -machine virt  -smp 1 -m 2G -kernel riscv-testdata/testdata/rv32ui-p-add
# nice qemu-system-riscv32 -bios none -nographic -machine virt  -smp 1 -m 2G -kernel riscv-testdata/testdata/rv32ui-p-add
# -S -s is to wait for debugger to connect.
nice qemu-system-riscv32 -S -s -bios none -nographic -machine virt  -smp 1 -m 256M -kernel examples/target/riscv32im-mozak-mozakvm-elf/debug/empty
