//! Standalone `wasm32-wasip1` timing binary for `wasm_bitpack::Scalar::decompress` — one leg
//! of the three-way `bitpacking`/`stream-vbyte`/`wasm_bitpack` comparison in
//! `plans/05-benchmarking-strategy.md` §3, isolating "how much did `simd128` actually buy
//! us" (compare against `wasm_wasm128`'s numbers) separately from "how much faster are we
//! than the competition" (compare against `wasm_bitpacking`'s numbers).
//!
//! Not runnable directly with meaningful output on any other target — see `cargo xtask
//! bench-wasm`, which builds and invokes this (and its three siblings in this directory)
//! under `wasmtime` with `<num_bits> <pattern>` arguments, looping internally over
//! `bench_support::BENCH_SIZES` to amortize `wasmtime` process-startup cost across the size
//! sweep.

// This benchmark binary is dev-only tooling, never built for a downstream library consumer,
// so it isn't bound by the crate's `rust-version` MSRV promise — `std::hint::black_box`
// (stable since 1.66) is fine here even though the library itself targets 1.64.0.
#![allow(clippy::incompatible_msrv)]

#[path = "common.rs"]
mod common;

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bitpack::bench_support::{BENCH_SEED, BENCH_SIZES};
    use wasm_bitpack::Scalar;

    let (num_bits, pattern) = common::parse_args();

    for &size in &BENCH_SIZES {
        let values = pattern.generate(BENCH_SEED, num_bits, size);
        let compressed = common::compress_blocks::<Scalar>(&values, num_bits);
        let mut out = vec![0u32; size];

        let (median_ns, stddev_ns) = common::time_calls(|| {
            common::decode_blocks::<Scalar>(
                std::hint::black_box(&compressed),
                std::hint::black_box(&mut out),
                num_bits,
            );
        });

        common::report(
            "wasm_bitpack_scalar",
            num_bits,
            pattern.name(),
            size,
            median_ns,
            stddev_ns,
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("this benchmark binary only runs on wasm32 — see `cargo xtask bench-wasm`");
}
