# To run

To compile for Mozak-VM:
```sh
# from project root
cd examples && cargo +nightly build --release --bin stdin
```

To run on your system, use the following command (kindly change target triple as per your machine's architecture):
```sh
# from project root
cd examples && cargo +nightly run --target x86_64-unknown-linux-gnu --release --bin stdin-native --features="native"
```

Finally, you can use `mozak-cli` to run the example. Keep in mind that the built `stdin` program will be found in `examples/target/riscv32im-mozak-zkvm-elf/release`, while the `iotape.txt` can be found in `examples`. For example:

```sh
# from project root
cd examples && RUST_LOG=debug cargo run --release --bin stdin iotape.txt 
```
