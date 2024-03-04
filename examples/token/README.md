# To run

## Native

To run on your system, use the following command (kindly change [target triple](https://doc.rust-lang.org/cargo/appendix/glossary.html#target) as per your machine's architecture):

```sh
# from project root
cargo run --release --features="native" --bin token-native --target aarch64-apple-darwin
```

This produces the `SystemTape` in both binary and debug formats.

## Mozak-VM

First, build the mozakvm binary:

```sh
# inside examples directory
cargo build --release --bin tokenbin
```

Test the ELF in mozak-vm using the below command. Note that you must run
the native execution above to produce the system tape prior to running this.

```sh
# from project root
MOZAK_STARK_DEBUG=true \
    cargo run --bin mozak-cli -- prove-and-verify -vvv \
    examples/target/riscv32im-mozak-mozakvm-elf/release/tokenbin \
    --system-tape examples/token_tfr.tape_bin \
    --self-prog-id MZK-0b7114fb-021f033e-00;
```
