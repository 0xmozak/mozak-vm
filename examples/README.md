# Guest Programs

*WARNING*: this workspace specifies default cargo target as native, which means that for building mozakvm versions we need to manually specify the system target via `--target` (see below), as well as build std libraries for the platform with `Zbuild-std` unstable feature. But as long as we are using the provided commands `cargo build-mozakvm` and `cargo run-mozakvm`, everything should be taken care of under the hood.

Example contains cargo projects which generate ELF compatible with MozakVM. The target ISA is RISC-V with I and M extensions, described best in `.cargo/riscv32im-mozak-mozakvm-elf.json`.

Building the programs require Rust nightly toolchain. Exploring the generated ELF requires RISC-V toolkit, especially `objdump` or equivalent.

### Mozak ZK-VM
Each example has `mozakvm` directory inside, which contains the code for our guest programs.
We can use following command to build it for `riscv32im-mozak-mozakvm-elf` target.

```bash
# inside example/mozakvm
cargo build-mozakvm
```

Some examples use `std`:
```bash
cargo build --release --features=std
```

This would build ELF executables under `target/riscv32im-mozak-mozakvm-elf/release/`.

For more details, our configuration is found at `.cargo/config.toml` at the root of the `examples` directory.

### Native

To build for native targets, we can `cd` into `native` directory, and use usual rust commands to build

```bash
cargo build --release 
```

This would build ELF executables under `target/{{NATIVE_ARCH_TRIPLE}}/release/`.

## Running ELFs

### Mozak ZK-VM

The RISC-V ELFs can be run with our CLI. Simply use the command `cargo run-mozakvm`, which invokes the cli command `run` under the hood.

```bash
# in example/mozakvm
cargo run-mozakvm -- --self-prog-id SELF_PROG_ID_HERE
```
### Native

Native example can be run as usual with cargo

```bash
cargo run --release
```

## Exploring binaries

### To dump assembly files
```bash
RUSTFLAGS="--emit asm" cargo +nightly build
```
After this, `target/riscv32im-risc0-mozakvm-elf/debug/deps/` would contain assembly files with `.s` extension

### Exploring via `objdump`
`objdump` utility (differently built for riscV) can be fetched via
- MacOS: https://github.com/riscv-software-src/homebrew-riscv
- Others: https://github.com/riscv-software-src/riscv-tools

Once done, this should feature as `riscv64-unknown-elf-objdump` in your `$PATH`. Use the following to explore ELFs:

**Find sections**
```bash
riscv64-unknown-elf-objdump -h target/riscv32im-mozak-mozakvm-elf/debug/<ELF_NAME>
```
**Find contents of specific section**
```bash
riscv64-unknown-elf-objdump -d -j .sdata target/riscv32im-mozak-mozakvm-elf/debug/<ELF_NAME>
```

NOTE: The build config tries to optimize binary size, and location information is removed. Kindly update config if you want location info.
