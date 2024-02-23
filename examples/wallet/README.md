# To run

To compile for Mozak-VM:

```sh
# inside examples directory
cargo +nightly build --release --bin walletbin
```

To run on your system, use the following command (kindly change [target triple](https://doc.rust-lang.org/cargo/appendix/glossary.html#target) as per your machine's architecture):

```sh
# from project root
cargo run --release --features="native" --bin wallet-native --target aarch64-apple-darwin
```

Test the ELF in mozak-vm using:
```
MOZAK_STARK_DEBUG=true ./target/debug/mozak-cli prove-and-verify \
    examples/target/riscv32im-mozak-zkvm-elf/release/wallet \
    examples/wallet/private_input.tape \
    examples/wallet/public_input.tape
```

Prove and verify:

```
../target/debug/mozak-cli prove-and-verify -vvv target/riscv32im-mozak-zkvm-elf/release/walletbin --system-tape wallet_tfr.tape_bin --self-prog-id MZK-155a7957-1f2314bd-0
```
