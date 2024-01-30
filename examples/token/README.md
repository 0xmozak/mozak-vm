# AMM design
This example AMM is a simple pair pool based design similar to Uniswap V2 Pair. 
This AMM holds two different type of tokens interchangeable without a peer being online (the AMM design).
In state, AMM holds the following different objects:
1. **The *metadata* object**: Holds information regarding available reserves (both sides) and the program identifiers related to both sides. The "reserves" mentioned in this object is equivalent to accumulated sum of all tokens "economically owned" by the AMM at all times. Divergences in this value from the actual leads to wrong pricing and not handled in this example.
2. Multiple ***token* objects**: "Economically owned" token objects, that can be fed as the counterparty for token swap during a user's swap request invocation.

# To run

To compile for Mozak-VM:

```sh
# inside examples directory
cargo +nightly build --release --bin amm
```

To run on your system, use the following command (kindly change [target triple](https://doc.rust-lang.org/cargo/appendix/glossary.html#target) as per your machine's architecture):

```sh
# from project root
cargo run --release --features="native" --bin amm-native --target aarch64-apple-darwin
```

Test the ELF in mozak-vm using:
```
MOZAK_STARK_DEBUG=true ./target/debug/mozak-cli prove-and-verify \
    examples/target/riscv32im-mozak-zkvm-elf/release/amm \
    examples/amm/private_input.tape \
    examples/amm/public_input.tape
```
