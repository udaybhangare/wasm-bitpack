//! Native x86_64 `criterion` baseline — sanity check only, **not** the headline claim (see
//! `plans/05-benchmarking-strategy.md` §1 row 1: this is bench #1 of 3). It exists to show
//! there's no hidden native regression, nothing more.
//!
//! The real comparison is `cargo xtask bench-wasm`, under `wasm32-wasip1` + `wasmtime` (row
//! 2 of the same table). [`Wasm128`](wasm_bitpack::Wasm128)'s `BitPacker` impl doesn't even
//! exist on this native target — it's `cfg`-gated to `wasm32` + `simd128` — so only this
//! crate's own [`Scalar`](wasm_bitpack::Scalar), `bitpacking::BitPacker4x`, and
//! `stream-vbyte`'s `Scalar` codec are compared here. Build/run with
//! `cargo bench --features bench-support` (the feature reuses this crate's own
//! `testing::bench_gen` input generators — see `plans/04-testing-strategy.md` §9 — instead
//! of a second, potentially-drifting copy).

use bitpacking::{BitPacker as BitPackerTrait, BitPacker4x};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use stream_vbyte::decode::decode;
use stream_vbyte::encode::encode;
use stream_vbyte::scalar::Scalar as StreamVByteScalar;
use wasm_bitpack::bench_support::{BenchPattern, BENCH_BIT_WIDTHS, BENCH_SEED};
use wasm_bitpack::{
    compress_sorted, decompress_sorted, num_bits_sorted, pack, unpack, BitPacker, Scalar,
};

/// `wasm_bitpack::Scalar` vs `bitpacking::BitPacker4x` — both fixed-128-block bit-packing,
/// directly comparable bit-width for bit-width.
fn bench_fixed_width_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_native_fixed_width");
    let block_len = Scalar::BLOCK_LEN;

    for &num_bits in &BENCH_BIT_WIDTHS {
        let values = BenchPattern::Random.generate(BENCH_SEED, num_bits, block_len);
        group.throughput(Throughput::Elements(block_len as u64));

        let mut compressed = vec![0u8; block_len * 4];
        let written = Scalar::compress(&values, &mut compressed, num_bits);
        let compressed = &compressed[..written];
        let mut out = vec![0u32; block_len];
        group.bench_with_input(
            BenchmarkId::new("wasm_bitpack_scalar", num_bits),
            &num_bits,
            |b, &num_bits| {
                b.iter(|| {
                    Scalar::decompress(black_box(compressed), black_box(&mut out), num_bits);
                    black_box(&out);
                });
            },
        );

        let bp = BitPacker4x::new();
        let mut bp_compressed = vec![0u8; BitPacker4x::compressed_block_size(num_bits)];
        let bp_written = bp.compress(&values, &mut bp_compressed, num_bits);
        let bp_compressed = &bp_compressed[..bp_written];
        let mut bp_out = vec![0u32; BitPacker4x::BLOCK_LEN];
        group.bench_with_input(
            BenchmarkId::new("bitpacking_bitpacker4x", num_bits),
            &num_bits,
            |b, &num_bits| {
                b.iter(|| {
                    bp.decompress(black_box(bp_compressed), black_box(&mut bp_out), num_bits);
                    black_box(&bp_out);
                });
            },
        );
    }
    group.finish();
}

/// `stream-vbyte` is a different codec family (variable-byte groups of 4, not fixed-width
/// bit-packing) — included for broader context per
/// `plans/05-benchmarking-strategy.md` §3, not a block-for-block comparison, and not part of
/// the headline `bitpacking` claim.
fn bench_stream_vbyte(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_native_streamvbyte");
    let len = 10_000usize;
    let values = BenchPattern::Random.generate(BENCH_SEED, 20, len);

    let mut encoded = vec![0u8; 5 * len];
    let encoded_len = encode::<StreamVByteScalar>(&values, &mut encoded);
    let encoded = &encoded[..encoded_len];
    let mut out = vec![0u32; len];

    group.throughput(Throughput::Elements(len as u64));
    group.bench_function("stream_vbyte_scalar", |b| {
        b.iter(|| {
            decode::<StreamVByteScalar>(black_box(encoded), len, black_box(&mut out));
            black_box(&out);
        });
    });
    group.finish();
}

/// Phase 4's delta-encoded variant, decode side: `decompress_sorted` vs. plain `unpack` on
/// the *same* logical values — a synthetic "large timestamps, small steps" sequence
/// (deterministic, no RNG needed), the realistic case `plans/05-benchmarking-strategy.md`
/// §4's `delta_friendly`/`sorted_ascending` patterns exist for and the one
/// `crates/wasm-bitpack/src/sorted.rs`'s module docs describe: `Scalar::num_bits` alone must
/// cover the full magnitude of the absolute values (here, ~31 bits), while
/// `num_bits_sorted` only has to cover the step sizes between them (here, 4 bits) — smaller
/// packed input, same decode kernel underneath (`decompress_sorted` delegates to `unpack`
/// for the delta-unpacking step, see the module docs). Native sanity check only, per this
/// file's module doc — not the headline wasm32 claim.
fn bench_sorted_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_native_sorted");
    let len = 10 * Scalar::BLOCK_LEN;

    // Unix-timestamp-scale base, deterministic small steps (always in 2..=14, never
    // wrapping) — unlike `BenchPattern::SortedAscending`, which wraps at the bit-width's
    // max value and would violate `compress_sorted`'s non-decreasing precondition here.
    const BASE: u32 = 1_700_000_000;
    let values: Vec<u32> = (0..len as u32).map(|i| BASE + i * 8 + (i % 7)).collect();

    let plain_num_bits = Scalar::num_bits(&values);
    let sorted_num_bits = num_bits_sorted(&values);
    println!(
        "decode_native_sorted: len={len} plain_num_bits={plain_num_bits} \
         sorted_num_bits={sorted_num_bits}"
    );

    let mut plain_compressed = vec![0u8; len * 4 + 8];
    let plain_written = pack(&values, &mut plain_compressed, plain_num_bits);
    let plain_compressed = &plain_compressed[..plain_written];
    let mut plain_out = vec![0u32; len];

    let mut sorted_compressed = vec![0u8; len * 4 + 8];
    let sorted_written = compress_sorted(&values, &mut sorted_compressed, sorted_num_bits);
    let sorted_compressed = &sorted_compressed[..sorted_written];
    let mut sorted_out = vec![0u32; len];

    group.throughput(Throughput::Elements(len as u64));
    group.bench_function("plain_unpack", |b| {
        b.iter(|| {
            unpack(
                black_box(plain_compressed),
                black_box(&mut plain_out),
                plain_num_bits,
            );
            black_box(&plain_out);
        });
    });
    group.bench_function("decompress_sorted", |b| {
        b.iter(|| {
            decompress_sorted(
                black_box(sorted_compressed),
                black_box(&mut sorted_out),
                sorted_num_bits,
            );
            black_box(&sorted_out);
        });
    });
    group.finish();
}

criterion_group! {
    name = benches;
    // Shorter than criterion's defaults (sample_size 100, measurement_time 5s): this bench
    // is an explicitly secondary sanity baseline (see the module doc), not the headline
    // claim, and the full bit-width matrix across three competitors otherwise takes
    // several minutes for numbers nobody's shipping a claim on. `sample_size(10)` is
    // criterion's own documented minimum.
    config = Criterion::default()
        .sample_size(10)
        .measurement_time(std::time::Duration::from_millis(500))
        .warm_up_time(std::time::Duration::from_millis(300));
    targets = bench_fixed_width_decode, bench_stream_vbyte, bench_sorted_decode
}
criterion_main!(benches);
