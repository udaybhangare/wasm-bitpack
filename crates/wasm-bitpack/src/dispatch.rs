//! [`best_available`] — resolves to the fastest [`BitPacker`] implementation for the
//! current compile target.

use crate::traits::BitPacker;

#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
use crate::Scalar;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
use crate::Wasm128;

/// Returns the fastest [`BitPacker`] implementation available for the current compile
/// target: [`Wasm128`](crate::Wasm128) when compiled for `wasm32` with `simd128` enabled,
/// [`Scalar`] otherwise.
///
/// This is a purely compile-time, `cfg`-driven type selection — WASM engines either
/// support `simd128` for the whole module at compile time or they don't, so there is no
/// runtime feature-detection branch to perform (unlike x86 CPUID checks).
///
/// # Examples
///
/// The concrete type behind the returned `impl BitPacker` can't be named directly, so it's
/// used through a small generic helper, same as this crate's own integration tests do:
///
/// ```
/// use wasm_bitpack::{best_available, BitPacker};
///
/// fn round_trip<B: BitPacker>(packer: &B, values: &[u32], num_bits: u8) -> Vec<u32> {
///     let _ = packer;
///     let mut compressed = vec![0u8; B::BLOCK_LEN * num_bits as usize / 8];
///     let written = B::compress(values, &mut compressed, num_bits);
///     let mut decompressed = vec![0u32; B::BLOCK_LEN];
///     B::decompress(&compressed[..written], &mut decompressed, num_bits);
///     decompressed
/// }
///
/// let packer = best_available();
/// let values = vec![9u32; 128];
/// let decompressed = round_trip(&packer, &values, 4);
/// assert_eq!(decompressed, values);
/// ```
#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
pub fn best_available() -> impl BitPacker {
    Wasm128
}

/// Returns the fastest [`BitPacker`] implementation available for the current compile
/// target: [`Wasm128`](crate::Wasm128) when compiled for `wasm32` with `simd128` enabled,
/// [`Scalar`] otherwise.
///
/// This is a purely compile-time, `cfg`-driven type selection — WASM engines either
/// support `simd128` for the whole module at compile time or they don't, so there is no
/// runtime feature-detection branch to perform (unlike x86 CPUID checks).
///
/// # Examples
///
/// The concrete type behind the returned `impl BitPacker` can't be named directly, so it's
/// used through a small generic helper, same as this crate's own integration tests do:
///
/// ```
/// use wasm_bitpack::{best_available, BitPacker};
///
/// fn round_trip<B: BitPacker>(packer: &B, values: &[u32], num_bits: u8) -> Vec<u32> {
///     let _ = packer;
///     let mut compressed = vec![0u8; B::BLOCK_LEN * num_bits as usize / 8];
///     let written = B::compress(values, &mut compressed, num_bits);
///     let mut decompressed = vec![0u32; B::BLOCK_LEN];
///     B::decompress(&compressed[..written], &mut decompressed, num_bits);
///     decompressed
/// }
///
/// let packer = best_available();
/// let values = vec![9u32; 128];
/// let decompressed = round_trip(&packer, &values, 4);
/// assert_eq!(decompressed, values);
/// ```
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
pub fn best_available() -> impl BitPacker {
    Scalar
}
