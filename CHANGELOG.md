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
