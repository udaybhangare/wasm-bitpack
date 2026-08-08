# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Cargo's semver conventions](https://doc.rust-lang.org/cargo/reference/semver.html)
(see `plans/06-versioning-and-release.md` for the project-specific clarifications).

## [Unreleased]

### Added

- Initial workspace scaffold: Cargo workspace (`crates/wasm-bitpack`, `xtask`), crate
  module stubs (`BitPacker` trait, `Scalar`, `Wasm128`, `best_available`, `pack`/`unpack`)
  with `todo!()` bodies, `xtask ci` automation, `wasm32-wasip1` + `wasmtime` test runner,
  CI workflow, licensing, and contributor docs. No codec logic yet.
- MSRV resolved and pinned: Rust 1.64.0 (see `plans/decisions/0008-msrv.md`).
- `.gitattributes` forcing `eol=lf` on checkout, fixing `fmt (windows-latest)` in CI
  (Windows runners otherwise convert the repo's LF endings to CRLF on checkout, which
  fails `rustfmt`'s `newline_style = "Unix"` check).
- Phase 0 premise validation: confirmed `bitpacking::BitPacker4x` never runs its hand-written
  SIMD fast path on `wasm32` (source-confirmed `cfg(target_arch = "x86_64"/"aarch64")` gating)
  and is 3.7-4.4x slower there than native by default. Also surfaced an important nuance for
  Phase 3's benchmark methodology: with `RUSTFLAGS="-C target-feature=+simd128"` explicitly
  enabled, LLVM autovectorizes `bitpacking`'s plain scalar fallback into real `v128` code,
  closing most (not all) of the gap to native — see `plans/results/phase0-premise-validation.md`.
- Phase 1: real `Scalar` codec (LSB-first tight bit-packing, `num_bits` 1..=32, panics on
  precondition violations per the locked API's error policy), the arbitrary-length
  `pack`/`unpack` wrapper over the block-exact `BitPacker` trait (full blocks dispatch
  generically, the trailing partial block reuses `Scalar`'s own bit-twiddling directly, per
  `plans/decisions/0006-partial-block-api.md`), the naive bit-by-bit proptest oracle, and
  full unit/property/integration/doctest coverage. `Wasm128` remains the unimplemented
  Phase -1 stub; `best_available()` resolves to `Scalar` unconditionally until Phase 2.
  Verified clean under `cargo +nightly miri test`, with property-test file-based failure
  persistence disabled and case counts scaled down under `cfg(miri)` (miri has no
  filesystem/cwd access by default, and is orders of magnitude slower than native).
- Phase 2: the hand-written `wasm32` + `simd128` SIMD decode fast path. `wasm128/decode_macros.rs`
  macro-generates all 32 per-bit-width `decode_{n}bit` routines from one template — each
  decodes a pair of values at a time with a 2-lane `i64x2` register, using
  `v128_load64_zero`/`v128_load64_lane` to read an 8-byte window per value from a local
  zero-padded stack buffer, `i64x2_mul` by a per-lane power-of-two multiplier to emulate a
  per-lane variable left shift (`core::arch::wasm32` has no such instruction), a uniform
  `u64x2_shr`, and a defensive `v128_and` mask. `Wasm128::decompress` is now real and
  byte-for-byte equivalent to `Scalar::decompress` by construction (proved by a new
  Scalar-vs-Wasm128 equivalence matrix: every bit-width 1..=32 × {zeros, max-value, random,
  sorted, alternating} patterns, plus a proptest sweep, run genuinely under `wasmtime` via
  `cargo test --target wasm32-wasip1`). `Wasm128::num_bits`/`compress` delegate to `Scalar`
  (encode stays scalar-only in v0.1; delegating rather than panicking keeps `pack()`
  functional on `wasm32` + `simd128`, where it picks `Wasm128` as its full-block codec).
  `best_available()` now genuinely resolves to `Wasm128` on that target. Added
  `-C target-feature=+simd128` to `.cargo/config.toml`'s `rustflags` for
  `wasm32-unknown-unknown`/`wasm32-wasip1` so the `simd128`-gated code path actually compiles
  under the plain `cargo build --target wasm32-unknown-unknown`/`cargo test --target
  wasm32-wasip1` invocations, instead of silently staying dead code. `cargo +nightly miri
  test` remains clean (`wasm128/` stays `cfg`'d out on miri's host target, by design).
- Phase 3: the benchmark harness and the project's first real, reproducible throughput
  measurement. `cargo xtask bench-all` runs a native `criterion` sanity baseline
  (`benches/decode_native.rs`; `Wasm128` doesn't exist on that target, so this only compares
  `Scalar`/`bitpacking`/`stream-vbyte`) and the real comparison — four standalone
  `Instant`-based timing binaries (`benches/wasm/*.rs`) cross-compiled to `wasm32-wasip1` and
  run under `wasmtime`, covering 9 bit-widths × 3 patterns (random/sorted/delta-friendly) × 4
  log-scaled sizes (128 to 10,000,000 elements) × 4 competitors, 100 timed iterations per
  point — then writes a dated snapshot to `plans/results/` and regenerates the root
  `BENCHMARKS.md`. Added `testing/bench_gen.rs`, a dependency-free generator module (fixed
  seed `0x2545_F491`) shared by the equivalence matrix and the benches, exposed to
  `benches/` targets through a new `bench-support` Cargo feature (`#[doc(hidden)]`, off by
  default, not part of the locked v0.1 public API — see its doc comment for why it's safe to
  gate a dev-only feature behind something that needs `dev-dependencies`).
  **Result: the ≥3x decode-throughput claim (`plans/00-overview.md` §6) is not met.**
  Measured at steady state (10,000,000 elements, both `wasm-bitpack` and `bitpacking`
  compiled with the identical `+simd128` `RUSTFLAGS`): `Wasm128` beats this crate's own
  `Scalar` path in almost every case (up to ~16x at wide bit-widths), but trails
  `bitpacking::BitPacker4x`'s LLVM-autovectorized `simd128` fallback — median ratio 0.41x
  (two independent runs: 0.50x and 0.41x, range 0.17x-1.18x combined) across the full
  bit-width × pattern matrix, never approaching 1x let alone 3x — see `BENCHMARKS.md` for
  the exact numbers and full methodology, including the two-run reproducibility comparison
  (qualitative finding reproduces cleanly; absolute numbers vary up to ~45% run-to-run on
  this non-isolated dev machine, reported honestly rather than smoothed over). This confirms
  the risk Phase 0 flagged in advance (`plans/results/phase0-premise-validation.md` §6): the
  real bar on `wasm32` turned out to be an already-near-native autovectorized baseline, not
  scalar code, and the current 2-values-per-`i64x2`-lane decode kernel doesn't clear it.
  Reported honestly per `plans/05-benchmarking-strategy.md` §6's hard rule rather than
  adjusting methodology to manufacture a passing number; closing the gap (wider SIMD lanes,
  fewer loads per decoded element) is left as a follow-up, not retrofitted into this phase.
  Also added a minimal,
  unpolished `wasm-bindgen` browser demo (`demo/browser/`, a standalone scratch crate outside
  the main workspace) that reproduces the `Scalar`-vs-`Wasm128` comparison live in a browser
  tab with `performance.now()`, and `.github/workflows/benchmarks.yml` (informational,
  `main`-push-only, uploads the dated snapshot as a workflow artifact since `plans/` itself
  is git-ignored).
