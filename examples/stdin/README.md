# To run

To compile for Mozak-VM:
```sh
cargo +nightly build --release --bin stdin
```

To run on your system, use the following (kindly change target triple as per your machine's architecture):
```sh
# from root
cd examples && cargo +nightly run --target x86_64-unknown-linux-gnu --release --bin stdin-native --features="native"`
```

You will have to type some input to be captured on the IO tape.

Finally, you can use `mozak-cli` to run the example. Keep in mind that the built `stdin` program will be found in `./examples/target/riscv32im-mozak-zkvm-elf/release`, while the `iotape.txt` can be found in `examples`. For example:

```sh
# from project root
cargo run --bin mozak-cli run ./examples/target/riscv32im-mozak-zkvm-elf/release/stdin ./examples/iotape.txt 
```
