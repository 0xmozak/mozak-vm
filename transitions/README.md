# Guest Programs

Examples contains Rust implemented transition functions, which can be compiled to the ELF, compatible with MozakVM. The
target ISA is Risc-V with I and M extensions, described best in `.cargo/riscv32im-mozak-zkvm-elf.json`.

Building the programs requires Rust nightly toolchain. Exploring the generated ELF requires Risc-V toolkit,
especially `objdump` or equivalent.

## Building ELFs

```bash
cargo +nightly build
```

This would build ELF executables under `target/riscv32im-mozak-zkvm-elf/debug/`.

## Running ELFs

The generated ELFs can be executed with `mozak-cli`.

To build mozak-cli, run (in the project root).

```bash
cargo build --package mozak-cli --release
```

After building `mozak-cli` use any of following ways to run the ELFs.

Cargo run command:

```bash
cargo run --bin <EXECUTABLE_NAME>
```

Example:

```bash
cargo run --bin min-max
```

Note: For `cargo run` to work `mozak-cli` must be present at `../target/release/`, i.e already compiled in release mode.

Otherwise use `mozak-cli`'s run command to execute generated ELF.

```bash
mozak-cli -vvv run target/riscv32im-mozak-zkvm-elf/debug/<ELF_NAME>

```

## Uploading ELFs to the network

TODO