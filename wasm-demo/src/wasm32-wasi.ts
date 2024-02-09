import { Terminal } from "xterm";
import {
  WASI,
  File,
  OpenFile,
  ConsoleStdout,
  PreopenDirectory,
} from "@bjorn3/browser_wasi_shim";

const term = new Terminal();
term.open(document.getElementById("terminal")!);
term.writeln("Loading wasm32-wasi version of demo...");

let args: string[] = [];
// doesn't do much, as there is no rust backtrace
let env = ["RUST_BACKTRACE=full"];
let fds = [
  // stdin
  new OpenFile(new File([])),
  // stdout
  ConsoleStdout.lineBuffered((msg) => term.writeln(`[WASI stdout] ${msg}`)),
  // stderr
  ConsoleStdout.lineBuffered((msg) => term.writeln(`[WASI stderr] ${msg}`)),
  // working directory
  new PreopenDirectory(".", {}),
];

let wasi = new WASI(args, env, fds);

let wasm = await WebAssembly.compileStreaming(
  fetch("./dist/demo.wasm", { cache: "no-store" }),
);
let inst = await WebAssembly.instantiate(wasm, {
  wasi_snapshot_preview1: wasi.wasiImport,
});

// For some reason this is incompatible
wasi.start(inst as any);

term.writeln("wasm32-wasi demo completed.");
