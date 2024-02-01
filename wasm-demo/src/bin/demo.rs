// How to run
//
// Install wasm32-wasi target:
//
// rustup target add wasm32-wasi
//
// Compile
//
// cargo build --target wasm32-wasi
//
// Run using wasmtime
//
// RUST_BACKTRACE=1 WASMTIME_BACKTRACE_DETAILS=1 wasmtime
// ../target/wasm32-wasi/debug/demo.wasm
//

pub fn main() { wasm_demo::wasm_demo_(99, 99); }
