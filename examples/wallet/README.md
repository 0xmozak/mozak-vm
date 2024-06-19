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
# inside wallet/mozakvm directory
cargo mozakvm-build --features="std"
```

To run mozakvm binary inside our runner, use the following command

```sh
# inside wallet/mozakvm directory
cargo mozakvm-run --features="std" -- --self-prog-id MZK-c51b8a31c98b9fe13065b485c9f8658c194c430843570ccac2720a3b30b47adb \
  --system-tape ../native/out/tape.json
```

Test producing proof for ELF executions in mozak-vm using the below command. Note that you must run
the native execution above to produce the system tape prior to running this.

```sh
# from project root
MOZAK_STARK_DEBUG=true \
    cargo run --bin mozak-cli -- prove-and-verify -vvv \
    examples/wallet/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/wallet-mozakvm \
    --system-tape examples/wallet/native/out/wallet.tape.json \
    --self-prog-id \
    MZK-c51b8a31c98b9fe13065b485c9f8658c194c430843570ccac2720a3b30b47adb;
```
