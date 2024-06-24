Example of a Counter as `StateObject`

# To run

To run on your system, use the following command.
```sh
# in directory mozakvm
cargo mozakvm-run
```

This produces the `SystemTape` in both binary and debug formats in `native/out`.
It can be analyzed to see the CPC calls and emitted events.

Test producing proof for ELF executions  using the below command. Note that you must have produced
tapes and binary using above command.

```sh
# from project root
MOZAK_STARK_DEBUG=true \\
    cargo run --bin mozak-cli --features="parallel" -- prove-and-verify -vvv \
    examples/counter/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/counter-mozakvm \
    --system-tape examples/counter/native/out/tape.json
```
