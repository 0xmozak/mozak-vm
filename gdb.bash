#!/bin/bash
riscv64-elf-gdb examples/target/riscv32im-mozak-mozakvm-elf/debug/empty

# then need to run the following commands in gdb
# # target remote :1234
# 1234 seems to be the default port.  Not sure how to set it.
