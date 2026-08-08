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
