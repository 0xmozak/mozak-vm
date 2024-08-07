// The wasm-pack uses wasm-bindgen to build and generate JavaScript binding file.
// Import the wasm-bindgen crate.
use wasm_bindgen::prelude::*;

// Our fibonacci function
// wasm-pack requires "exported" functions
// to include #[wasm_bindgen]
#[wasm_bindgen]
pub fn fibonacci(a: u64) -> u64 {
    if a == 0 || a == 1 {
        return 1;
    }
    return fibonacci(a-1) + fibonacci(a - 2);
}
