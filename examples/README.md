# Guest Programs

Examples contains cargo projects which generate ELF compatible with MozakVM. The target ISA is RISC-V with I and M extensions, described best in `.cargo/riscv32im-mozak-zkvm-elf.json`.

Building the programs require Rust nightly toolchain. Exploring the generated ELF requires RISC-V toolkit, especially `objdump` or equivalent.

## Building ELFs

### To build for mozak-vm
```bash
cargo build --release
```
if example is using `std` crate then pass `--features=std` too.

This would build ELF executables under `target/riscv32im-mozak-zkvm-elf/release/`.

### To build for x86_64 with Linux OS
```bash
cargo build --release --target x86_64-unknown-linux-gnu --features=std
```
Currently we don't support `no_std` for the native target so `--features=std` is a must.

This would build ELF executables under `target/x86_64-unknown-linux-gnu/release/`.

## Running ELFs

### To run ELFs on mozak-vm
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
### To run ELFs on X86_64 with Linux OS
```bash
./target/x86_64-unknown-linux-gnu/debug/<EXECUTABLE_NAME>
```

## Exploring binaries
### To dump assembly files
```bash
RUSTFLAGS="--emit asm -Cpasses=loweratomic -Zlocation-detail=none -Clink-arg=-T./.cargo/riscv32im-mozak-zkvm.ld" cargo +nightly build --target ../.cargo/riscv32im-mozak-zkvm-elf.json
```
After this, `target/riscv32im-risc0-zkvm-elf/debug/deps/` would contain assembly files with `.s` extension

### Exploring via `objdump`
`objdump` utility (differently built for riscV) can be fetched via
- MacOS: https://github.com/riscv-software-src/homebrew-riscv
- Others: https://github.com/riscv-software-src/riscv-tools

Once done, this should feature as `riscv64-unknown-elf-objdump` in your `$PATH`. Use the following to explore ELFs:

**Find sections**
```bash
riscv64-unknown-elf-objdump -h target/riscv32im-mozak-zkvm-elf/debug/<ELF_NAME>
```
**Find contents of specific section**
```bash
riscv64-unknown-elf-objdump -d -j .sdata target/riscv32im-mozak-zkvm-elf/debug/<ELF_NAME>
```

NOTE: The build config tries to optimize binary size, and location information is removed. Kindly update config if you want location info.
