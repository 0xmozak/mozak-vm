# Mozak Risc-V Virtual Machine

If you are unfamiliar with the Risc-V instruction set, please have a look at the [Risc-V instruction set reference](https://github.com/jameslzhu/riscv-card/blob/master/riscv-card.pdf).

# Installation

- [Rust toolchain](https://www.rust-lang.org/tools/install)

The Mozak VM is built in Rust, so the Rust toolchain is a pre-requisite.

```bash
cargo build
```

# Update official Risc-V tests

- [Docker](https://www.docker.com/)

Updating the official Risc-V tests relies on Docker to install the RISC-V toolchain and build the ELF files necessary for our tests to run.

The Mozak VM implements the base RV32I instruction set with the M-extension,
so we are using rv32ui and rv32um ELF files from the [riscv-software-src/riscv-tests](https://github.com/riscv-software-src/riscv-tests) repo.

You can update the tests via:

```bash
cd vm/tests/create_testdata/
./update_testdata
```

# Updating Rust toolchain

To update the Rust toolchain, change `rust-toolchain.toml`.
