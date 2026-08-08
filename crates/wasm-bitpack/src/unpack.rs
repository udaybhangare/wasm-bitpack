//! [`unpack`] — arbitrary-length convenience wrapper over
//! [`BitPacker::decompress`](crate::BitPacker::decompress).
//!
//! Chunks `compressed` into `BitPacker::BLOCK_LEN`-sized blocks, dispatching full blocks
//! to the fastest available [`BitPacker`](crate::BitPacker) implementation and handling any
//! trailing partial block with direct scalar bit-twiddling rather than a second
//! `BitPacker` implementation (see `plans/decisions/0006-partial-block-api.md`).

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
pub fn unpack(compressed: &[u8], decompressed: &mut [u32], num_bits: u8) -> usize {
    let _ = (compressed, decompressed, num_bits);
    todo!("implemented in Phase 1 (see plans/prompts/phase-1-scalar-reference.md)")
}
