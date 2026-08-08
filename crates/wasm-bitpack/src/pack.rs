//! [`pack`] — arbitrary-length convenience wrapper over
//! [`BitPacker::compress`](crate::BitPacker::compress).
//!
//! Chunks `decompressed` into `BitPacker::BLOCK_LEN`-sized blocks, dispatching full blocks
//! to the fastest available [`BitPacker`](crate::BitPacker) implementation and handling any
//! trailing partial block with direct scalar bit-twiddling rather than a second
//! `BitPacker` implementation (see `plans/decisions/0006-partial-block-api.md`).

use crate::scalar::{bitpack_into, check_num_bits, packed_len_bytes};
use crate::traits::BitPacker;

#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
use crate::Scalar;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use crate::Wasm128;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
type BestPacker = Wasm128;
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
type BestPacker = Scalar;

/// Packs an arbitrary-length slice of `u32` values into `compressed` at `num_bits` bits per
/// value, returning the number of bytes written.
///
/// Unlike [`BitPacker::compress`](crate::BitPacker::compress), `decompressed` does not need
/// to be an exact multiple of the block length — any trailing partial block is packed
/// directly.
///
/// # Panics
///
/// Panics if `compressed` is too small to hold `decompressed.len()` values packed at
/// `num_bits` bits each, or if `num_bits` is `0` or greater than `32`.
///
/// # Examples
///
/// ```
/// use wasm_bitpack::pack;
///
/// let values = vec![1u32, 2, 3, 4, 5];
/// let mut compressed = vec![0u8; values.len() * 4]; // generous upper bound
/// let written = pack(&values, &mut compressed, 3);
/// assert_eq!(written, 2); // 5 values * 3 bits = 15 bits -> 2 bytes
/// ```
pub fn pack(decompressed: &[u32], compressed: &mut [u8], num_bits: u8) -> usize {
    pack_generic::<BestPacker>(decompressed, compressed, num_bits)
}

/// Generic core of [`pack`], parameterized over the [`BitPacker`] implementation used for
/// full blocks — kept generic so this logic also works for [`Wasm128`](crate::Wasm128)
/// without modification once its `compress` path exists.
fn pack_generic<B: BitPacker>(decompressed: &[u32], compressed: &mut [u8], num_bits: u8) -> usize {
    check_num_bits(num_bits);
    let needed = packed_len_bytes(decompressed.len(), num_bits);
    assert!(
        compressed.len() >= needed,
        "compressed buffer too small: need at least {needed} bytes, got {}",
        compressed.len()
    );

    let block_len = B::BLOCK_LEN;
    let full_blocks = decompressed.len() / block_len;
    let tail_len = decompressed.len() % block_len;

    let mut in_pos = 0usize;
    let mut out_pos = 0usize;
    for _ in 0..full_blocks {
        out_pos += B::compress(
            &decompressed[in_pos..in_pos + block_len],
            &mut compressed[out_pos..],
            num_bits,
        );
        in_pos += block_len;
    }
    if tail_len > 0 {
        out_pos += bitpack_into(
            &decompressed[in_pos..],
            &mut compressed[out_pos..],
            num_bits,
        );
    }

    out_pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::gen;
    use crate::unpack::unpack;
    use proptest::prelude::*;

    #[test]
    fn empty_input_writes_nothing() {
        let mut compressed = vec![0u8; 16];
        let written = pack(&[], &mut compressed, 5);
        assert_eq!(written, 0);
    }

    #[test]
    fn single_element() {
        let values = vec![9u32];
        let mut compressed = vec![0u8; 4];
        let written = pack(&values, &mut compressed, 4);
        assert_eq!(written, 1);
        assert_eq!(compressed[0], 9);
    }

    #[test]
    #[should_panic(expected = "num_bits must be in 1..=32")]
    fn panics_on_num_bits_zero() {
        let mut compressed = vec![0u8; 16];
        pack(&[1, 2, 3], &mut compressed, 0);
    }

    #[test]
    #[should_panic(expected = "num_bits must be in 1..=32")]
    fn panics_on_num_bits_33() {
        let mut compressed = vec![0u8; 16];
        pack(&[1, 2, 3], &mut compressed, 33);
    }

    #[test]
    #[should_panic(expected = "compressed buffer too small")]
    fn panics_on_undersized_output() {
        let values = vec![u32::MAX; 200];
        let mut compressed = vec![0u8; 1];
        pack(&values, &mut compressed, 32);
    }

    proptest! {
        #![proptest_config(gen::proptest_config(1000))]

        #[test]
        fn prop_round_trip(
            (num_bits, values) in gen::num_bits_and_values(gen::arbitrary_len_strategy())
        ) {
            let mut compressed = vec![0u8; values.len() * 4 + 8];
            let written = pack(&values, &mut compressed, num_bits);

            let mut decompressed = vec![0u32; values.len()];
            let read = unpack(&compressed[..written], &mut decompressed, num_bits);

            prop_assert_eq!(read, written);
            prop_assert_eq!(decompressed, values);
        }
    }
}
