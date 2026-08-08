//! Shared proptest/bench input-generation strategies, used by both the correctness test
//! suite and the criterion/xtask benches so both draw from the same input methodology.

#![allow(dead_code)]

use proptest::prelude::*;

/// A `num_bits` strategy, uniform over the full valid range `1..=32`.
pub(crate) fn num_bits_strategy() -> impl Strategy<Value = u8> {
    1u8..=32
}

/// The largest value representable in `num_bits` bits.
pub(crate) fn max_value_for_num_bits(num_bits: u8) -> u32 {
    if num_bits == 32 {
        u32::MAX
    } else {
        (1u32 << num_bits) - 1
    }
}

/// A strategy for exactly `len` `u32` values, each bounded to fit in `num_bits` bits.
pub(crate) fn values_strategy(num_bits: u8, len: usize) -> impl Strategy<Value = Vec<u32>> {
    proptest::collection::vec(0..=max_value_for_num_bits(num_bits), len)
}

/// A strategy for `(num_bits, values)` pairs where `values` has a length drawn from
/// `len_strategy` and every value is bounded to fit in `num_bits` bits.
pub(crate) fn num_bits_and_values(
    len_strategy: impl Strategy<Value = usize>,
) -> impl Strategy<Value = (u8, Vec<u32>)> {
    (num_bits_strategy(), len_strategy).prop_flat_map(|(num_bits, len)| {
        values_strategy(num_bits, len).prop_map(move |values| (num_bits, values))
    })
}

/// A strategy for `(num_bits, values)` pairs where `values` has exactly `block_len`
/// elements — for testing the block-exact [`BitPacker`](crate::BitPacker) trait directly.
pub(crate) fn num_bits_and_block_values(block_len: usize) -> impl Strategy<Value = (u8, Vec<u32>)> {
    num_bits_and_values(Just(block_len))
}

/// Arbitrary lengths for the `pack`/`unpack` wrapper: covers `0`, sub-block, exact-block,
/// and multi-block-plus-remainder sizes (assuming a `BLOCK_LEN` of 128, the only value used
/// in v0.1).
pub(crate) fn arbitrary_len_strategy() -> impl Strategy<Value = usize> {
    0usize..=300
}

/// A `ProptestConfig` running `cases` cases under normal execution, scaled down to a much
/// smaller case count under miri with file-based failure persistence disabled.
///
/// Two miri-specific problems, both worked around here rather than at every call site:
/// - Miri's default sandbox has no filesystem/cwd access, which proptest's default
///   `FileFailurePersistence` needs merely to *locate* the `proptest-regressions/`
///   directory (even when no failure is ever recorded) — so file persistence must be off
///   whenever `cfg(miri)` is true, or every property test aborts with an
///   unsupported-operation error before running a single case.
/// - Miri interprets rather than compiles, so it is orders of magnitude slower; running the
///   full case count under miri would make it impractically slow. `cfg(miri)` scales the
///   count down, since miri's job here is UB detection, not exhaustive property
///   verification (already covered by the full case count under normal `cargo test`).
///
/// Persistence and the full case count stay on for normal `cargo test` runs, so
/// proptest-discovered failures still land in a committed regression file as required by
/// `plans/04-testing-strategy.md` §3.
pub(crate) fn proptest_config(cases: u32) -> ProptestConfig {
    #[cfg(not(miri))]
    {
        ProptestConfig::with_cases(cases)
    }
    #[cfg(miri)]
    {
        ProptestConfig {
            cases: cases.min(16),
            failure_persistence: None,
            ..ProptestConfig::default()
        }
    }
}
