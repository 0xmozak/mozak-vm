# Examples Builder

This crate allows easy embedding of example `elf` binaries into your code, usually for testing purposes.

Simply add `mozak-examples` to your dependencies and enable the `features` for the binaries you want. Then reference the corresponding globals.

```toml
# Cargo.toml
# ...

[dependencies]
mozak-examples = { path = "../examples-builder", features = ["fibonacci"] }
# ...
```

```rust
// foo.rs
use mozak_examples::FIBONACCI_ELF;
```
