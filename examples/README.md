# Guest Programs

*WARNING*: this workspace specifies default cargo target as native, which means that for building mozakvm versions we need to manually specify the system target via `--target` (see below), as well as build std libraries for the platform with `Zbuild-std` unstable feature. But as long as we are using the provided commands `cargo mozakvm-build` and `cargo mozakvm-run`, everything should be taken care of under the hood.

Example contains cargo projects which generate ELF compatible with MozakVM. The target ISA is RISC-V with I and M extensions, described best in `.cargo/riscv32im-mozak-mozakvm-elf.json`.

Building the programs require Rust nightly toolchain. Exploring the generated ELF requires RISC-V toolkit, especially `objdump` or equivalent.

### Mozak ZK-VM
Each example has `mozakvm` directory inside, which contains the code for our guest programs.
We can use following command to build it for `riscv32im-mozak-mozakvm-elf` target.

```bash
# inside {example}/mozakvm
cargo mozakvm-build
```

By default, our examples are `no_std`. Examples can make use of `std` through feature flag:
```bash
cargo mozakvm-build --features=std
```

This would build ELF executables under `{example}/target/riscv32im-mozak-mozakvm-elf/mozak-release/{example}-mozakvm`.
Note the profile `mozak-release` in above path. Defined in `.cargo/config.toml`. It's intended to build optimized ELF for mozakvm
targets. The command `mozakvm-build` ensures that this profile is used while building.

For more details, our configuration is found at `.cargo/config.toml` in the root directory

### Native

To build for native targets, we can `cd` into `native` directory, and use usual rust commands to build

```bash
# inside {example}/native
cargo build --release 
```

## Running ELFs

### Mozak ZK-VM

The RISC-V ELFs can be run with our CLI. Simply use the command `cargo mozakvm-run`, which invokes the cli command `run` under the hood.

```bash
# in example/mozakvm
cargo mozakvm-run 
```
### Native

Native example can be run as usual with cargo

```bash
# in example/native
cargo run --release
```

## Exploring binaries

### To dump assembly files
```bash
RUSTFLAGS="--emit asm" cargo mozakvm-build
```
After this, `target/riscv32im-risc0-mozakvm-elf/debug/deps/` would contain assembly files with `.s` extension

### Exploring via `objdump`
`objdump` utility (differently built for riscV) can be fetched via
- MacOS: https://github.com/riscv-software-src/homebrew-riscv
- Others: https://github.com/riscv-software-src/riscv-tools

Once done, this should feature as `riscv64-unknown-elf-objdump` in your `$PATH`. Use the following to explore ELFs:

**Find sections**
```bash
riscv64-unknown-elf-objdump -h {example}/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/<ELF_NAME>
```
**Find contents of specific section**
```bash
riscv64-unknown-elf-objdump -d -j .sdata {example}/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/<ELF_NAME>
```

NOTE: The build config tries to optimize binary size, and location information is removed. Kindly update config if you want location info.
