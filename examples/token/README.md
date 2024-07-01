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
# inside token/mozakvm directory
cargo mozakvm-build --features="std"
```

To run mozakvm binary inside our runner, use the following command

```sh
# inside token/mozakvm directory
cargo mozakvm-run 
```

Test producing proof for ELF executions in mozak-vm using the below command. Note that you must run
the native execution above to produce the system tape prior to running this.

```sh
# from project root
MOZAK_STARK_DEBUG=true \
    cargo run --bin mozak-cli -- prove-and-verify -vvv \
    examples/token/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/token-mozakvm \
    --system-tape examples/token/native/out/token.tape.json \
```
