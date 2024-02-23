# To run

To compile for Mozak-VM:

```sh
# inside examples directory
cargo +nightly build --release --bin merkleproof-trustedroot
```

To run on your system, use the following command (kindly change [target triple](https://doc.rust-lang.org/cargo/appendix/glossary.html#target) as per your machine's architecture):

```sh
# from project root
cargo run --release --features="native" --bin merkleproof-trustedroot-native --target aarch64-apple-darwin
```

Test the ELF in mozak-vm using:
```
MOZAK_STARK_DEBUG=true ./target/debug/mozak-cli prove-and-verify \
    examples/target/riscv32im-mozak-mozakvm-elf/release/merkleproof-trustedroot \
    examples/merkleproof-trustedroot/private_input.tape \
    examples/merkleproof-trustedroot/public_input.tape
```
