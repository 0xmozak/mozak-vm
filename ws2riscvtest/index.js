// Import our outputted wasm ES6 module
// Which, export default's, an initialization function
import init from "./pkg/ws2riscvtest.js";

const runWasm = async () => {
  // Instantiate our wasm module
  const module = await init("./pkg/ws2riscvtest_bg.wasm");

  // Call the function exported from wasm, save the result
  const funcResult = module.fibonacci(BigInt("8"));

  // Set the result onto the body
  document.body.textContent = `WASM TESTER! Function result: ${funcResult}`;
};
runWasm();
