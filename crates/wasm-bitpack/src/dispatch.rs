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
#[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
pub fn best_available() -> impl BitPacker {
    Scalar
}
