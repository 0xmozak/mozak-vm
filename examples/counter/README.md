Example of a Counter as `StateObject`

## Flow

- core-logic -> define the dispatcher of each action and related things.
- native
  - src/main.rs -> you write all the external logic using core-logic and generate tapes out of it
  - out -> all the generate tapes get stored here.
- elf-data -> this directory is used as library for to fetch PROGRAM_IDENTIFIER
- mozakvm -> defines the entry point of VM, here we have a while loop which waits for dispatch methods to execute.

## Build

To build on your target VM, use the following command.

```sh
# in directory mozakvm
cargo mozakvm-build
# this gets stored in mozakvm/target/riscv32im-mozak-mozakvm-elf
```

## Run

To run on your system, use the following command.
here run = build + run the native program dump the tapes + emulate/run these tapes in vm build

```sh
# in directory mozakvm
cargo mozakvm-run
```

This produces the `SystemTape` in both binary and debug formats in `native/out`.
It can be analyzed to see the CPC calls and emitted events.

Test producing proof for ELF executions using the below command. Note that you must have produced
tapes and binary using above command.

- Run the below commands in root dir

## Prove

```sh
# --features="parallel" is for speeding up the proving by generating proof for multiple STARKy tables in parallel.
MOZAK_STARK_DEBUG=true \
    cargo run --bin mozak-cli --features="parallel" -- prove -vvv \
    examples/counter/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/counter-mozakvm \
    examples/counter/starkyProof.bin \
    examples/counter/recursiveProof.bin \
    --system-tape examples/counter/native/out/tape.json
```

## Verify

- STARKY Proof -> hashmap of tableType vs proof (aka AllProof in code)

```sh
MOZAK_STARK_DEBUG=true \
    cargo run --bin mozak-cli --features="parallel" -- verify -vvv \
    examples/counter/starkyProof.bin
```

- Recursive Proof
Now here, we also need the Program Identifier, so let's first generate that.

```sh
PROGRAM_ID=$(MOZAK_STARK_DEBUG=true \
  cargo run --bin mozak-cli -- self-prog-id -vvv \
  examples/counter/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/counter-mozakvm | tail -n 1)
```

```sh
MOZAK_STARK_DEBUG=true \
  cargo run --bin mozak-cli --features="parallel" -- verify-recursive-proof -vvv \
  examples/counter/recursiveProof.bin \
  examples/counter/recursiveProof.vk \
  $PROGRAM_ID
```

## Prove and Verify

```sh
# from project root
MOZAK_STARK_DEBUG=true \
    cargo run --bin mozak-cli --features="parallel" -- prove-and-verify -vvv \
    examples/counter/mozakvm/target/riscv32im-mozak-mozakvm-elf/mozak-release/counter-mozakvm \
    --system-tape examples/counter/native/out/tape.json
```
