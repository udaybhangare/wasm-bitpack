//! Standalone `wasm32-wasip1` timing binary for `stream-vbyte`'s `Scalar` codec — broader
//! context per `plans/05-benchmarking-strategy.md` §3, **not** a block-for-block comparison
//! (variable-byte groups of 4, not fixed-128-block bit-packing) and not part of the headline
//! ≥3x-vs-`bitpacking` claim. `stream-vbyte` has no `wasm32` SIMD path at all (its SIMD
//! variants are `feature(portable_simd)`-gated x86 intrinsics behind non-default Cargo
//! features, nightly-only) — only its portable `Scalar` codec is exercised here, which is
//! the honest, fair comparison: it's also the only codec this crate offers on `wasm32`
//! that's actually shipped by default.

// This benchmark binary is dev-only tooling, never built for a downstream library consumer,
// so it isn't bound by the crate's `rust-version` MSRV promise — `std::hint::black_box`
// (stable since 1.66) is fine here even though the library itself targets 1.64.0.
#![allow(clippy::incompatible_msrv)]

#[path = "common.rs"]
mod common;

#[cfg(target_arch = "wasm32")]
fn main() {
    use stream_vbyte::decode::decode;
    use stream_vbyte::encode::encode;
    use stream_vbyte::scalar::Scalar as StreamVByteScalar;
    use wasm_bitpack::bench_support::{BENCH_SEED, BENCH_SIZES};

    let (num_bits, pattern) = common::parse_args();

    for &size in &BENCH_SIZES {
        let values = pattern.generate(BENCH_SEED, num_bits, size);

        let mut encoded = vec![0u8; 5 * size];
        let encoded_len = encode::<StreamVByteScalar>(&values, &mut encoded);
        encoded.truncate(encoded_len);

        let mut out = vec![0u32; size];
        let (median_ns, stddev_ns) = common::time_calls(|| {
            decode::<StreamVByteScalar>(
                std::hint::black_box(&encoded),
                size,
                std::hint::black_box(&mut out),
            );
            std::hint::black_box(&out);
        });

        common::report(
            "stream_vbyte_scalar",
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
