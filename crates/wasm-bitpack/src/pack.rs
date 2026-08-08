//! [`pack`] — arbitrary-length convenience wrapper over
//! [`BitPacker::compress`](crate::BitPacker::compress).
//!
//! Chunks `decompressed` into `BitPacker::BLOCK_LEN`-sized blocks, dispatching full blocks
//! to the fastest available [`BitPacker`](crate::BitPacker) implementation and handling any
//! trailing partial block with direct scalar bit-twiddling rather than a second
//! `BitPacker` implementation (see `plans/decisions/0006-partial-block-api.md`).

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
pub fn pack(decompressed: &[u32], compressed: &mut [u8], num_bits: u8) -> usize {
    let _ = (decompressed, compressed, num_bits);
    todo!("implemented in Phase 1 (see plans/prompts/phase-1-scalar-reference.md)")
}
