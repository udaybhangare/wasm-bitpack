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
- Phase 4: `compress_sorted`/`compress_strictly_sorted` and their `decompress_sorted`/
  `decompress_strictly_sorted` counterparts (new `src/sorted.rs`) — a delta-encoded
  convenience layer for monotonic sequences (sorted IDs, timestamps). Wire format: the first
  value is written as a raw 4-byte little-endian header and only `values[1..]` is
  delta-encoded and packed, deliberately, so a sequence of large absolute values with small
  steps between them (e.g. Unix timestamps) doesn't force `num_bits` up to the first value's
  own magnitude — `num_bits_sorted`/`num_bits_strictly_sorted` size only against genuine step
  widths. `compress_sorted` subtracts consecutive differences (duplicates allowed);
  `compress_strictly_sorted` additionally subtracts `1` per step (no duplicates), so a run of
  consecutive integers packs to `0` bits. Encode delta-encodes into fixed `128`-element stack
  buffers (no heap allocation) and delegates each chunk to `pack` — stays scalar-only, same
  non-goal as plain `compress`/`pack` (no SIMD encode path in v0.1 for any variant). Decode's
  key reuse: `decompress_sorted`/`decompress_strictly_sorted` delegate to `unpack` for the
  delta-unpacking step, which transparently runs the existing `Wasm128` SIMD decode fast path
  on `wasm32` + `simd128` unmodified (a delta value unpacks exactly like any other
  fixed-width `u32`) — only the final prefix-sum reconstruction is scalar, since it's an
  inherently sequential dependency chain. Covered by a round-trip proptest suite (new
  sorted/strictly-sorted generators in `testing/gen.rs`, bounded so generated test sequences
  can never overflow `u32` regardless of how wide a bit-width gets drawn) across bit-widths
  1..=32, full unit/boundary/panic coverage, and doctests; verified clean under `cargo
  +nightly miri test` and `cargo test --target wasm32-wasip1` (existing Phase 1/2 equivalence
  tests unaffected — no changes to the plain `BitPacker` trait surface). Added a native
  `decode_native_sorted` criterion bench group (`benches/decode_native.rs`) comparing
  `decompress_sorted` against plain `unpack` on a synthetic large-timestamp/small-step
  sequence, demonstrating the effective-bit-width win this module exists for (31 bits down to
  4, in that bench's fixed example).
- Phase 4b: rewrote the packed byte format and the `Wasm128` decode kernel to close the
  Phase 3 performance gap, per `plans/decisions/0010-bp128-style-packed-format.md`. Root
  cause: the Phase 2 kernel paired *sequentially adjacent* values into one `i64x2` register,
  which forced a per-lane-divergent shift (adjacent values generally sit at different
  intra-byte bit offsets) that WASM SIMD128 has no instruction for — faked instead with an
  `i64x2_mul` (a genuinely expensive 64x64->64 multiply) recomputed every iteration, on top of
  two scalar loads and two scalar stores per 2 values with no load reuse. Fix: adopted the
  classic Lemire BP128 scheme `bitpacking::BitPacker4x` itself uses — `Scalar`'s packed format
  for `BLOCK_LEN`-exact blocks now splits each 128-value block into 4 stride-4 sub-streams
  (lane `j` holds values `j, j+4, ..., j+124`), bit-packs each lane independently via the
  existing `bitpack_into`/`bitunpack_into` primitives, and interleaves the 4 lanes' packed
  32-bit words round-robin (new `bitpack_interleaved_into`/`bitunpack_interleaved_into` in
  `scalar.rs`, checked against an independently-built `oracle_encode_interleaved`/
  `oracle_decode_interleaved` in `testing/oracle.rs`, plus a hand-verified known-layout test).
  Total packed size is unchanged (`packed_len_bytes(128, num_bits)`), and the non-block-exact
  `pack`/`unpack` tail path is untouched (still the old flat format via `bitpack_into`/
  `bitunpack_into` directly, per `plans/decisions/0006-partial-block-api.md`). Because all 4
  lanes now stay in phase (same relative bit position within their own sub-stream at every
  step), `wasm128/decode_macros.rs` was rewritten to use WASM's native runtime-operand shifts
  (`u32x4_shr`/`u32x4_shl`) directly — no multiply-as-shift trick, and loads/stores go straight
  against the caller's slices with no zero-padded scratch buffer (every load is provably
  in-bounds from `compressed.len() >= 16 * num_bits`). `num_bits == 32` is now a trivial
  hand-written copy loop (no shift/mask/straddle needed at all), matching `bitpacking`'s own
  special-casing. `wasm128/mod.rs`'s Scalar-vs-Wasm128 equivalence matrix (fixed patterns +
  200-case proptest) needed no structural changes and passes unmodified against the new
  format; `sorted.rs`'s delta/prefix-sum layer needed zero code changes, confirmed by its
  existing proptest suite passing natively and under `cargo test --target wasm32-wasip1` with
  no edits — the new kernel emits `decompressed[]` in natural sequential order by construction,
  exactly what the prefix-sum reconstruction needs. Verified clean under `cargo test` (host),
  `cargo +nightly miri test`, `cargo test --target wasm32-wasip1`, `cargo fmt --all -- --check`,
  and `cargo clippy --workspace --all-targets --features bench-support -- -D warnings`.
  **Performance result: closes most of the Phase 3 gap, but the ≥3x claim
  (`plans/00-overview.md` §6) is still not met.** Measured at steady state (10,000,000
  elements, both `wasm-bitpack` and `bitpacking` compiled with identical `+simd128`
  `RUSTFLAGS`, two independent `cargo xtask bench-all` runs): median ratio vs
  `bitpacking::BitPacker4x` improved from Phase 3's 0.41x to **0.62x-0.66x** (run 1: median
  0.66x, range 0.55x-0.98x; run 2: median 0.62x, range 0.49x-0.94x — see
  `plans/results/2026-08-09-decode-throughput.md` and the `-run1` backup of the first run,
  taken before the same-dated file was overwritten). That's roughly a 50-60% throughput
  improvement over the Phase 3 kernel at the same steady-state size, reproducible across both
  runs within normal wall-clock noise, and the best-performing points (32-bit width, ~0.86x-
  0.98x) come close to parity with `bitpacking` — but no bit-width/pattern combination in
  either run reached 1x, let alone 3x. This matches the risk flagged when this phase was
  planned: `bitpacking`'s own `wasm32` fallback already runs structurally the same BP128
  algorithm once LLVM autovectorizes it under `+simd128`, so this phase closed a
  self-inflicted inefficiency rather than introducing a fundamentally faster algorithm than
  the competitor. Reported honestly per `plans/05-benchmarking-strategy.md` §6's hard rule;
  closing the remaining gap (e.g. full compile-time unrolling of all 32 rows, mirroring
  `bitpacking`'s own `crunchy::unroll!`, or `wasm-opt` post-processing) is left as a
  separately-scoped follow-up, not retrofitted into this phase.
