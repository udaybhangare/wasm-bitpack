//! Minimal `wasm-bindgen` bindings for the browser demo page (`demo/browser/index.html`) —
//! a proof artifact for `plans/05-benchmarking-strategy.md` §1 row 3 ("actual browser,
//! `wasm-bindgen` demo page + `performance.now()`... convincing visual proof, not CI-gated or
//! part of the numeric claim"), not a general-purpose JS wrapper. All timing happens on the
//! JS side with `performance.now()` (see `index.html`) — these bindings just do the decode
//! work; `std::time::Instant` isn't available at all on bare `wasm32-unknown-unknown`
//! (no clock, unlike `wasm32-wasip1`'s WASI clock), which is exactly why the browser needs
//! its own timing story separate from `xtask bench-wasm`'s.

use wasm_bindgen::prelude::*;
use wasm_bitpack::{BitPacker, Scalar, Wasm128};

const BLOCK_LEN: usize = 128;

/// Generates `block_count * 128` deterministic demo values (a simple xorshift32 stream, not
/// the crate's proptest/bench RNG — this is a visual demo, not a source of published
/// numbers, so it doesn't need to match `plans/05-benchmarking-strategy.md` §4's methodology)
/// masked to `num_bits`, and packs them with [`Scalar::compress`]. Returns the packed bytes.
///
/// # Panics
///
/// Panics if `num_bits` is `0` or greater than `32` (via [`Scalar::compress`]'s own
/// precondition).
#[wasm_bindgen]
pub fn generate_and_pack(num_bits: u8, block_count: u32) -> Vec<u8> {
    let mask: u32 = if num_bits == 32 {
        u32::MAX
    } else {
        (1u32 << num_bits) - 1
    };
    let mut state: u32 = 0xDE30_5EED_u32; // any fixed nonzero seed
    let total = block_count as usize * BLOCK_LEN;
    let values: Vec<u32> = (0..total)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state & mask
        })
        .collect();

    let mut compressed = vec![0u8; total * 4];
    let mut pos = 0;
    for chunk in values.chunks(BLOCK_LEN) {
        pos += Scalar::compress(chunk, &mut compressed[pos..], num_bits);
    }
    compressed.truncate(pos);
    compressed
}

/// Decodes `block_count` blocks from `compressed` via [`Scalar::decompress`], returning a
/// wrapping checksum of every decoded value (both to give the JS side something to display
/// and to stop the optimizer from treating the decode as dead code — `std::hint::black_box`
/// isn't available across the `wasm-bindgen` FFI boundary the way it is within a single
/// process, so this plays the same role here).
#[wasm_bindgen]
pub fn decode_scalar(compressed: &[u8], block_count: u32, num_bits: u8) -> u32 {
    let mut out = vec![0u32; block_count as usize * BLOCK_LEN];
    let mut in_pos = 0;
    for chunk in out.chunks_mut(BLOCK_LEN) {
        in_pos += Scalar::decompress(&compressed[in_pos..], chunk, num_bits);
    }
    out.iter().fold(0u32, |acc, &v| acc.wrapping_add(v))
}

/// Decodes `block_count` blocks from `compressed` via [`Wasm128::decompress`] — this crate's
/// hand-written SIMD128 decode path, and the whole point of this demo. Only compiles when
/// `simd128` is enabled (see `demo/browser/.cargo/config.toml`); see [`decode_scalar`] for
/// why the return value is a checksum.
#[wasm_bindgen]
pub fn decode_wasm128(compressed: &[u8], block_count: u32, num_bits: u8) -> u32 {
    let mut out = vec![0u32; block_count as usize * BLOCK_LEN];
    let mut in_pos = 0;
    for chunk in out.chunks_mut(BLOCK_LEN) {
        in_pos += Wasm128::decompress(&compressed[in_pos..], chunk, num_bits);
    }
    out.iter().fold(0u32, |acc, &v| acc.wrapping_add(v))
}
