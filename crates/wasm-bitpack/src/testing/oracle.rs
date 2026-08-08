//! A naive, obviously-correct bit-by-bit reference codec that proptest checks every real
//! implementation against. Never `pub` — this exists purely as a test oracle.

#![allow(dead_code)]

/// Naive bit-by-bit encode: pushes each value's low `num_bits` bits into the output, one
/// value at a time, in order, with no batching or shifting tricks.
pub(crate) fn oracle_encode(values: &[u32], num_bits: u8) -> Vec<u8> {
    let _ = (values, num_bits);
    todo!("implemented in Phase 1 (see plans/prompts/phase-1-scalar-reference.md)")
}

/// Naive bit-by-bit decode, the inverse of [`oracle_encode`].
pub(crate) fn oracle_decode(bytes: &[u8], num_bits: u8, len: usize) -> Vec<u32> {
    let _ = (bytes, num_bits, len);
    todo!("implemented in Phase 1 (see plans/prompts/phase-1-scalar-reference.md)")
}
