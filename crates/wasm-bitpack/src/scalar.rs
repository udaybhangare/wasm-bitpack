//! [`Scalar`] — the pure safe-Rust reference bit-packing implementation, always available.

use crate::traits::BitPacker;

/// Portable, safe-Rust reference implementation of [`BitPacker`].
///
/// Correct on every target, including under miri. Every other implementation in this
/// crate is checked for byte-for-byte equivalence against `Scalar`'s output.
pub struct Scalar;

impl BitPacker for Scalar {
    const BLOCK_LEN: usize = 128;

    fn num_bits(decompressed: &[u32]) -> u8 {
        let _ = decompressed;
        todo!("implemented in Phase 1 (see plans/prompts/phase-1-scalar-reference.md)")
    }

    fn compress(decompressed: &[u32], compressed: &mut [u8], num_bits: u8) -> usize {
        let _ = (decompressed, compressed, num_bits);
        todo!("implemented in Phase 1 (see plans/prompts/phase-1-scalar-reference.md)")
    }

    fn decompress(compressed: &[u8], decompressed: &mut [u32], num_bits: u8) -> usize {
        let _ = (compressed, decompressed, num_bits);
        todo!("implemented in Phase 1 (see plans/prompts/phase-1-scalar-reference.md)")
    }
}
