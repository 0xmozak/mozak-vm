# The run command

The run command is used to execute the program.

```rust
mozak-cli run <ELF> <PRIVATE_TAPE> <PUBLIC_TAPE>
```

where `<ELF>` is the path to the ELF file. If you are running `cargo build --release`, it is usually in the

```rust
target/risc32im-mozak-mozakvm-elf/release/<name>
```

folder where `<name>` is the program name.

`<PRIVATE_TAPE>` and `<PUBLIC_TAPE>` are private and public inputs to the program
