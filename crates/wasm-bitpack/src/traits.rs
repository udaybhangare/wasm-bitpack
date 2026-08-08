//! The [`BitPacker`] trait — the single definition of codec behavior, implemented by
//! [`Scalar`](crate::Scalar) and [`Wasm128`](crate::Wasm128).

mod private {
    pub trait Sealed {}
    impl Sealed for crate::Scalar {}
    impl Sealed for crate::Wasm128 {}
}

/// A fixed-block-size integer bit-packing codec.
///
/// Packs `u32` values into a tightly-bit-packed byte buffer at a caller-chosen bit width,
/// and unpacks them back out. Every implementation operates on exactly
/// [`BLOCK_LEN`](Self::BLOCK_LEN) values at a time — arbitrary-length input is handled by
/// the [`pack`](crate::pack)/[`unpack`](crate::unpack) wrapper, not by this trait directly.
///
/// This trait is sealed: it can only be implemented by types defined in this crate, so
/// adding a new required method here is never a breaking change for downstream users.
pub trait BitPacker: private::Sealed {
    /// The number of `u32` values a single block holds. `128` for every v0.1
    /// implementation.
    const BLOCK_LEN: usize;

    /// Returns the minimum number of bits needed to represent every value in
    /// `decompressed`.
    fn num_bits(decompressed: &[u32]) -> u8;

    /// Packs exactly [`BLOCK_LEN`](Self::BLOCK_LEN) values from `decompressed` into
    /// `compressed` at `num_bits` bits per value, returning the number of bytes written.
    ///
    /// # Panics
    ///
    /// Panics if `decompressed.len() != Self::BLOCK_LEN`, if `compressed` is too small to
    /// hold `Self::BLOCK_LEN * num_bits` bits, or if `num_bits` is `0` or greater than
    /// `32`.
    fn compress(decompressed: &[u32], compressed: &mut [u8], num_bits: u8) -> usize;

    /// Unpacks exactly [`BLOCK_LEN`](Self::BLOCK_LEN) values from `compressed` into
    /// `decompressed`, returning the number of bytes read.
    ///
    /// # Panics
    ///
    /// Panics if `decompressed.len() != Self::BLOCK_LEN`, if `compressed` is too small to
    /// hold `Self::BLOCK_LEN * num_bits` bits, or if `num_bits` is `0` or greater than
    /// `32`.
    fn decompress(compressed: &[u8], decompressed: &mut [u32], num_bits: u8) -> usize;
}
