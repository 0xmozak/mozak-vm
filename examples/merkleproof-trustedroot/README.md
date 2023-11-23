# To run

To compile for Mozak-VM:

```sh
# from project root
cargo +nightly build --release --bin merkleproof-trustedroot
```

To run on your system, use the following command (kindly change [target triple](https://doc.rust-lang.org/cargo/appendix/glossary.html#target) as per your machine's architecture):

```sh
# from project root
cargo run --release --features="native" --bin merkleproof-trustedroot-native --target aarch64-apple-darwin
```
