# Guest Programs

Examples contains cargo projects which generate ELF compatible with MozakVM. The target ISA is Risc-V with I and M extensions, described best in `.cargo/riscv32im-mozak-zkvm-elf.json`.

Building the programs require rust nightly toolchain. Exploring the generated ELF requires riscV toolkit, especially `objdump` or equivalent.

## Building ELFs

```bash
cargo +nightly build
```
This would build ELF executables under `target/riscv32im-mozak-zkvm-elf/debug/`. Ensure building under `debug` (non `--release` mode) so as to preserve unoptimised code useful for testing later.

## Exploring ELF files
### Dumping `.text` section
The `.text` section of the elf files can be dumped via
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
