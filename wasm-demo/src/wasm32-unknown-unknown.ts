export {};

import { Terminal } from "xterm";

const term = new Terminal();
term.open(document.getElementById("terminal")!);

// Using window to inject funcalls to terminal
declare global {
  interface Window {
    log: (s: string) => void;
    err: (s: string) => void;
  }
}

window.log = function (s: string) {
  term.writeln(`[WASM32 STDOUT] ${s}`);
};

window.err = function (s: string) {
  term.writeln(`[WASM32 STDERR] ${s}`);
};

term.writeln("Loading wasm32-unknown-unknown version of demo...");
import init, { wasm_demo } from "../pkg/wasm_demo.js";
init().then(() => {
  wasm_demo(99, 99);

  term.writeln("wasm32-unknown-unknown demo completed.");
});
