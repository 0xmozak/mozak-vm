# To run

## Native

To run on your system, use the following command (kindly change [target triple](https://doc.rust-lang.org/cargo/appendix/glossary.html#target) as per your machine's architecture):

```sh
# from project root
cargo run --release --features="native" --bin wallet-native --target aarch64-apple-darwin
```

This produces the `SystemTape` in both binary and debug formats.

## Mozak-VM

First, build the mozakvm binary:

```sh
# inside examples directory
cargo build --release --bin walletbin --features="std"
```

Test the ELF in mozak-vm using the below command. Note that you must run
the native execution above to produce the system tape prior to running this.

```sh
# from project root
MOZAK_STARK_DEBUG=true \
    cargo run --bin mozak-cli -- prove-and-verify -vvv \
    examples/target/riscv32im-mozak-mozakvm-elf/release/walletbin \
    --system-tape examples/wallet_approve.tape.json \
    --self-prog-id \
    MZK-b10da48cea4c09676b8e0efcd806941465060736032bb898420d0863dca72538;
```
