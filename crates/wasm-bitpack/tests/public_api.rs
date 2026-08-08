//! Black-box integration tests against only `wasm-bitpack`'s public API — no access to
//! crate-private internals (the naive oracle, `pub(crate)` bit-twiddling helpers, etc.),
//! per `plans/04-testing-strategy.md` §6.

use wasm_bitpack::{best_available, pack, unpack, BitPacker, Scalar};

#[test]
fn scalar_round_trips_an_exact_block() {
    let values: Vec<u32> = (0..Scalar::BLOCK_LEN as u32).collect();
    let num_bits = Scalar::num_bits(&values);

    let mut compressed = vec![0u8; (Scalar::BLOCK_LEN * num_bits as usize + 7) / 8];
    let written = Scalar::compress(&values, &mut compressed, num_bits);

    let mut decompressed = vec![0u32; Scalar::BLOCK_LEN];
    let read = Scalar::decompress(&compressed[..written], &mut decompressed, num_bits);

    assert_eq!(read, written);
    assert_eq!(decompressed, values);
}

#[test]
fn pack_unpack_round_trip_arbitrary_length() {
    let values: Vec<u32> = (0..300u32).map(|i| i % 100).collect();
    let num_bits = 7;

    let mut compressed = vec![0u8; values.len() * 4];
    let written = pack(&values, &mut compressed, num_bits);

    let mut decompressed = vec![0u32; values.len()];
    let read = unpack(&compressed[..written], &mut decompressed, num_bits);

    assert_eq!(read, written);
    assert_eq!(decompressed, values);
}

#[test]
fn pack_unpack_round_trip_empty() {
    let values: Vec<u32> = vec![];
    let mut compressed = vec![0u8; 0];
    let written = pack(&values, &mut compressed, 5);
    assert_eq!(written, 0);

    let mut decompressed: Vec<u32> = vec![];
    let read = unpack(&compressed, &mut decompressed, 5);
    assert_eq!(read, 0);
}

#[test]
fn best_available_round_trips_a_block() {
    // Purely compile-time `cfg`-driven dispatch (see `plans/01-architecture.md` §3):
    // `best_available()` resolves to `Wasm128` only when compiled for wasm32 with
    // `simd128` enabled, and to `Scalar` otherwise. This crate's CI does not currently
    // enable `simd128` for any wasm32 build (that lands with Phase 2's benchmark/CI
    // wiring), so on every target this test actually runs on today, `best_available()`
    // *is* `Scalar` under the hood — exercised here through the opaque `impl BitPacker`
    // value via a small generic helper, since its concrete type can't be named directly.
    let packer = best_available();

    // BLOCK_LEN is documented as 128 for every v0.1 implementation.
    let values: Vec<u32> = (0..Scalar::BLOCK_LEN as u32).map(|i| i % 17).collect();
    let num_bits = 5;

    let mut compressed = vec![0u8; (values.len() * num_bits as usize + 7) / 8];
    let written = compress_with(&packer, &values, &mut compressed, num_bits);

    let mut decompressed = vec![0u32; values.len()];
    let read = decompress_with(&packer, &compressed[..written], &mut decompressed, num_bits);

    assert_eq!(read, written);
    assert_eq!(decompressed, values);
}

fn compress_with<B: BitPacker>(_packer: &B, values: &[u32], out: &mut [u8], num_bits: u8) -> usize {
    B::compress(values, out, num_bits)
}

fn decompress_with<B: BitPacker>(
    _packer: &B,
    bytes: &[u8],
    out: &mut [u32],
    num_bits: u8,
) -> usize {
    B::decompress(bytes, out, num_bits)
}
