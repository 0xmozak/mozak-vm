## To run

To compile for Mozak-VM:

```sh
# from project root
cd examples && cargo +nightly build --release --bin fibonacci-input
```

To run fibonacci example for value `n`

```sh
cd examples && echo -n "{n value here}" > iotape
cargo run --release --bin fibonacci-input iotape
```
