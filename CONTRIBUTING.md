# Contributing to wasm-bitpack

Thanks for taking a look at this project. It's a solo-maintained crate, but it's set up
and reviewed as if contributors matter — that's part of the point.

## Before you open a PR

Every PR must pass, locally, before requesting review/merge:

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace` (or `cargo xtask ci`, which runs the three checks above
      together — the same checks CI runs, so there's no drift between local and CI)
- [ ] Every new `pub` item has a `///` doc comment (enforced by `#![deny(missing_docs)]`
      anyway, but call it out)
- [ ] Every new `unsafe` block has a `// SAFETY:` comment explaining why its preconditions
      hold at that call site — and if it touches `wasm128/`, treat this as the primary
      review focus (see "`unsafe` policy" below)
- [ ] `CHANGELOG.md`'s `[Unreleased]` section has an entry, if the change is user-visible

## Local dev setup

### Toolchain

`rust-toolchain.toml` at the repo root pins the toolchain and targets automatically —
`rustup` will install what's needed the first time you run `cargo` in this repo. That
includes the `wasm32-unknown-unknown` and `wasm32-wasip1` targets and the `clippy`/
`rustfmt` components.

Miri needs a separate `nightly` toolchain (miri isn't shipped on stable), installed once
with:

```sh
rustup toolchain install nightly --component miri
```

Running the wasm32 test suite (see below) also requires
[`wasmtime`](https://wasmtime.dev/) installed and on `PATH`.

### Formatting and linting

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

`clippy::pedantic` is enabled crate-wide as a `warn`, not a `deny`, so it doesn't block
local iteration — but combined with `-D warnings` in CI, every pedantic lint hit becomes a
hard failure. If you genuinely need a new `#[allow(...)]`, justify it in the PR
description; the crate's existing allow-list (in `crates/wasm-bitpack/src/lib.rs`) is kept
short on purpose.

### Running the test suite

- **Native**: `cargo test --workspace`.
- **`wasm32` under `wasmtime`**: `cargo test --target wasm32-wasip1 -p wasm-bitpack`. This
  is wired up via a `.cargo/config.toml` runner entry that invokes `wasmtime run` under the
  hood — it's not a build-only check, it genuinely executes the compiled `.wasm` test
  binary and reports real pass/fail results.
- **Miri** (undefined-behavior detection, native/non-SIMD code only):
  `cargo +nightly miri test -p wasm-bitpack`. Miri interprets against the host target, so it
  cannot execute `wasm128/` (that module is `cfg`-gated to `wasm32` and doesn't exist in
  miri's host-target build) — its coverage is `Scalar`, `pack`/`unpack`, and any other
  non-SIMD code. This is a known, accepted limitation, not an oversight.
- **Everything at once, matching CI**: `cargo xtask ci`.

### `unsafe` policy

This crate hand-writes WebAssembly SIMD128 intrinsics — `unsafe` is unavoidable in
`wasm128/`, but nowhere else in the crate:

- `#![deny(unsafe_code)]` at the crate root (`crates/wasm-bitpack/src/lib.rs`) means
  `unsafe` appearing anywhere outside `wasm128/mod.rs` fails to compile. This is a
  compiler-enforced boundary, not a review convention.
- Every `unsafe fn` needs a `# Safety` doc section stating the exact preconditions the
  caller must uphold.
- Every `unsafe` block needs a `// SAFETY:` comment directly above it, explaining *why*
  those preconditions are actually upheld at that specific call site — not a restatement of
  the function's `# Safety` doc, but the concrete justification.
- No PR touching `wasm128/` merges without every new `unsafe` block's `SAFETY:` comment
  reviewed for actual correctness, not just presence.

## Commit convention

[Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`,
`test:`, `bench:`, `chore:`) are recommended for clarity, but not enforced by CI — a nice
habit, not a merge gate.

## Legal

No formal CLA. An optional DCO-style sign-off (`git commit -s`) is encouraged but not
required — the dual MIT/Apache-2.0 license already covers the permissive-contribution
intent.

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By participating,
you're expected to uphold it.

## Issue and PR templates

Opening an issue or PR on GitHub will offer you a template:

- **Bug report** — repro steps, expected vs. actual behavior, target (native/wasm32/
  browser), toolchain versions.
- **Feature request** — problem statement, proposed API shape if relevant.
- **Benchmark regression** — a dedicated template for reporting a performance regression
  against a previously published benchmark snapshot, since benchmark integrity is core to
  this project's claim.
- **Pull request** — mirrors the "Before you open a PR" checklist above, plus a "what does
  this change and why" prompt.
