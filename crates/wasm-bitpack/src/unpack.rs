//! [`unpack`] — arbitrary-length convenience wrapper over
//! [`BitPacker::decompress`](crate::BitPacker::decompress).
//!
//! Chunks `compressed` into `BitPacker::BLOCK_LEN`-sized blocks, dispatching full blocks
//! to the fastest available [`BitPacker`](crate::BitPacker) implementation and handling any
//! trailing partial block with direct scalar bit-twiddling rather than a second
//! `BitPacker` implementation (see `plans/decisions/0006-partial-block-api.md`).

use crate::scalar::{bitunpack_into, check_num_bits, packed_len_bytes};
use crate::traits::BitPacker;
use crate::Scalar;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use crate::Wasm128;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
type BestPacker = Wasm128;
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
type BestPacker = Scalar;

/// Unpacks an arbitrary-length, `num_bits`-packed byte buffer into `decompressed`,
/// returning the number of bytes read.
///
/// Unlike [`BitPacker::decompress`](crate::BitPacker::decompress), `compressed` does not
/// need to encode an exact multiple of the block length — any trailing partial block is
/// unpacked directly.
///
/// # Panics
///
/// Panics if `compressed` is too small for `decompressed.len()` values packed at
/// `num_bits` bits each, or if `num_bits` is `0` or greater than `32`.
///
/// # Examples
///
/// ```
/// use wasm_bitpack::{pack, unpack};
///
/// let values = vec![1u32, 2, 3, 4, 5];
/// let mut compressed = vec![0u8; values.len() * 4];
/// let written = pack(&values, &mut compressed, 3);
///
/// let mut decompressed = vec![0u32; values.len()];
/// let read = unpack(&compressed[..written], &mut decompressed, 3);
/// assert_eq!(read, written);
/// assert_eq!(decompressed, values);
/// ```
pub fn unpack(compressed: &[u8], decompressed: &mut [u32], num_bits: u8) -> usize {
    unpack_generic::<BestPacker>(compressed, decompressed, num_bits)
}

/// Generic core of [`unpack`], parameterized over the [`BitPacker`] implementation used for
/// full blocks — kept generic so this logic also works for [`Wasm128`](crate::Wasm128)
/// without modification once its `decompress` path exists.
fn unpack_generic<B: BitPacker>(
    compressed: &[u8],
    decompressed: &mut [u32],
    num_bits: u8,
) -> usize {
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
        in_pos += B::decompress(
            &compressed[in_pos..],
            &mut decompressed[out_pos..out_pos + block_len],
            num_bits,
        );
        out_pos += block_len;
    }
    if tail_len > 0 {
        in_pos += bitunpack_into(
            &compressed[in_pos..],
            &mut decompressed[out_pos..],
            num_bits,
        );
    }

    in_pos
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)] // test data lengths/values are always small
mod tests {
    use super::*;
    use crate::pack::pack;

    #[test]
    fn empty_input_reads_nothing() {
        let mut decompressed: Vec<u32> = vec![];
        let read = unpack(&[], &mut decompressed, 5);
        assert_eq!(read, 0);
    }

    #[test]
    fn single_element() {
        let compressed = vec![9u8];
        let mut decompressed = vec![0u32; 1];
        let read = unpack(&compressed, &mut decompressed, 4);
        assert_eq!(read, 1);
        assert_eq!(decompressed[0], 9);
    }

    #[test]
    #[should_panic(expected = "num_bits must be in 1..=32")]
    fn panics_on_num_bits_zero() {
        let compressed = vec![0u8; 16];
        let mut decompressed = vec![0u32; 3];
        unpack(&compressed, &mut decompressed, 0);
    }

    #[test]
    #[should_panic(expected = "num_bits must be in 1..=32")]
    fn panics_on_num_bits_33() {
        let compressed = vec![0u8; 16];
        let mut decompressed = vec![0u32; 3];
        unpack(&compressed, &mut decompressed, 33);
    }

    #[test]
    #[should_panic(expected = "compressed buffer too small")]
    fn panics_on_undersized_input() {
        let compressed = vec![0u8; 1];
        let mut decompressed = vec![0u32; 200];
        unpack(&compressed, &mut decompressed, 32);
    }

    // Boundary lengths called out explicitly by plans/04-testing-strategy.md §2, beyond
    // what the proptest suite below already covers via `gen::arbitrary_len_strategy`.
    #[test]
    fn boundary_lengths_round_trip() {
        use crate::BitPacker;
        use crate::Scalar;

        for len in [
            0usize,
            1,
            Scalar::BLOCK_LEN - 1,
            Scalar::BLOCK_LEN,
            Scalar::BLOCK_LEN + 1,
            2 * Scalar::BLOCK_LEN + 37,
        ] {
            let values: Vec<u32> = (0..len as u32).map(|i| i % 13).collect();
            let num_bits = 4u8;
            let mut compressed = vec![0u8; len * 4 + 8];
            let written = pack(&values, &mut compressed, num_bits);

            let mut decompressed = vec![0u32; len];
            let read = unpack(&compressed[..written], &mut decompressed, num_bits);

            assert_eq!(read, written, "length {len}");
            assert_eq!(decompressed, values, "length {len}");
        }
    }
}
