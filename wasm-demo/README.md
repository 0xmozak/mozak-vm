This demo tries to run Mozak-VM and its proof system on WASM. For now it just tries to execute single instruction of ADD.
The execution works :sparkles: but proving fails as described below.

To Compile:

`wasm-pack build --target web`

Then run webserver from wasm-demo dir with following command

`python3 -m http.server`

Open local server's URL in browser and you should see two prompts, first after execution and second after proving.
More details about how to compile [Rust_to_Wasm](https://developer.mozilla.org/en-US/docs/WebAssembly/Rust_to_Wasm)
