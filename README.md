![CI status](https://github.com/0xmozak/mozak-vm/actions/workflows/ci.yml/badge.svg)
![Unused dependencies status](https://github.com/0xmozak/mozak-vm/actions/workflows/unused-deps.yml/badge.svg)
![MacOS CI status](https://github.com/0xmozak/mozak-vm/actions/workflows/macos-ci.yml/badge.svg)

# Mozak RISC-V Virtual Machine

If you are unfamiliar with the RISC-V instruction set, please have a look at the [RISC-V instruction set reference](https://github.com/jameslzhu/riscv-card/blob/master/riscv-card.pdf).

## Setting up your build environment

Below are instructions for setting up development environment. For instructions on enrolling CI machines, see the [notion page](https://www.notion.so/0xmozak/Enroll-Self-Hosted-CI-Runner-af6ddd3897594970b6ec4106ebde228f)

### Quickstart in GitHub codespaces

You can [open this repository in GitHub Codespaces](https://codespaces.new/0xmozak/mozak-vm?quickstart=1), click on `Create new codespace` and start developing straight away in your browser.  All build requirements will be taken care of. You can stop or resume the instance anytime.  (You can also find the '<> Code' button on the top right of the main page of the repository on GitHub to access the codespaces you created.  See the [Codespaces documentation](https://github.com/features/codespaces) for background information.)

### Local Build requirements

Mozak VM is built in Rust, so [installing the Rust toolchain](https://www.rust-lang.org/tools/install) is a pre-requisite, if you want to develop on your local machine.

### Rust Analyzer

Because we are using nightly versions of Rust toolchain we prefer to
use nightly versions of `rust-analyzer` which can understand those
nightly releases of Rust.

Previously, we were specifying `rust-analyzer` as a component in
`rust-toolcahin.toml`, however this made an artificial restriction on
the version of toolchain we could use if `rust-analyzer` would break
on our codebase.  In addition, it is much easier to update
`rust-analyzer` once it is fixed, rather than update the whole Rust
toolchain.

Therefore we recommed to let Developer handle the version of
`rust-analyzer` thay want to use.  Most editors can automatically
download the latest version of `rust-analyzer`:

- VSCode extension will download the latest version of `rust-analyzer`.  If you
  need the `nightly` version, you can switch to `pre-release` version
  of the VSCode extension;
- Zed will automatically download the latest nightly version of `rust-analyzer`;
- Emacs and NeoVim can be pointed to [manually
  installed](https://rust-analyzer.github.io/manual.html#rust-analyzer-language-server-binary)
  `rust-analyzer` binary downloaded from the [GitHub Releases
  Page](https://github.com/rust-lang/rust-analyzer/releases).

#### Working with Code Targetting MozakVM

Please use `rust-analyzer` released _after_ `2024-04-01`.

In order for `rust-analyzer` to work with code that targets `mozakvm` we need to

- start it with the right configuration, and
- prevent it from building `test` crate.

Rust does not provide prebuilt `std` for neither for MozakVM, nor for
`riscv32-im` which MozakVM is based on, therefore we need to build our version
of `std` by using `-Zbuild-std` feature.  However, that also requires that we
specify `restricted_std` in crates that we build, which will break `test` crate,
as it it not marked by `restricted_std`.

In Visual Studio Code:

- instead of opening the whole project, use `Open Folder...` to open the
  directory containing `.cargo` directory that specifies a `mozakvm` taget in
  `.cargo/config.toml`.
- in the same directory containing `.cargo`, create `.vscode` directory and put
  the following in `.vscode/settings.json`:

  ```json
  {
    "rust-analyzer.cargo.allTargets": false,
  }
  ```

By opening the directory containing `.cargo` directly, we ensure that
`rust-analyzer` will pick the configuration from it.  By setting
`rust-analyzer.cargo.allTargets` to false, we prevent `rust-analyzer` from
passing `--all-targets` to `cargo`, which will prevent building the `test`
crate.

For example, to open `mozak-vm/sdk` directory in Visual Studio Code, we

- open `mozak-vm/sdk` directory directly,
- add `rust-analyzer.cargo.allTargets: false` to
  `mozak-vm/sdk/.vscode/settings.json`, and
- reload Visual Studio Code window to reload the configuration.

#### Diagnosing Issues

If you are experiencing issues with Rust analyzer you can check if it
can correctly analyse our codebase from the command line.  Remember to
figure out the location of your `rust-analyzer` binary used by your
text editor.

Start by running `rust-analyzer analysis-stats` on our codebase

```bash
mozak-vm $ rust-analyzer analysis-stats .
Database loaded:     1.99s, 0b (metadata 359.47ms, 0b; build 823.73ms, 0b)
  item trees: 194
Item Tree Collection: 72.07ms, 0b
  crates: 20, mods: 311, decls: 4142, bodies: 3161, adts: 336, consts: 256
Item Collection:     7.70s, 0b
Body lowering:       1.01s, 0b
  exprs: 98389, ??ty: 73 (0%), ?ty: 37 (0%), !ty: 7
  pats: 17493, ??ty: 4 (0%), ?ty: 6 (0%), !ty: 0
Inference:           65.56s, 0b
MIR lowering:        8.71s, 0b
Mir failed bodies: 25 (0%)
Data layouts:        56.44ms, 0b
Failed data layouts: 3 (1%)
Const evaluation:    109.09ms, 0b
Failed const evals: 4 (1%)
Total:               83.22s, 0b
```

If it doesn't report any panics then your version `rust-analyzer`
should be able to handle our codebase.

If it panics, then you should report it as a bug to [`rust-analyzer`
upstream](https://github.com/rust-lang/rust-analyzer).

You can try to narrow down a code sample.  One helpful tool is
`rust-analyzer highlight` which can be run on a particular source
file, but be warned that some errors span multiple files, and won't be
picked by `rust-analyzer hightlight`.

## Building

```bash
cargo build
```

## Running test

To run all the tests in this repo, use:

```bash
cargo test
```

Selectively run tests using the following command:

```bash
cargo test --package <pkg> --lib -- <testname> --exact --nocapture
```

For example:

```bash
cargo test --package mozak-circuits --lib -- cross_table_lookup::tests::test_ctl --exact --nocapture
```

## Running

We have a rudimentary CLI.  You can run it via eg `cargo run -- -vvv run vm/tests/testdata/rv32ui-p-addi`.

Use `cargo run -- --help` to see what sub-commands are implemented.

## Update official RISC-V tests

- [Docker](https://www.docker.com/)

Updating the official RISC-V tests relies on Docker to install the RISC-V toolchain and build the ELF files necessary for our tests to run.

The Mozak VM implements the base RV32I instruction set with the M-extension,
so we are using rv32ui and rv32um ELF files from the [riscv-software-src/riscv-tests](https://github.com/riscv-software-src/riscv-tests) repo.

You can update the tests via `./update_testdata` in the root of the repository.

## Updating Rust toolchain

To update the Rust toolchain you need to
- update `rust-toolchain.toml`, and
- update `flake.nix`.

The easiest way to update `flake.nix` is to set the `sha256` in
`packages.rust-toolchain` to an empty string `""`:

```nix
        packages.rust-toolchain = pkgs.fenix.fromToolchainFile {
          file = ./rust-toolchain.toml;
          sha256 = "";
        };
```

and then try to build `rust-toolchain` using `nix build --no-link
.#rust-toolchain`.  Nix will then assume a default hash, and then
report a hash mismatch.  You can then copy the reported hash back to
the file.

```bash
$ nix build --no-link .#rust-toolchain
warning: found empty hash, assuming 'sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA='
<...>
         specified: sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=
            got:    sha256-kfnhNT9AcZARVovq9+6aay+4rOV3G7ZRdmMQdbd9+Pg=
```

# Mozak Node

Welcome to zk-backed high throughput stateful network!

## Building and contributing

- See [building](docs/building.md) for building the components for running the system.
- See [contributing](docs/constributing.md) for guidelines on contributions towards to the codebase.

## Components

- `sdk/` hosts interfaces for building programs for the platform.
- `rpc/` hosts server implementation for RPC interactions with the platform.
- `node-cli/` hosts command-line interface for managing running nodes.

### Docs

Architecture docs along with other design overwiews are available in `docs/`. Relevant docs are inter-spread in the codebase as comments.

## Licenses

All crates of this monorepo are licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
