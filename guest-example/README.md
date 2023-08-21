Guest Program
---

This repo contains two example cargo projects which generate ELF compatible with MozakVM.

**Build**

```
cargo +nightly build
```

**To dump assembly files**

```
RUSTFLAGS="--emit asm" cargo +nightly build
```

 Now check in `./target/riscv32im-risc0-zkvm-elf/debug/deps` for assembly files (with `.s` extension)
