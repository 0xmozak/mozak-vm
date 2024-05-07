# Command Line Tool

The `mozak-cli` command-line tool is used to interact with the ELF.

After you have [installed](../guide/installation.md) `mozak-cli`, you can run the `mozak-cli help` command in your terminal to view the available commands.

This following sections provide detailed information on the different commands available.

* [`mozak-cli decode <ELF>`](decode.md) — Decode a given ELF and prints the program.
* [`mozak-cli run <ELF> <PRIVATE_TAPE> <PUBLIC_TAPE>`](run.md) — Decode and execute a given ELF. Prints the final state of the registers.
* [`mozak-cli prove-and-verify <ELF> <PRIVATE_TAPE> <PUBLIC_TAPE>`](prove-and-verify.md) — Prove and verify the execution of a given ELF.
* [`mozak-cli prove <ELF> <PRIVATE_TAPE> <PUBLIC_TAPE> <PROOF>`](prove.md) — Prove the execution of given ELF and write proof to file.
* [`mozak-cli verify <PROOF>`](verify.md) — Verify the given proof from file.
* [`mozak-cli program-rom-hash <ELF>`](program-rom-hash.md) — Compute the Program Rom Hash of the given ELF.
* [`mozak-cli memory-init-hash <ELF>`](memory-init-hash.md) — Compute the Memory Init Hash of the given ELF.
* [`mozak-cli bench`](bench.md) - Bench the function with given parameters.

As a general note, you can run the command with `-vvv` to get debug level information. For example

```rust
mozak-cli -vvv run ...
```

Replace `<ELF>` is the path to the ELF file. If you are running `cargo build --release`, it is usually in the

```rust
target/risc32im-mozak-mozakvm-elf/release/<name>
```

folder where `<name>` is the program name.

Replace `<PRIVATE_TAPE>` and `<PUBLIC_TAPE>` to paths to the private and public inputs of the program
