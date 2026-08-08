//! Standalone `wasm32-wasip1` timing binary for `wasm_bitpack::Wasm128::decompress` — **the
//! headline benchmark**: this crate's hand-written SIMD128 decode path, compared against
//! `wasm_scalar` (this crate's own scalar path, isolating the `simd128` delta) and
//! `wasm_bitpacking` (the ≥3x claim from `plans/00-overview.md` §6). See
//! `plans/05-benchmarking-strategy.md` §1-3.
//!
//! `main` only exists under `wasm32` + `simd128` — [`Wasm128`](wasm_bitpack::Wasm128)'s
//! `BitPacker` impl is `cfg`-gated identically, so this binary genuinely cannot compile
//! (let alone run) without it, which is exactly the property that makes this the real
//! comparison and not a silently-scalar-fallback one.

// This benchmark binary is dev-only tooling, never built for a downstream library consumer,
// so it isn't bound by the crate's `rust-version` MSRV promise — `std::hint::black_box`
// (stable since 1.66) is fine here even though the library itself targets 1.64.0.
#![allow(clippy::incompatible_msrv)]

#[path = "common.rs"]
mod common;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
fn main() {
    use wasm_bitpack::bench_support::{BENCH_SEED, BENCH_SIZES};
    use wasm_bitpack::Wasm128;

    let (num_bits, pattern) = common::parse_args();

    for &size in &BENCH_SIZES {
        let values = pattern.generate(BENCH_SEED, num_bits, size);
        let compressed = common::compress_blocks::<Wasm128>(&values, num_bits);
        let mut out = vec![0u32; size];

        let (median_ns, stddev_ns) = common::time_calls(|| {
            common::decode_blocks::<Wasm128>(
                std::hint::black_box(&compressed),
                std::hint::black_box(&mut out),
                num_bits,
            );
        });

        common::report(
            "wasm_bitpack_wasm128",
            num_bits,
            pattern.name(),
            size,
            median_ns,
            stddev_ns,
        );
    }
}

#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
fn main() {
    eprintln!("this benchmark binary only runs on wasm32 + simd128 — see `cargo xtask bench-wasm`");
}
