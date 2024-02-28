This is an "empty" example, needed to test the behavior of the compiler with and without the
linker script. Linker script modifies the memory layout of ELF generated and can be found to
be documented [here](../../docs/linker-script.md).

# To run

To build for Mozak-VM:

```
# inside examples directory
# [overseer/0-0]
cargo +nightly build --release --bin empty
```
