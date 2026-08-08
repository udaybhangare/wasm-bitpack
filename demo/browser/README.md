# wasm-bitpack browser demo

A minimal, unpolished proof artifact for `plans/05-benchmarking-strategy.md` §1 row 3: a
live, in-browser comparison of `wasm-bitpack`'s `Scalar` vs. hand-written `Wasm128` (SIMD128)
decode path, timed with `performance.now()`. **This is not the headline numeric claim** — that
comes from `cargo xtask bench-wasm` under `wasmtime` (see the root `BENCHMARKS.md`). This page
exists so anyone can watch the SIMD path actually run, in a real browser, with their own eyes.

Deliberately a standalone scratch crate outside the main Cargo workspace (its own
`[workspace]` root, same pattern as Phase 0's premise-validation spike) — not the
general-purpose `wasm-bindgen` JS wrapper package reserved for a future phase at
`crates/wasm-bitpack-js/` (`plans/01-architecture.md` §1). Not published, not CI-gated, no UI
polish intended.

## Build

Requires `wasm-bindgen-cli` matching the `wasm-bindgen` version in `Cargo.toml`:

```sh
cargo install wasm-bindgen-cli --version 0.2.127 --locked
```

Then, from this directory:

```sh
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir pkg \
  target/wasm32-unknown-unknown/release/wasm_bitpack_browser_demo.wasm
```

(`simd128` is already enabled via this directory's own `.cargo/config.toml` — no extra
`RUSTFLAGS` needed, unlike the root crate's benches which rely on the root `.cargo/config.toml`.)

## Run

Serve this directory over HTTP (opening `index.html` directly via `file://` won't work — ES
module imports and `fetch`-based wasm loading both require a real origin):

```sh
python -m http.server 8000
# then open http://localhost:8000/
```

Pick a bit width and block count, click "Run comparison". Both paths' checksums are shown and
compared — a mismatch would mean something is badly wrong (it shouldn't happen; this is the
same `Wasm128`/`Scalar` pair proven byte-for-byte equivalent by the Phase 2 equivalence
matrix, `plans/04-testing-strategy.md` §4).

## Quick correctness check without a browser

`smoke_test.js` exercises the same two bindings under Node instead (Node also has a WASM
engine, so this is a fast way to confirm the build works before opening a real browser):

```sh
wasm-bindgen --target nodejs --out-dir pkg-node-test \
  target/wasm32-unknown-unknown/release/wasm_bitpack_browser_demo.wasm
node smoke_test.js
```
