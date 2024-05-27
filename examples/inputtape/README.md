# To run

## Native

To run on your system, use the following command (kindly change [target triple](https://doc.rust-lang.org/cargo/appendix/glossary.html#target) as per your machine's architecture):

```sh
# from project root
cargo run --release --bin inputtape-native 
```

This produces the `SystemTape` in both binary and debug formats.

## Mozak-VM

First, build the mozakvm binary:

```sh
# inside inputtape/mozakvm directory
cargo build-mozakvm --features="std"
```

To run mozakvm binary inside our runner, use the following command

```sh
# inside inputtape/mozakvm directory
cargo run-mozakvm --features="std" -- --self-prog-id MZK-b10da48cea4c09676b8e0efcd806941465060736032bb898420d0863dca72538
```

Test producing proof for ELF executions in mozak-vm using the below command. Note that you must run
the native execution above to produce the system tape prior to running this.

```sh
# from project root
MOZAK_STARK_DEBUG=true \
    cargo run --bin mozak-cli -- prove-and-verify -vvv \
    examples/target/riscv32im-mozak-mozakvm-elf/release/inputtapebin \
    --system-tape examples/inputtape/out/inputtape.tape.json \
    --self-prog-id \
    MZK-b10da48cea4c09676b8e0efcd806941465060736032bb898420d0863dca72538;
```
