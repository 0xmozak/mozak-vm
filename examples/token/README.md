# To run

To compile for Mozak-VM:

```sh
# inside examples directory
cargo +nightly build --release --bin token
```

To run on your system, use the following command (kindly change [target triple](https://doc.rust-lang.org/cargo/appendix/glossary.html#target) as per your machine's architecture):

```sh
# from project root
cargo run --release --features="native" --bin token-native --target aarch64-apple-darwin
```

Test the ELF in mozak-vm using:
```
MOZAK_STARK_DEBUG=true ./target/debug/mozak-cli prove-and-verify \
    examples/target/riscv32im-mozak-zkvm-elf/release/token \
    examples/token/private_input.tape \
    examples/token/public_input.tape
```
