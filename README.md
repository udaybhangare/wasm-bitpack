# wasm-bitpack

[![CI](https://github.com/udaybhangare/wasm-bitpack/actions/workflows/ci.yml/badge.svg)](https://github.com/udaybhangare/wasm-bitpack/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/badge/crates.io-not%20yet%20published-lightgrey)](https://crates.io/crates/wasm-bitpack)
[![docs.rs](https://img.shields.io/badge/docs.rs-not%20yet%20published-lightgrey)](https://docs.rs/wasm-bitpack)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

A SIMD-accelerated integer bit-packing library for Rust — with a real WebAssembly SIMD128
decode fast path, which no existing crate in this space has.

## The pitch

Every popular Rust integer-compression crate (`bitpacking`, `stream-vbyte`, `FastPFOR-rs`)
gets its speed from x86 AVX2/SSE intrinsics and silently falls back to plain scalar code
when compiled to `wasm32` — this crate implements the missing WebAssembly SIMD128 decode
fast path, so it wins the benchmark by default the moment either crate is used in a
browser, a Cloudflare Worker, or any other `wasm32` target.

## The problem

Rust's fast integer-compression crates are written against
`#[target_feature(enable = "avx2")]` / `"sse3"` — x86-only CPU features. When you compile
any of them to `wasm32-unknown-unknown` (e.g. to run compression inside a browser tab, an
edge function, or a WASM plugin host), that `cfg(target_arch = "x86_64")` gate fails and
the crate quietly drops to its scalar reference implementation. The crate still *works* —
it's just no longer doing the thing it's known for.

WebAssembly has had its own 128-bit SIMD instruction set (`v128`, exposed in Rust via
`core::arch::wasm32` + `#[target_feature(enable = "simd128")]`) stable in V8, SpiderMonkey,
and `wasmtime` since 2021. This crate hand-writes the bit-unpacking kernels against it.

Scoping honesty: this crate is **not** trying to beat `bitpacking` on native x86_64 — its
AVX2 path will likely stay faster there, and that's fine. The claim is narrow and provable:
*fastest bit-unpacking crate when compiled to `wasm32`*, benchmarked against the same
crates running on the same target, with a public reproduction script.

## Who this is for

| Audience | Use case |
|---|---|
| Browser-side analytics/observability tooling | Compress numeric columns (metric values, timestamps, IDs) before IndexedDB storage or wire transfer from a WASM-compiled app |
| Edge compute (Cloudflare Workers, Fastly Compute@Edge, other `wasm32` hosts) | Numeric/log/time-series processing under `wasm32` constraints |
| WASM plugin hosts | Cheaply moving packed integer arrays across the host/guest WASM boundary |

## Scope

- **In scope**: a portable scalar reference codec, and a hand-written `wasm32` + `simd128`
  **decode** (unpack) fast path, byte-for-byte equivalent to the scalar path for every
  supported bit-width (1–32 bits per value).
- **Out of scope**: native x86 AVX2/SSE or ARM NEON fast paths (use `bitpacking` directly
  on native x86_64), and arbitrary bit-widths beyond packing `u32` values.
- **Decode before encode**: decompression is the hotter path in most real usage (query
  engines, log readers, analytics dashboards decode far more often than they encode).
  Encode ships scalar-only initially.

## Project status

Early scaffold stage — see `CHANGELOG.md` for what's landed so far. Not yet published to
crates.io.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in this crate by you, as defined in the Apache-2.0 license, shall be dual
licensed as above, without any additional terms or conditions.
