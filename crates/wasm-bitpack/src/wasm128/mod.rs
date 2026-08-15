//! `wasm32` + `simd128` hand-written SIMD decode fast path.
//!
//! The only module in this crate allowed to contain `unsafe` code (see
//! `plans/03-coding-standards.md` §3 for the enforcement mechanics). The [`Wasm128`] type
//! itself is declared unconditionally so downstream code can name it on any target without
//! a hard compile error; its [`BitPacker`](crate::BitPacker) implementation is `cfg`-gated
//! to `wasm32` + `simd128` and is compiled out entirely for any other target — attempting
//! to call its methods off-target is a compile error at the call site, not a runtime
//! failure.

#![allow(unsafe_code)]

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod decode_macros;

/// SIMD128-accelerated [`BitPacker`](crate::BitPacker) implementation.
///
/// Only actually constructible/useful when compiled for `wasm32` with the `simd128`
/// target feature enabled.
///
/// # Examples
///
/// ```ignore
/// // Only compiles when targeting wasm32 with simd128 enabled (e.g. under
/// // `cargo test --target wasm32-wasip1`) — ignored on the default doctest run since
/// // `impl BitPacker for Wasm128` doesn't exist off-target.
/// use wasm_bitpack::{BitPacker, Wasm128};
///
/// let values = vec![7u32; Wasm128::BLOCK_LEN];
/// let num_bits = Wasm128::num_bits(&values);
///
/// let mut compressed = vec![0u8; Wasm128::BLOCK_LEN * num_bits as usize / 8];
/// let written = Wasm128::compress(&values, &mut compressed, num_bits);
///
/// let mut decompressed = vec![0u32; Wasm128::BLOCK_LEN];
/// let read = Wasm128::decompress(&compressed[..written], &mut decompressed, num_bits);
/// assert_eq!(read, written);
/// assert_eq!(decompressed, values);
/// ```
pub struct Wasm128;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod imp {
    use super::Wasm128;
    use crate::scalar::{check_num_bits, packed_len_bytes};
    use crate::traits::BitPacker;
    use crate::Scalar;

    #[cfg_attr(
        docsrs,
        doc(cfg(all(target_arch = "wasm32", target_feature = "simd128")))
    )]
    impl BitPacker for Wasm128 {
        const BLOCK_LEN: usize = 128;

        /// Delegates to [`Scalar::num_bits`] — computing the minimum bit width is a cheap,
        /// branchy, non-hot-path scan with no SIMD-shaped work in it, so there's no reason
        /// for a second implementation to maintain and keep in sync.
        fn num_bits(decompressed: &[u32]) -> u8 {
            Scalar::num_bits(decompressed)
        }

        /// Delegates to [`Scalar::compress`] — encode is scalar-only in v0.1 (see
        /// `plans/00-overview.md` §4 non-goals). This delegates rather than panicking so
        /// that [`pack`](crate::pack), which picks [`Wasm128`] as its full-block codec on
        /// this target, stays functional end-to-end instead of losing its encode path the
        /// moment it's compiled for `wasm32` + `simd128`.
        fn compress(decompressed: &[u32], compressed: &mut [u8], num_bits: u8) -> usize {
            Scalar::compress(decompressed, compressed, num_bits)
        }

        /// # Panics
        ///
        /// Panics if `decompressed.len() != Self::BLOCK_LEN`, if `compressed` is too small
        /// to hold `Self::BLOCK_LEN * num_bits` bits, or if `num_bits` is `0` or greater
        /// than `32` — identical preconditions to [`Scalar::decompress`], since both must
        /// satisfy the same [`BitPacker::decompress`] contract.
        fn decompress(compressed: &[u8], decompressed: &mut [u32], num_bits: u8) -> usize {
            assert_eq!(
                decompressed.len(),
                Self::BLOCK_LEN,
                "decompressed.len() must be exactly BLOCK_LEN ({}), got {}",
                Self::BLOCK_LEN,
                decompressed.len()
            );
            check_num_bits(num_bits);
            let needed = packed_len_bytes(Self::BLOCK_LEN, num_bits);
            assert!(
                compressed.len() >= needed,
                "compressed buffer too small: need at least {needed} bytes, got {}",
                compressed.len()
            );

            // SAFETY: `simd128` is enabled — this whole module is `cfg`-gated on it, which
            // is `super::decode_macros::decode`'s sole target-feature precondition.
            // `compressed.len() >= needed` and `decompressed.len() == Self::BLOCK_LEN` were
            // just asserted above, satisfying its other two (panic-on-violation, not
            // UB-risk) preconditions.
            unsafe {
                super::decode_macros::decode(num_bits, compressed, decompressed);
            }

            needed
        }
    }

    /// The Scalar-vs-Wasm128 equivalence matrix — see `plans/04-testing-strategy.md` §4.
    /// **The single most important test suite in the crate**: it's the entire correctness
    /// argument for the hand-written SIMD decode path. Only compiles under this module's own
    /// `cfg` gate, and is only meaningful run under `cargo test --target wasm32-wasip1`
    /// (native runs never build this module at all, since `target_feature = "simd128"` is
    /// off by default off-wasm32).
    #[cfg(test)]
    mod tests {
        use super::Wasm128;
        use crate::testing::bench_gen;
        use crate::testing::gen;
        use crate::{BitPacker, Scalar};
        use proptest::prelude::*;

        /// Every bit-width's `# Panics`-documented boundary is `Self::BLOCK_LEN`, so every
        /// pattern here is exactly one block — the matrix compares `BitPacker::decompress`
        /// directly, not the arbitrary-length `pack`/`unpack` wrapper.
        ///
        /// `random`/`sorted_ascending` reuse `testing::bench_gen`'s generators — the same
        /// ones the benchmark harness draws from (`plans/05-benchmarking-strategy.md` §4) —
        /// so a SIMD bug can never hide behind inputs the benchmark suite happens not to
        /// exercise, or vice versa (`plans/04-testing-strategy.md` §9).
        fn patterns(num_bits: u8) -> [(&'static str, Vec<u32>); 5] {
            let block_len = Scalar::BLOCK_LEN;
            let max = gen::max_value_for_num_bits(num_bits);
            // Vary the seed per bit-width so different widths don't share one raw xorshift
            // sequence before masking.
            let seed = bench_gen::BENCH_SEED ^ u32::from(num_bits).wrapping_mul(0x9E37_79B9);

            [
                ("zeros", vec![0u32; block_len]),
                ("max_value", vec![max; block_len]),
                (
                    "random",
                    bench_gen::random_values(seed, num_bits, block_len),
                ),
                (
                    "sorted_ascending",
                    bench_gen::sorted_ascending_values(num_bits, block_len),
                ),
                (
                    "alternating_min_max",
                    (0..block_len)
                        .map(|i| if i % 2 == 0 { 0 } else { max })
                        .collect(),
                ),
            ]
        }

        fn assert_wasm128_matches_scalar(num_bits: u8, values: &[u32], label: &str) {
            let block_len = Scalar::BLOCK_LEN;
            let mut compressed = vec![0u8; block_len * 4 + 8];
            let written = Scalar::compress(values, &mut compressed, num_bits);
            let compressed = &compressed[..written];

            let mut scalar_out = vec![0u32; block_len];
            Scalar::decompress(compressed, &mut scalar_out, num_bits);

            let mut wasm_out = vec![0u32; block_len];
            Wasm128::decompress(compressed, &mut wasm_out, num_bits);

            assert_eq!(
                wasm_out, scalar_out,
                "Wasm128::decompress mismatched Scalar::decompress at num_bits={num_bits}, \
                 pattern={label}"
            );
        }

        #[test]
        #[allow(clippy::cast_possible_truncation)] // `i % modulus < 2^32` always, by construction
        fn equivalence_matrix_fixed_patterns() {
            for num_bits in 1u8..=32 {
                for (label, values) in patterns(num_bits) {
                    assert_wasm128_matches_scalar(num_bits, &values, label);
                }
            }
        }

        proptest! {
            #![proptest_config(gen::proptest_config(200))]

            #[test]
            fn equivalence_matrix_random(
                (num_bits, values) in gen::num_bits_and_block_values(Scalar::BLOCK_LEN)
            ) {
                assert_wasm128_matches_scalar(num_bits, &values, "proptest_random");
            }
        }
    }
}
