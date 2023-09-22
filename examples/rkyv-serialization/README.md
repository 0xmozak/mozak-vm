# To run

To compile for Mozak-VM:

```sh
# from project root
cd examples && cargo +nightly build --release --bin rkyv-serialization
```

To run on your system, use the following command (kindly change [target triple](https://doc.rust-lang.org/cargo/appendix/glossary.html#target) as per your machine's architecture):

```sh
# from project root
cd examples && cargo +nightly run --target x86_64-unknown-linux-gnu --release --bin rkyv-serialization-native --features="native"
```
For more details on rkyv please check https://github.com/rkyv/rkyv
