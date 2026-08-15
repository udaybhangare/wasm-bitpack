//! [`Scalar`] — the pure safe-Rust reference bit-packing implementation, always available.

use crate::traits::BitPacker;

/// Portable, safe-Rust reference implementation of [`BitPacker`].
///
/// Correct on every target, including under miri. Every other implementation in this
/// crate is checked for byte-for-byte equivalence against `Scalar`'s output.
///
/// # Examples
///
/// ```
/// use wasm_bitpack::{BitPacker, Scalar};
///
/// let values = vec![7u32; Scalar::BLOCK_LEN];
/// let num_bits = Scalar::num_bits(&values);
///
/// let mut compressed = vec![0u8; (Scalar::BLOCK_LEN * num_bits as usize + 7) / 8];
/// let written = Scalar::compress(&values, &mut compressed, num_bits);
///
/// let mut decompressed = vec![0u32; Scalar::BLOCK_LEN];
/// Scalar::decompress(&compressed[..written], &mut decompressed, num_bits);
/// assert_eq!(decompressed, values);
/// ```
pub struct Scalar;

impl BitPacker for Scalar {
    const BLOCK_LEN: usize = 128;

    fn num_bits(decompressed: &[u32]) -> u8 {
        let max = decompressed.iter().copied().max().unwrap_or(0);
        bits_for_max_value(max)
    }

    fn compress(decompressed: &[u32], compressed: &mut [u8], num_bits: u8) -> usize {
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

        bitpack_interleaved_into(decompressed, compressed, num_bits)
    }

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

        bitunpack_interleaved_into(compressed, decompressed, num_bits)
    }
}

/// The minimum number of bits needed to represent `max` (`0` for `max == 0`).
///
/// `pub(crate)` (not private) so [`crate::sorted`]'s `num_bits_sorted`/
/// `num_bits_strictly_sorted` can reuse it for their own max-delta-to-bit-width conversion,
/// instead of a second copy of the same `leading_zeros` trick.
#[allow(clippy::cast_possible_truncation)] // `32 - leading_zeros()` is always in 0..=32
pub(crate) fn bits_for_max_value(max: u32) -> u8 {
    (32 - max.leading_zeros()) as u8
}

/// Panics unless `num_bits` is in the valid `1..=32` range.
///
/// Shared between [`Scalar`], and the [`pack`](crate::pack)/[`unpack`](crate::unpack)
/// wrapper, so every entry point rejects an out-of-range bit width with the same message.
pub(crate) fn check_num_bits(num_bits: u8) {
    assert!(
        (1..=32).contains(&num_bits),
        "num_bits must be in 1..=32, got {num_bits}"
    );
}

/// The number of bytes needed to tightly bit-pack `len` values at `num_bits` bits each.
pub(crate) fn packed_len_bytes(len: usize, num_bits: u8) -> usize {
    let total_bits = len as u64 * u64::from(num_bits);
    ((total_bits + 7) / 8) as usize
}

/// Tightly bit-packs `values` into `out` at `num_bits` bits per value (LSB-first: value
/// `i`'s bit `0` is the `(i * num_bits)`-th bit of the output stream, counting from the
/// least-significant bit of `out[0]`), returning the number of bytes written.
///
/// No length restriction — this is the primitive both [`Scalar::compress`] (exact
/// `BLOCK_LEN` blocks) and the [`pack`](crate::pack) wrapper's partial-block tail build on.
///
/// Trusts its caller: `out` must already be verified to be at least
/// `packed_len_bytes(values.len(), num_bits)` bytes, and `num_bits` must already be
/// verified to be in `1..=32`.
#[allow(clippy::cast_possible_truncation)] // `acc & 0xff` always fits in a u8
pub(crate) fn bitpack_into(values: &[u32], out: &mut [u8], num_bits: u8) -> usize {
    let mask = (1u64 << num_bits) - 1;
    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;
    let mut out_pos = 0usize;

    for &v in values {
        acc |= (u64::from(v) & mask) << acc_bits;
        acc_bits += u32::from(num_bits);
        while acc_bits >= 8 {
            out[out_pos] = (acc & 0xff) as u8;
            out_pos += 1;
            acc >>= 8;
            acc_bits -= 8;
        }
    }
    if acc_bits > 0 {
        out[out_pos] = (acc & 0xff) as u8;
        out_pos += 1;
    }

    out_pos
}

/// Unpacks `out.len()` values from `bytes`, the inverse of [`bitpack_into`], returning the
/// number of bytes read.
///
/// Trusts its caller in the same way [`bitpack_into`] does.
#[allow(clippy::cast_possible_truncation)] // `acc & mask` fits in a u32 for num_bits <= 32
pub(crate) fn bitunpack_into(bytes: &[u8], out: &mut [u32], num_bits: u8) -> usize {
    let mask = (1u64 << num_bits) - 1;
    let mut acc: u64 = 0;
    let mut acc_bits: u32 = 0;
    let mut in_pos = 0usize;

    for slot in out.iter_mut() {
        while acc_bits < u32::from(num_bits) {
            acc |= u64::from(bytes[in_pos]) << acc_bits;
            in_pos += 1;
            acc_bits += 8;
        }
        *slot = (acc & mask) as u32;
        acc >>= u32::from(num_bits);
        acc_bits -= u32::from(num_bits);
    }

    in_pos
}

/// Number of interleaved lanes the block-exact packed format below splits a block into — one
/// lane per `u32` position in the WASM SIMD128 register [`Wasm128`](crate::Wasm128)'s decode
/// kernel loads. See `plans/decisions/0010-bp128-style-packed-format.md` for the full
/// rationale.
const LANES: usize = 4;

/// Tightly bit-packs a `BLOCK_LEN`-exact (128-value) block into `out` using the 4-way
/// interleaved layout [`Wasm128`](crate::Wasm128)'s decode kernel expects, returning the
/// number of bytes written: split `values` into 4 stride-4 sub-streams (lane `j` holds values
/// `j, j+4, ..., j+124`), bit-pack each lane independently via [`bitpack_into`], then
/// interleave the 4 lanes' packed 32-bit words round-robin (`out`'s first 4 bytes are lane 0's
/// first word, the next 4 are lane 1's first word, and so on). See
/// `plans/decisions/0010-bp128-style-packed-format.md`.
///
/// Because `values.len() / LANES` is always `32` (the only block length this crate supports),
/// each lane's packed length is always an exact multiple of 4 bytes for every `num_bits` in
/// `1..=32` (`32 * num_bits` bits is always a whole number of 32-bit words) — no padding, no
/// partial words, ever, and the total output size always equals
/// `packed_len_bytes(values.len(), num_bits)`, unchanged from the flat format.
///
/// Trusts its caller the same way [`bitpack_into`] does: `values.len()` must already be
/// exactly `BLOCK_LEN` (128), `out` at least `packed_len_bytes(values.len(), num_bits)` bytes,
/// and `num_bits` already verified to be in `1..=32`.
pub(crate) fn bitpack_interleaved_into(values: &[u32], out: &mut [u8], num_bits: u8) -> usize {
    let lane_len = values.len() / LANES;
    let lane_bytes = packed_len_bytes(lane_len, num_bits);

    let mut lane_values = [0u32; 32];
    let mut lane_packed = [0u8; 128];

    for lane in 0..LANES {
        for (i, slot) in lane_values[..lane_len].iter_mut().enumerate() {
            *slot = values[lane + i * LANES];
        }
        bitpack_into(
            &lane_values[..lane_len],
            &mut lane_packed[..lane_bytes],
            num_bits,
        );

        for (word, chunk) in lane_packed[..lane_bytes].chunks_exact(4).enumerate() {
            let dst = word * LANES * 4 + lane * 4;
            out[dst..dst + 4].copy_from_slice(chunk);
        }
    }

    lane_bytes * LANES
}

/// Unpacks `out.len()` values from `bytes`, the inverse of [`bitpack_interleaved_into`],
/// returning the number of bytes read.
///
/// Trusts its caller in the same way [`bitpack_interleaved_into`] does.
pub(crate) fn bitunpack_interleaved_into(bytes: &[u8], out: &mut [u32], num_bits: u8) -> usize {
    let lane_len = out.len() / LANES;
    let lane_bytes = packed_len_bytes(lane_len, num_bits);

    let mut lane_values = [0u32; 32];
    let mut lane_packed = [0u8; 128];

    for lane in 0..LANES {
        for (word, chunk) in lane_packed[..lane_bytes].chunks_exact_mut(4).enumerate() {
            let src = word * LANES * 4 + lane * 4;
            chunk.copy_from_slice(&bytes[src..src + 4]);
        }
        bitunpack_into(
            &lane_packed[..lane_bytes],
            &mut lane_values[..lane_len],
            num_bits,
        );

        for (i, &value) in lane_values[..lane_len].iter().enumerate() {
            out[lane + i * LANES] = value;
        }
    }

    lane_bytes * LANES
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation)] // test data lengths/values are always small
mod tests {
    use super::*;
    use crate::testing::{gen, oracle};
    use proptest::prelude::*;

    fn block(value: u32) -> Vec<u32> {
        vec![value; Scalar::BLOCK_LEN]
    }

    #[test]
    fn num_bits_all_zero_is_zero() {
        assert_eq!(Scalar::num_bits(&[0, 0, 0]), 0);
    }

    #[test]
    fn num_bits_matches_expected_widths() {
        assert_eq!(Scalar::num_bits(&[1]), 1);
        assert_eq!(Scalar::num_bits(&[2]), 2);
        assert_eq!(Scalar::num_bits(&[3]), 2);
        assert_eq!(Scalar::num_bits(&[u32::MAX]), 32);
    }

    #[test]
    fn round_trip_num_bits_1() {
        let values: Vec<u32> = (0..Scalar::BLOCK_LEN as u32).map(|i| i % 2).collect();
        let mut compressed = vec![0u8; packed_len_bytes(Scalar::BLOCK_LEN, 1)];
        let written = Scalar::compress(&values, &mut compressed, 1);
        assert_eq!(written, compressed.len());

        let mut decompressed = vec![0u32; Scalar::BLOCK_LEN];
        let read = Scalar::decompress(&compressed, &mut decompressed, 1);
        assert_eq!(read, compressed.len());
        assert_eq!(decompressed, values);
    }

    #[test]
    fn round_trip_num_bits_32() {
        let values: Vec<u32> = (0..Scalar::BLOCK_LEN as u32)
            .map(|i| u32::MAX - i)
            .collect();
        let mut compressed = vec![0u8; packed_len_bytes(Scalar::BLOCK_LEN, 32)];
        let written = Scalar::compress(&values, &mut compressed, 32);
        assert_eq!(written, compressed.len());

        let mut decompressed = vec![0u32; Scalar::BLOCK_LEN];
        Scalar::decompress(&compressed, &mut decompressed, 32);
        assert_eq!(decompressed, values);
    }

    #[test]
    fn compress_matches_oracle() {
        for num_bits in [1u8, 3, 7, 8, 11, 16, 17, 24, 32] {
            let modulus = 1u64 << num_bits;
            let values: Vec<u32> = (0..Scalar::BLOCK_LEN as u64)
                .map(|i| (i % modulus) as u32)
                .collect();

            let mut compressed = vec![0u8; packed_len_bytes(Scalar::BLOCK_LEN, num_bits)];
            Scalar::compress(&values, &mut compressed, num_bits);

            let expected = oracle::oracle_encode_interleaved(&values, num_bits);
            assert_eq!(compressed, expected, "mismatch at num_bits={num_bits}");
        }
    }

    #[test]
    fn interleaved_round_trip() {
        for num_bits in [1u8, 3, 7, 8, 11, 16, 17, 24, 32] {
            let modulus = 1u64 << num_bits;
            let values: Vec<u32> = (0..Scalar::BLOCK_LEN as u64)
                .map(|i| (i * 7 % modulus) as u32)
                .collect();

            let mut packed = vec![0u8; packed_len_bytes(Scalar::BLOCK_LEN, num_bits)];
            let written = bitpack_interleaved_into(&values, &mut packed, num_bits);
            assert_eq!(written, packed.len());

            let mut decoded = vec![0u32; Scalar::BLOCK_LEN];
            let read = bitunpack_interleaved_into(&packed, &mut decoded, num_bits);
            assert_eq!(read, packed.len());
            assert_eq!(
                decoded, values,
                "round-trip mismatch at num_bits={num_bits}"
            );
        }
    }

    /// Hand-verifiable layout check: at `num_bits == 32` each lane's `w`-th packed word is
    /// simply that lane's `w`-th value's raw little-endian bytes (no bit-level packing
    /// overlap), so the interleaved output at word `w`, lane `j` must be the raw bytes of
    /// `values[j + w * 4]` — e.g. word 0 holds values `0, 1, 2, 3` (one per lane) and word 1
    /// holds values `4, 5, 6, 7`.
    #[test]
    fn interleaved_known_layout_num_bits_32() {
        let values: Vec<u32> = (0..Scalar::BLOCK_LEN as u32).collect();
        let mut packed = vec![0u8; packed_len_bytes(Scalar::BLOCK_LEN, 32)];
        bitpack_interleaved_into(&values, &mut packed, 32);

        for word in 0..32usize {
            for lane in 0..4usize {
                let expected = (lane as u32 + word as u32 * 4).to_le_bytes();
                let offset = word * 16 + lane * 4;
                assert_eq!(
                    &packed[offset..offset + 4],
                    &expected,
                    "mismatch at word={word}, lane={lane}"
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "decompressed.len() must be exactly BLOCK_LEN")]
    fn compress_panics_on_wrong_length() {
        let values = vec![0u32; Scalar::BLOCK_LEN - 1];
        let mut compressed = vec![0u8; 1024];
        Scalar::compress(&values, &mut compressed, 4);
    }

    #[test]
    #[should_panic(expected = "decompressed.len() must be exactly BLOCK_LEN")]
    fn decompress_panics_on_wrong_length() {
        let compressed = vec![0u8; 1024];
        let mut decompressed = vec![0u32; Scalar::BLOCK_LEN + 1];
        Scalar::decompress(&compressed, &mut decompressed, 4);
    }

    #[test]
    #[should_panic(expected = "num_bits must be in 1..=32")]
    fn compress_panics_on_num_bits_zero() {
        let values = block(0);
        let mut compressed = vec![0u8; 1024];
        Scalar::compress(&values, &mut compressed, 0);
    }

    #[test]
    #[should_panic(expected = "num_bits must be in 1..=32")]
    fn compress_panics_on_num_bits_33() {
        let values = block(0);
        let mut compressed = vec![0u8; 1024];
        Scalar::compress(&values, &mut compressed, 33);
    }

    #[test]
    #[should_panic(expected = "compressed buffer too small")]
    fn compress_panics_on_undersized_output() {
        let values = block(u32::MAX);
        let mut compressed = vec![0u8; 1];
        Scalar::compress(&values, &mut compressed, 32);
    }

    #[test]
    #[should_panic(expected = "compressed buffer too small")]
    fn decompress_panics_on_undersized_input() {
        let compressed = vec![0u8; 1];
        let mut decompressed = vec![0u32; Scalar::BLOCK_LEN];
        Scalar::decompress(&compressed, &mut decompressed, 32);
    }

    proptest! {
        #![proptest_config(gen::proptest_config(1000))]

        #[test]
        fn prop_compress_matches_oracle(
            (num_bits, values) in gen::num_bits_and_block_values(Scalar::BLOCK_LEN)
        ) {
            let mut compressed = vec![0u8; packed_len_bytes(Scalar::BLOCK_LEN, num_bits)];
            Scalar::compress(&values, &mut compressed, num_bits);

            let expected = oracle::oracle_encode_interleaved(&values, num_bits);
            prop_assert_eq!(compressed, expected);
        }

        #[test]
        fn prop_round_trip(
            (num_bits, values) in gen::num_bits_and_block_values(Scalar::BLOCK_LEN)
        ) {
            let mut compressed = vec![0u8; packed_len_bytes(Scalar::BLOCK_LEN, num_bits)];
            Scalar::compress(&values, &mut compressed, num_bits);

            let mut decompressed = vec![0u32; Scalar::BLOCK_LEN];
            Scalar::decompress(&compressed, &mut decompressed, num_bits);
            prop_assert_eq!(decompressed, values);
        }

        #[test]
        fn prop_decompress_matches_oracle(
            (num_bits, values) in gen::num_bits_and_block_values(Scalar::BLOCK_LEN)
        ) {
            let compressed = oracle::oracle_encode_interleaved(&values, num_bits);

            let mut decompressed = vec![0u32; Scalar::BLOCK_LEN];
            Scalar::decompress(&compressed, &mut decompressed, num_bits);
            prop_assert_eq!(decompressed, values);
        }
    }
}
