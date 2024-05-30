# Guest Programs

*WARNING*: this workspace specifies default cargo target as `riscv32im-mozak-mozakvm-elf`, which means that for building native versions we need to manually specify the system target via `--target` (see below).

Examples contains cargo projects which generate ELF compatible with MozakVM. The target ISA is RISC-V with I and M extensions, described best in `.cargo/riscv32im-mozak-mozakvm-elf.json`.

Building the programs require Rust nightly toolchain. Exploring the generated ELF requires RISC-V toolkit, especially `objdump` or equivalent.

## Building ELFs

### Mozak ZK-VM

By default, we configure Cargo to build for the mozak-mozakvm, so a plain
build command uses our custom target.

```bash
cargo build --release
```

Some examples use `std`:

```bash
cargo build --release --features=std
```

This would build ELF executables under `target/riscv32im-mozak-mozakvm-elf/release/`.

For more details, our configuration is found at `.cargo/config.toml` at the root of the `examples` directory.

### Native

To build for native targets, we need to manually specify the host target, which is returned by `rustc -vV`:

```bash
cargo build --release \
            --target "$(rustc -vV | grep host | awk '{ print $2; }')" \
            --features=std
```

Currently we don't support `no_std` for the native target so `--features=std` is a must.

You can build a particular example binary by specifying it with `--bin`, for instance to build `empty` use
```bash
cargo build --release \
            --target "$(rustc -vV | grep host | awk '{ print $2; }')" \
            --features=std \
            --bin empty
```

This would build ELF executables under `target/x86_64-unknown-linux-gnu/release/`.

## Running ELFs

### Mozak ZK-VM

The RISC-V ELFs can be used with `mozak-cli`.

To build mozak-cli (from project root):

```bash
cargo build --package mozak-cli --release
```

To run executables, for example, `min-max` example (from examples directory):

```bash
cargo run --bin min-max
```

Note: For `cargo run` to work `mozak-cli` must be present at `../target/release/`, i.e already compiled in release mode.

Otherwise use `mozak-cli`'s run command to execute generated ELF.
```bash
mozak-cli -vvv run target/riscv32im-mozak-mozakvm-elf/debug/<ELF_NAME>
```

### Native

Again, for `cargo run` you need to manually specify the system target and manually specify the binary.  For instance, to run `empty` use

```bash
cargo run --release \
          --target "$(rustc -vV | grep host | awk '{ print $2; }')" \
          --features=std \
          --bin empty
```

You can either run the binaries directly at
```bash
./target/<SYSTEM_TARGET>/<debug or release>/<EXECUTABLE_NAME>
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
