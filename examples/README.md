# Guest Programs

Examples contains cargo projects which generate ELF compatible with MozakVM. The target ISA is Risc-V with I and M extensions, described best in `.cargo/riscv32im-mozak-zkvm-elf.json`.

Building the programs require Rust nightly toolchain. Exploring the generated ELF requires Risc-V toolkit, especially `objdump` or equivalent.

## Building ELFs

```bash
cargo +nightly build
```
This would build ELF executables under `target/riscv32im-mozak-zkvm-elf/debug/`.

## Running ELFs
Use `mozak-cli`'s run command to execute generated ELF.

```bash
mozak-cli -vvv run target/riscv32im-mozak-zkvm-elf/debug/<ELF_NAME>

```

## Exploring binaries
### To dump assembly files
```bash
RUSTFLAGS="--emit asm" cargo +nightly build
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
