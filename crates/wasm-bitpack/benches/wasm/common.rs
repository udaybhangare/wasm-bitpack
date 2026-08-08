//! Shared timing/reporting helpers for the `wasm32-wasip1` standalone benchmark binaries
//! (`cargo xtask bench-wasm`, see `plans/05-benchmarking-strategy.md` §2). Not a `[[bench]]`
//! target itself — pulled in via `#[path = "common.rs"] mod common;` from each of the four
//! per-competitor binaries in this directory (separate `[[bench]]` targets are separate
//! crates and can't `use` each other directly).
//!
//! `criterion` has no clean `wasm32-wasip1` story (see the module doc on each binary), so
//! this hand-rolls the same rigor criterion would otherwise provide: warmup, `Instant`-based
//! per-call timing (the WASI clock — validated end-to-end in Phase 0's premise-validation
//! spike, `plans/results/phase0-premise-validation.md` §3-5), and median + stddev over
//! `ITERATIONS` (>= 100 per `plans/05-benchmarking-strategy.md` §5) rather than a
//! single-sample number.

#![allow(dead_code)] // each binary only uses a subset of these helpers

use std::time::Instant;
use wasm_bitpack::bench_support::BenchPattern;
use wasm_bitpack::BitPacker;

/// Timed samples per (competitor, bit-width, pattern, size) point — `plans/05-benchmarking-strategy.md`
/// §5's "no single-sample numbers" rule.
pub const ITERATIONS: usize = 100;
/// Untimed calls before measurement starts, to warm up any JIT/codegen and page faults.
pub const WARMUP_ITERATIONS: usize = 10;

/// Runs `f` `WARMUP_ITERATIONS` times untimed, then `ITERATIONS` times timed with `Instant`.
/// Returns `(median_nanos, stddev_nanos)` over the timed calls.
pub fn time_calls(mut f: impl FnMut()) -> (f64, f64) {
    for _ in 0..WARMUP_ITERATIONS {
        f();
    }

    let mut samples_ns = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        let start = Instant::now();
        f();
        samples_ns.push(start.elapsed().as_nanos() as f64);
    }

    samples_ns.sort_by(|a, b| a.partial_cmp(b).expect("timings are never NaN"));
    let median = samples_ns[samples_ns.len() / 2];
    let mean = samples_ns.iter().sum::<f64>() / samples_ns.len() as f64;
    let variance =
        samples_ns.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / samples_ns.len() as f64;
    let stddev = variance.sqrt();

    (median, stddev)
}

/// Prints one JSON result line to stdout — the structured output `xtask bench-wasm` parses
/// and aggregates. Includes `target_feature_simd128`, this specific binary's own compile-time
/// answer to "was `simd128` actually enabled" (`plans/05-benchmarking-strategy.md` §5's
/// anti-cheating rule: never silently absent) — `xtask` separately prints the exact
/// `rustc`/`wasmtime` versions and the `RUSTFLAGS` used to build these binaries once, at the
/// top of the aggregated results, rather than repeating them on every line.
pub fn report(
    competitor: &str,
    num_bits: u8,
    pattern: &str,
    size: usize,
    median_ns: f64,
    stddev_ns: f64,
) {
    let median_s = median_ns / 1.0e9;
    let elements_per_sec = size as f64 / median_s;
    let bytes_per_sec = (size * 4) as f64 / median_s; // decoded output is u32 = 4 bytes/element

    println!(
        "{{\"competitor\":\"{competitor}\",\"num_bits\":{num_bits},\"pattern\":\"{pattern}\",\
         \"size\":{size},\"iterations\":{ITERATIONS},\"median_ns\":{median_ns:.1},\
         \"stddev_ns\":{stddev_ns:.1},\"elements_per_sec\":{elements_per_sec:.1},\
         \"bytes_per_sec\":{bytes_per_sec:.1},\"target_feature_simd128\":{}}}",
        cfg!(target_feature = "simd128"),
    );
}

/// Parses `argv[1..]` as `<num_bits> <pattern>` — the two axes a single binary invocation
/// covers; the size axis is looped internally (see each binary's `main`) to amortize
/// `wasmtime` process-startup overhead across the size sweep.
pub fn parse_args() -> (u8, BenchPattern) {
    let mut args = std::env::args().skip(1);
    let usage = "usage: <bin> <num_bits> <random|sorted_ascending|delta_friendly>";
    let num_bits: u8 = args
        .next()
        .expect(usage)
        .parse()
        .expect("num_bits must be a u8");
    let pattern = match args.next().expect(usage).as_str() {
        "random" => BenchPattern::Random,
        "sorted_ascending" => BenchPattern::SortedAscending,
        "delta_friendly" => BenchPattern::DeltaFriendly,
        other => panic!("{usage} (got unknown pattern `{other}`)"),
    };
    (num_bits, pattern)
}

/// Packs `values` (length an exact multiple of `B::BLOCK_LEN`, guaranteed by
/// `bench_support::BENCH_SIZES`) into a tightly-packed buffer using `B`'s own
/// [`BitPacker::compress`] directly, one block at a time — not timed, this is setup.
pub fn compress_blocks<B: BitPacker>(values: &[u32], num_bits: u8) -> Vec<u8> {
    let block_len = B::BLOCK_LEN;
    assert_eq!(
        values.len() % block_len,
        0,
        "benchmark sizes must be exact multiples of BLOCK_LEN"
    );
    let mut out = vec![0u8; values.len() * 4]; // generous upper bound
    let mut pos = 0;
    for chunk in values.chunks(block_len) {
        pos += B::compress(chunk, &mut out[pos..], num_bits);
    }
    out.truncate(pos);
    out
}

/// The timed operation for this crate's own codecs: unpacks `out.len() / B::BLOCK_LEN` blocks
/// from `compressed` via `B`'s own [`BitPacker::decompress`] directly (not the arbitrary-length
/// `pack`/`unpack` wrapper, so `wasm128.rs`'s and `scalar.rs`'s binaries genuinely benchmark
/// the specific implementation named in their filename, not whichever `best_available()` would
/// have picked).
pub fn decode_blocks<B: BitPacker>(compressed: &[u8], out: &mut [u32], num_bits: u8) {
    let block_len = B::BLOCK_LEN;
    let mut in_pos = 0;
    for chunk in out.chunks_mut(block_len) {
        in_pos += B::decompress(&compressed[in_pos..], chunk, num_bits);
    }
}
