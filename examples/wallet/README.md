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

Prove and verify:

```
../target/debug/mozak-cli prove-and-verify -vvv target/riscv32im-mozak-mozakvm-elf/release/walletbin --system-tape wallet_approve.tape_bin --self-prog-id MZK-155a7957-1f2314bd-0
```
