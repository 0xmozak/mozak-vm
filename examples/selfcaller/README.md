# To run

## Native

To run on your system, use the following command (kindly change [target triple](https://doc.rust-lang.org/cargo/appendix/glossary.html#target) as per your machine's architecture):

```sh
# from project root
cargo run --release --features="native" --bin selfcaller-native --target aarch64-apple-darwin
```

This produces the `SystemTape` in both binary and debug formats.

## Mozak-VM

First, build the mozakvm binary:

```sh
# inside examples directory
cargo build --release --bin selfcallerbin 
```

Test the ELF in mozak-vm using the below command. Note that you must run
the native execution above to produce the system tape prior to running this.

```sh
# from project root
MOZAK_STARK_DEBUG=true \
    cargo run --bin mozak-cli -- prove-and-verify -vvv \
    examples/target/riscv32im-mozak-mozakvm-elf/release/selfcallerbin \
    --system-tape examples/selfcaller.tape.json \
    --self-prog-id \
    MZK-5b7b6135be198533f7c7ec46651216b762e6d47e69b408d1bc79d641f9ae06de;
```
