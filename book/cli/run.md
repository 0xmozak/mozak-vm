# The run command

The run command is used to execute the program.

```rust
mozak-cli run <ELF> <IO_TAPE_PRIVATE> <IO_TAPE_PUBLIC>
```

where `<ELF>` is the path to the ELF file. If you are running `cargo build --release`, it is usually in the

```rust
target/risc32im-mozak-zkvm-elf/release/<name>
```

folder where `<name>` is the program name.

`<IO_TAPE_PRIVATE>` and `<IO_TAPE_PUBLIC>` are private and public inputs to the program