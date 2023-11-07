![CI status](https://github.com/0xmozak/mozak-vm/actions/workflows/ci.yml/badge.svg)
![Unused dependencies status](https://github.com/0xmozak/mozak-vm/actions/workflows/unused-deps.yml/badge.svg)
![MacOS CI status](https://github.com/0xmozak/mozak-vm/actions/workflows/macos-ci.yml/badge.svg)

# Mozak RISC-V Virtual Machine

If you are unfamiliar with the RISC-V instruction set, please have a look at the [RISC-V instruction set reference](https://github.com/jameslzhu/riscv-card/blob/master/riscv-card.pdf).

## Setting up your build environment

### Quickstart in GitHub codespaces

You can [open this repository in GitHub Codespaces](https://codespaces.new/0xmozak/mozak-vm?quickstart=1), click on `Create new codespace` and start developing straight away in your browser.  All build requirements will be taken care of. You can stop or resume the instance anytime.  (You can also find the '<> Code' button on the top right of the main page of the repository on GitHub to access the codespaces you created.  See the [Codespaces documentation](https://github.com/features/codespaces) for background information.)

### Local Build requirements

Mozak VM is built in Rust, so [installing the Rust toolchain](https://www.rust-lang.org/tools/install) is a pre-requisite, if you want to develop on your local machine.

## Bulding

```bash
cargo build
```

## Running test

To run all the tests in this repo, use:
```bash
cargo test
```

Selectively run tests using the following command:
```bash
cargo test --package <pkg> --lib -- <testname> --exact --nocapture
```

For example:
```bash
cargo test --package mozak-circuits --lib -- cross_table_lookup::tests::test_ctl --exact --nocapture
```

## Running

We have a rudimentary CLI.  You can run it via eg `cargo run -- -vvv run vm/tests/testdata/rv32ui-p-addi`.

Use `cargo run -- --help` to see what sub-commands are implemented.

## Update official RISC-V tests

- [Docker](https://www.docker.com/)

Updating the official RISC-V tests relies on Docker to install the RISC-V toolchain and build the ELF files necessary for our tests to run.

The Mozak VM implements the base RV32I instruction set with the M-extension,
so we are using rv32ui and rv32um ELF files from the [riscv-software-src/riscv-tests](https://github.com/riscv-software-src/riscv-tests) repo.

You can update the tests via `./update_testdata` in the root of the repository.

## Updating Rust toolchain

To update the Rust toolchain, change `rust-toolchain.toml`.

# Mozak Node

Welcome to zk-backed high throughput stateful network!

## Building and contributing
- See [building](docs/building.md) for building the components for running the system.
- See [contributing](docs/constributing.md) for guidelines on contributions towards to the codebase.

## Components
- `sdk/` hosts interfaces for building programs for the platform.
- `rpc/` hosts server implementation for RPC interactions with the platform.
- `node-cli/` hosts command-line interface for managing running nodes.

### Docs
Architecture docs along with other design overwiews are available in `docs/`. Relevant docs are inter-spread in the codebase as comments.
