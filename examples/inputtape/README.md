# To run

## Native

To run on your system, use the following command.
```sh
# in directory native
cargo run --release
```

This produces the `SystemTape` in both binary and debug formats.

## Mozak-VM

First, build the mozakvm binary:

```sh
# inside inputtape/mozakvm directory
cargo mozakvm-build --features="std"
```

To run mozakvm binary inside our runner, use the following command

```sh
# inside inputtape/mozakvm directory
cargo mozakvm-run 
```

Test producing proof for ELF executions in mozak-vm using the below command. Note that you must run
the native execution above to produce the system tape prior to running this.

```sh
# from project root
MOZAK_STARK_DEBUG=true \
    cargo run --bin mozak-cli -- prove-and-verify -vvv \
    examples/inputtape/mozakvm/target/riscv32im-mozak-mozakvm-elf/release/inputtape-mozakvm \
    --system-tape examples/inputtape/native/out/inputtape.tape.json \
```
