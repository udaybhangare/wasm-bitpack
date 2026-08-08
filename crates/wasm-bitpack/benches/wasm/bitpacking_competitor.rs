//! Standalone `wasm32-wasip1` timing binary for `bitpacking::BitPacker4x::decompress` — the
//! direct competitor the ≥3x claim (`plans/00-overview.md` §6) is measured against. Same
//! 128-`u32` block length as this crate, so bit-width-for-bit-width numbers are directly
//! comparable to `wasm_wasm128`'s.
//!
//! Compiled with the exact same `RUSTFLAGS`/profile as this crate's own binaries (see
//! `.cargo/config.toml` and `plans/05-benchmarking-strategy.md` §5's anti-cheating rules —
//! no selectively hobbling the competitor). Per Phase 0's premise-validation spike
//! (`plans/results/phase0-premise-validation.md` §6), `bitpacking`'s hand-written SSE3/NEON
//! intrinsics never compile in on `wasm32` at all (`cfg(target_arch = "x86_64"/"aarch64")`),
//! but with `simd128` enabled its plain-array scalar fallback still gets LLVM
//! auto-vectorized to real `v128` code — this binary measures whatever `bitpacking`
//! genuinely produces under these conditions, not a deliberately weakened baseline.

// This benchmark binary is dev-only tooling, never built for a downstream library consumer,
// so it isn't bound by the crate's `rust-version` MSRV promise — `std::hint::black_box`
// (stable since 1.66) is fine here even though the library itself targets 1.64.0.
#![allow(clippy::incompatible_msrv)]

#[path = "common.rs"]
mod common;

#[cfg(target_arch = "wasm32")]
fn main() {
    use bitpacking::{BitPacker as BitPackerTrait, BitPacker4x};
    use wasm_bitpack::bench_support::{BENCH_SEED, BENCH_SIZES};

    let (num_bits, pattern) = common::parse_args();
    let bp = BitPacker4x::new();
    let block_len = BitPacker4x::BLOCK_LEN;

    for &size in &BENCH_SIZES {
        assert_eq!(
            size % block_len,
            0,
            "benchmark sizes must be exact multiples of BLOCK_LEN"
        );
        let values = pattern.generate(BENCH_SEED, num_bits, size);

        let mut compressed = vec![0u8; size * 4];
        let mut pos = 0;
        for chunk in values.chunks(block_len) {
            pos += bp.compress(chunk, &mut compressed[pos..], num_bits);
        }
        compressed.truncate(pos);

        let mut out = vec![0u32; size];
        let (median_ns, stddev_ns) = common::time_calls(|| {
            let compressed = std::hint::black_box(&compressed);
            let mut in_pos = 0;
            for chunk in out.chunks_mut(block_len) {
                in_pos += bp.decompress(&compressed[in_pos..], chunk, num_bits);
            }
            std::hint::black_box(&out);
        });

        common::report(
            "bitpacking_bitpacker4x",
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
