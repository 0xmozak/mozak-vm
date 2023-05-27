# Mozak Risc-V Virtual Machine

If you are unfamiliar with the Risc-V instruction set, please have a look at the [Risc-V instruction set reference](https://github.com/jameslzhu/riscv-card/blob/master/riscv-card.pdf).

# Installation

The Mozak VM is built in Rust, so the [Rust toolchain](https://www.rust-lang.org/tools/install) is a pre-requisite.

The test setup involves [Docker](https://www.docker.com/) so Docker installation is required as well.

To test our implementation, we have the script `build-riscv-tests.sh` to set up a docker instance, install the RISC-V toolchain and build the ELF files.

The Mozak VM implements the base RV32I instruction set with the M-extension,
so we are using rv32ui and rv32um ELF files from the [riscv-software-src/riscv-tests](https://github.com/riscv-software-src/riscv-tests) repo.
