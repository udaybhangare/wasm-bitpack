//! Deterministic, dependency-free input generation for the benchmark harness (native
//! criterion benches and `xtask bench-wasm`) — see `plans/05-benchmarking-strategy.md` §4.
//!
//! Kept dependency-free (no `proptest`) and compiled unconditionally (not `#[cfg(test)]`-only
//! like the rest of `testing/`) so it can be re-exported through the `bench-support` Cargo
//! feature (see [`crate::bench_support`]) without pulling a dev-dependency into what would
//! otherwise look like a normal, always-buildable feature.

#![allow(dead_code)]
#![allow(clippy::cast_possible_truncation)] // every truncating cast below is a value already
                                            // proven to fit `u32` (bounded by `num_bits <=
                                            // 32`, so `mask`/`modulus - 1` always fit)

/// Fixed RNG seed for deterministic benchmark input generation — the same xorshift32 seed
/// used by Phase 0's premise-validation spike (`plans/results/phase0-premise-validation.md`)
/// and Phase 2's equivalence-matrix `random` pattern, reused here so every run of
/// `cargo xtask bench-all` generates byte-identical input data. Not cryptographic; chosen
/// only for reproducibility.
pub const BENCH_SEED: u32 = 0x2545_F491;

/// Bit-width spot checks used across the benchmark matrix (`plans/05-benchmarking-strategy.md`
/// §4): the low end (near-degenerate), common real-world widths, odd widths that stress
/// shuffle/shift logic, and the high end (no packing headroom at all).
pub const BENCH_BIT_WIDTHS: [u8; 9] = [1, 2, 4, 8, 11, 16, 20, 24, 32];

/// Input sizes (element counts), log-scaled from one block up to 10,000,000 elements, per
/// `plans/05-benchmarking-strategy.md` §4. Each is an exact multiple of `BLOCK_LEN` (128) —
/// `10_000_000 = 128 * 78_125` — so the benchmark harness never has to special-case a partial
/// trailing block for `bitpacking::BitPacker4x`, which (unlike this crate's `pack`/`unpack`)
/// has no partial-block API at all. Four points (not every power of ten in between) keeps the
/// full bit-width × pattern × competitor matrix's wall-clock time practical while still
/// spanning the overhead-dominated-small to steady-state-large regimes the methodology cares
/// about (`plans/05-benchmarking-strategy.md` §4).
pub const BENCH_SIZES: [usize; 4] = [128, 12_800, 1_280_000, 10_000_000];

/// The largest value representable in `num_bits` bits.
pub fn max_value_for_num_bits(num_bits: u8) -> u32 {
    if num_bits == 32 {
        u32::MAX
    } else {
        (1u32 << num_bits) - 1
    }
}

/// The named input patterns used across the benchmark matrix.
#[derive(Clone, Copy, Debug)]
pub enum BenchPattern {
    /// Uniform-random within the bit-width's representable range.
    Random,
    /// Ascending, wrapping at the bit-width's max value plus one — the favorable case for
    /// real time-series/ID data.
    SortedAscending,
    /// A bounded random walk (small deltas between consecutive values, wrapping at the
    /// bit-width's max value plus one) — relevant context for the Phase 4 `compress_sorted`
    /// variant, even though v0.1 only ever benchmarks decode.
    DeltaFriendly,
}

impl BenchPattern {
    /// Every pattern, in the order the benchmark matrix reports them.
    pub const ALL: [Self; 3] = [Self::Random, Self::SortedAscending, Self::DeltaFriendly];

    /// The name used in benchmark output (JSON field values, results tables).
    pub fn name(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::SortedAscending => "sorted_ascending",
            Self::DeltaFriendly => "delta_friendly",
        }
    }

    /// Generates `len` values, each fit to `num_bits`, deterministically from `seed`.
    pub fn generate(self, seed: u32, num_bits: u8, len: usize) -> Vec<u32> {
        match self {
            Self::Random => random_values(seed, num_bits, len),
            Self::SortedAscending => sorted_ascending_values(num_bits, len),
            Self::DeltaFriendly => delta_friendly_values(seed, num_bits, len),
        }
    }
}

/// Deterministic xorshift32 PRNG, masked to `num_bits`. The same generator used by the
/// Phase 2 Scalar-vs-Wasm128 equivalence matrix's `random` pattern (see `wasm128/mod.rs`),
/// so benchmark inputs are never a separate, potentially-drifted generator from what's
/// actually proven correct (`plans/04-testing-strategy.md` §9).
pub fn random_values(seed: u32, num_bits: u8, len: usize) -> Vec<u32> {
    let mask = max_value_for_num_bits(num_bits);
    let mut state = seed | 1; // xorshift32 requires a nonzero state
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state & mask
        })
        .collect()
}

/// Ascending values, wrapping at `num_bits`'s max representable value plus one.
pub fn sorted_ascending_values(num_bits: u8, len: usize) -> Vec<u32> {
    let modulus = u64::from(max_value_for_num_bits(num_bits)) + 1;
    (0..len as u64).map(|i| (i % modulus) as u32).collect()
}

/// A bounded random walk: each step moves by a small deterministic delta (derived from the
/// same xorshift32 stream as [`random_values`]), wrapping at `num_bits`'s max value plus one.
pub fn delta_friendly_values(seed: u32, num_bits: u8, len: usize) -> Vec<u32> {
    let mask = max_value_for_num_bits(num_bits);
    let modulus = u64::from(mask) + 1;
    let delta_bound = modulus.min(8); // a small step, never larger than the representable range
    let mut state = seed | 1;
    let mut value = 0u64;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            value = (value + u64::from(state) % delta_bound) % modulus;
            value as u32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_patterns_fit_bit_width() {
        for &num_bits in &BENCH_BIT_WIDTHS {
            let max = max_value_for_num_bits(num_bits);
            for pattern in BenchPattern::ALL {
                let values = pattern.generate(BENCH_SEED, num_bits, 1000);
                assert_eq!(values.len(), 1000);
                assert!(
                    values.iter().all(|&v| v <= max),
                    "pattern {} produced a value exceeding {num_bits}-bit range",
                    pattern.name()
                );
            }
        }
    }

    #[test]
    fn deterministic_across_calls() {
        let a = BenchPattern::Random.generate(BENCH_SEED, 11, 500);
        let b = BenchPattern::Random.generate(BENCH_SEED, 11, 500);
        assert_eq!(a, b);
    }
}
