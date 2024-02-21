# The decode command

The decode command is used to decode the program. To print the program, run

```rust
mozak-cli -vvv decode <ELF>
```

where `<ELF>` is the path to the ELF file. If you are running `cargo build --release`, it is usually in the

```rust
target/risc32im-mozak-mozakvm-elf/release/<name>
```

folder where `<name>` is the program name.