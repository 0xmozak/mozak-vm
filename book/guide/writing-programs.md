# Writing Programs

[RISC-V] is a general purpose instruction set architecture. As a RISC-V Zero-Knowledge Virtual Machine,
Mozak-VM aims to be able to prove and verify arbitrary programs that are compiled to RISC-V regardless of whether the program
was written in Rust, C++, or another language.

For now, we support the RV32I Base Integer Instructions and RV32M Multiply Extension Instructions of RISC-V and writing
programs in Rust.

If you are not sure what these instructions mentioned above include, checkout [a succinct reference of the RISC-V instructions].

## Executable and Linkable Format

At a high level, programs are compiled to RISC-V [ELFs], that are executed and proven independently. The execution generates a computation trace
and then proven with the Zero-Knowledge Proof System.

<!-- If you are interested in learning more about this check out architecture section (not written yet) -->

## Writing a simple fibonacci program

You can write what we term "guest programs" by adding the `guest` crate to your dependency.

```rust
[dependencies]
guest = { git = "https://github.com/0xmozak/mozak-vm", package = "guest", tag = "v0.1" }
```

<!---
Add cargo add command once `guest` is published to crate.io

```
cargo add guest
```
-->

We do not support the Rust standard library at the moment. Add the following to your `main.rs` or `lib.rs` file.

```rust
#![no_std]
```

If you are not familiar with how to write Rust programs in an environment without the standard library, check out the [Rust Embedded Book].

If you are writing a binary program, add name of the program and path to the executable to your `Cargo.toml` file.

```rust
[[bin]]
name = "fibonacci"
path = "main.rs"
```

and the following to your binary file

```
#![no_main]
```

use the `entry!()` macro of the guest crate as the entry of the `main()` function.

```rust
pub fn main() {
    ...
}

guest::entry!(main);

```

Here is the entire code of the fibonnaci program.

```rust
{{#include ../../examples/fibonacci/main.rs}}
```

Building the programs requires the [Rust nightly toolchain](https://www.rust-lang.org/tools/install). To Build the program, run

```rust
cargo build --release
```

This would build ELF executables under `target/riscv32im-mozak-mozakvm-elf/debug/`.

<!---
change the following to actual files after iotapes are added to examples
-->

use `mozak-cli`'s run command to execute generated ELF, where `<PRIVATE_TAPE>` and `<PUBLIC_TAPE>` are files containing the private and public inputs of the program.

```rust
mozak-cli run target/riscv32im-mozak-mozakvm-elf/release/<ELF_NAME> <PRIVATE_TAPE> <PUBLIC_TAPE>
```

For this fibonnacci example, both `<PRIVATE_TAPE>` and `<PUBLIC_TAPE>` are empty files.

To prove the execution of the program, run:

```rust
mozak-cli prove target/riscv32im-mozak-mozakvm-elf/release/<ELF_NAME> <PRIVATE_TAPE> <PUBLIC_TAPE> <PROOF>
```

where `<PROOF>` is the path to the proof file

To verify the execution of the program, run:

```rust
mozak-cli verify <PROOF>
```

If you want to see more examples, check out the examples in the [examples folder of our repository].




[RISC-V]: https://github.com/riscv/riscv-isa-manual/releases/tag/Ratified-IMAFDQC
[a succinct reference of the RISC-V instructions]: https://github.com/jameslzhu/riscv-card/blob/master/riscv-card.pdf
[Rust Embedded Book]: https://docs.rust-embedded.org/book/intro/no-std.html
[examples folder of our repository]: https://github.com/0xmozak/mozak-vm/tree/main/examples
[ELFs]: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
