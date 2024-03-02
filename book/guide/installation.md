# Installation

There are many ways to install the CLI tool that helps interacting with the VM. Choose one below that best suit your needs.

<!---
# Precompiled Binary

Add Precompiled binary after tested on different platforms
related issue https://github.com/0xmozak/mozak-vm/issues/852
-->

<!---
# Crate.io

Add

```rust
cargo install
```

After publish to crate.io
-->

# Build from latest master version using Cargo

Check that Cargo is installed on your machine.

After that, run

```
cargo install --git https://github.com/0xmozak/mozak-vm.git mozak-cli
```

Also, make sure the Cargo bin directory is added to your `PATH`.

# Running the binary directly from codebase

clone the repo:

```
git clone https://github.com/0xmozak/mozak-vm.git && cd mozak-vm
```

run Mozak-VM CLI commands with

```
cargo run --bin mozak-cli [OPTIONS] <COMMAND>
```

If you are interested in making changes or found a bug related to the CLI, feel free to submit an [issue](https://github.com/0xmozak/mozak-vm/issues)
