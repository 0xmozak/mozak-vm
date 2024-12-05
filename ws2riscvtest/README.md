# What
This directory aims at testing if we can convert a WASM binary to RISC-V target and make it run well on `mozak-vm`. We simulate a naive implementation of Fibonacci for this.

## Building and testing a WASM module
We need `wasm-pack` via
```
cargo install wasm-pack
```

Inside the directory `wasmtest`, we build a WASM module via:
```
wasm-pack build --target web
```
This builds binary for the target `wasm-unknown-unknown`

This would build artifacts accessible in the directory `./pkg`. Most importantly, these files would be generated:
1. `pkg/ws2riscvtest_bg.wasm`, the WASM binary file.
2. `pkg/ws2riscvtest.js`, the JS file required to run WASM code in browser

Run a dummy testing server (on port 8000 by default) via:
```
python3 -m http.server
```
and load up `http://localhost:8000/` in the browser. The correct run should output:
```
WASM TESTER! Function result: 34
```
