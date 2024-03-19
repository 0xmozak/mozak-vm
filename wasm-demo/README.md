# WASM DEMO

This demo tries to run Mozak-VM and its proof system on WASM. For now it just tries to execute single instruction of ADD.
The execution and proving works :sparkles:.

## Installing dependencies

This test suite depnds on

- `node` (`v20.11.0`),
- `rust`, (version in `../rust-toolchain.toml`),
- `wasmtime`,
- `playwright`.

In order to run tests you need to install `playwright` browsers by running

```bash
npx playwright install --with-deps
```

which will install playwright and all its associated runners needed to run the browsers.

## First build

If you checked out the project for the first time run

```bash
npm ci
```

which will

- install all the dependencies,
- run `wasm-pack` to build `wasm32-unknown-unknown` target,
- run `cargo` to build `wasm32-wasi` target, and
- compile all the TypeScript files and bundle them in their respective JavaScript files.

All final build artifacts will be placed in `./dist` directory.

## Automated Testing

You can run the Wasmtime test suite by using

```bash
npm test
```

which will
- test `wasm32-wasi` in `wasmtime`.

And you can run the Playwright test suite by using

```bash
npm run test-slow
```

which will

- test `wasm32-unknown-unknown` in 3 major browsers, and
- test `wasm32-wasi` in 3 major browsers.

Note that upon failure playwright will open the report.  You can manually reopen the report by running

```bash
npx playwright show-report
```

## Rebuild

You can rebuild everything using

```bash
npm run prepare
```

which will call

- `wasm-pack build --target web`,
- `cargo build --target wasm32-wasi`, and
- `webpack`.

## Opening in browser

You can start a local server with all caches disabled on `localhost:3000` by using

```bash
npm start
```

There are two test cases

- [`http://localhost:3000/wasm32-unknown-unknown.html`](http://localhost:3000/wasm32-unknown-unknown.html) for testing `wasm32-unkown-unknown` build, and
- [`http://localhost:3000/wasm32-wasi.html`](http://localhost:3000/wasm32-wasi.html) for testing `wasm32-wasi` build.
