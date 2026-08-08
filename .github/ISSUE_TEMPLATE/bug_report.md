---
name: Bug report
about: Report incorrect behavior, a panic, or a compile failure
title: ""
labels: bug
assignees: ""
---

## Describe the bug

A clear description of what's wrong.

## Repro steps

Minimal steps (ideally a minimal code snippet) to reproduce the behavior.

## Expected behavior

What you expected to happen.

## Actual behavior

What actually happened — include panic messages / stack traces / compiler errors verbatim.

## Environment

- Target: [e.g. `x86_64-unknown-linux-gnu`, `wasm32-unknown-unknown`, `wasm32-wasip1`,
  in-browser]
- `rustc --version`:
- `wasm-bitpack` version:
- If wasm32: runtime (`wasmtime --version`, or browser + version)

## Additional context

Anything else relevant (e.g. only reproduces at a specific `num_bits`, only under
`--release`, etc.).
