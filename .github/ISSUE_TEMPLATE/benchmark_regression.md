---
name: Benchmark regression
about: Report a measured performance regression against a previously published snapshot
title: ""
labels: performance
assignees: ""
---

## Which snapshot are you comparing against?

Link or path to the `plans/results/` (or published `BENCHMARKS.md`) snapshot you're using
as the baseline.

## What regressed

The specific benchmark name(s) and the numbers: baseline vs. observed, and the percentage/
multiplier change.

## Environment

- Target: [e.g. `wasm32-wasip1` under `wasmtime`, native `x86_64`, in-browser]
- `rustc --version`:
- If wasm32: `wasmtime --version` (or browser + version)
- Hardware (CPU, since this affects absolute throughput numbers even though the relative
  claim shouldn't move much)

## Reproduction

Exact command(s) run to reproduce the regression (e.g. `cargo xtask bench-all`), and
whether it's reproducible across multiple runs (benchmarks are noisy — please run more than
once before filing).

## Additional context

Anything else relevant (recent changes on your branch, unusual build flags, etc.).
