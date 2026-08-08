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
pub struct Wasm128;

#[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
mod imp {
    use super::Wasm128;
    use crate::traits::BitPacker;

    impl BitPacker for Wasm128 {
        const BLOCK_LEN: usize = 128;

        fn num_bits(decompressed: &[u32]) -> u8 {
            let _ = decompressed;
            todo!("implemented in Phase 2 (see plans/prompts/phase-2-wasm-simd-decode.md)")
        }

        fn compress(decompressed: &[u32], compressed: &mut [u8], num_bits: u8) -> usize {
            let _ = (decompressed, compressed, num_bits);
            todo!("Wasm128 encode is out of scope for v0.1 (see plans/00-overview.md §4)")
        }

        fn decompress(compressed: &[u8], decompressed: &mut [u32], num_bits: u8) -> usize {
            let _ = (compressed, decompressed, num_bits);
            todo!("implemented in Phase 2 (see plans/prompts/phase-2-wasm-simd-decode.md)")
        }
    }
}
