//! A naive, obviously-correct bit-by-bit reference codec that proptest checks every real
//! implementation against. Never `pub` — this exists purely as a test oracle.

#![allow(dead_code)]

/// Naive bit-by-bit encode: pushes each value's low `num_bits` bits into the output, one
/// value at a time, one bit at a time, LSB first, in order. No batching, no shifting
/// tricks beyond "read one bit, write one bit" — this is the ground truth every other
/// implementation in the crate is checked against.
pub(crate) fn oracle_encode(values: &[u32], num_bits: u8) -> Vec<u8> {
    assert!((1..=32).contains(&num_bits), "num_bits must be in 1..=32");

    let mut bits: Vec<bool> = Vec::with_capacity(values.len() * num_bits as usize);
    for &value in values {
        for bit_index in 0..num_bits {
            bits.push((value >> bit_index) & 1 == 1);
        }
    }

    let mut out = vec![0u8; (bits.len() + 7) / 8];
    for (i, bit) in bits.into_iter().enumerate() {
        if bit {
            out[i / 8] |= 1 << (i % 8);
        }
    }
    out
}

/// Naive bit-by-bit decode, the inverse of [`oracle_encode`]: reads `len` values' worth of
/// bits back out one bit at a time, LSB first, in order.
pub(crate) fn oracle_decode(bytes: &[u8], num_bits: u8, len: usize) -> Vec<u32> {
    assert!((1..=32).contains(&num_bits), "num_bits must be in 1..=32");

    let mut out = Vec::with_capacity(len);
    let mut bit_index = 0usize;
    for _ in 0..len {
        let mut value = 0u32;
        for i in 0..num_bits {
            let byte = bytes[bit_index / 8];
            let bit = (byte >> (bit_index % 8)) & 1;
            value |= u32::from(bit) << i;
            bit_index += 1;
        }
        out.push(value);
    }
    out
}

/// Independent ground truth for the 4-way interleaved, `BLOCK_LEN`-exact packed format (see
/// `plans/decisions/0010-bp128-style-packed-format.md`): splits `values` into 4 stride-4
/// sub-streams (lane `j` holds values `j, j+4, ...`), encodes each with [`oracle_encode`],
/// then interleaves the 4 lanes' packed 32-bit words round-robin. Built independently from
/// `scalar::bitpack_interleaved_into` (never calls it) so it's a genuine second
/// implementation to check against, not a restatement of the same logic.
///
/// Requires `values.len()` to be a multiple of `32` (currently only exercised at `128`, the
/// crate's only block length) so every lane's packed length is a whole number of 4-byte words
/// for every `num_bits` in `1..=32` — the same invariant `bitpack_interleaved_into` relies on.
pub(crate) fn oracle_encode_interleaved(values: &[u32], num_bits: u8) -> Vec<u8> {
    const LANES: usize = 4;
    assert_eq!(
        values.len() % 32,
        0,
        "values.len() must be a multiple of 32"
    );
    let lane_len = values.len() / LANES;

    let lanes: Vec<Vec<u8>> = (0..LANES)
        .map(|lane| {
            let lane_values: Vec<u32> = (0..lane_len).map(|i| values[lane + i * LANES]).collect();
            oracle_encode(&lane_values, num_bits)
        })
        .collect();

    let lane_bytes = lanes[0].len();
    let mut out = vec![0u8; lane_bytes * LANES];
    for (lane, packed) in lanes.iter().enumerate() {
        for (word, chunk) in packed.chunks_exact(4).enumerate() {
            let dst = word * LANES * 4 + lane * 4;
            out[dst..dst + 4].copy_from_slice(chunk);
        }
    }
    out
}

/// Inverse of [`oracle_encode_interleaved`], built independently the same way.
pub(crate) fn oracle_decode_interleaved(bytes: &[u8], num_bits: u8, len: usize) -> Vec<u32> {
    const LANES: usize = 4;
    assert_eq!(len % 32, 0, "len must be a multiple of 32");
    let lane_len = len / LANES;
    let lane_bytes = bytes.len() / LANES;

    let mut out = vec![0u32; len];
    for lane in 0..LANES {
        let mut packed = vec![0u8; lane_bytes];
        for (word, chunk) in packed.chunks_exact_mut(4).enumerate() {
            let src = word * LANES * 4 + lane * 4;
            chunk.copy_from_slice(&bytes[src..src + 4]);
        }
        let values = oracle_decode(&packed, num_bits, lane_len);
        for (i, value) in values.into_iter().enumerate() {
            out[lane + i * LANES] = value;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let values = vec![0u32, 1, 2, 3, 4, 5, 6, 7];
        let bytes = oracle_encode(&values, 3);
        let decoded = oracle_decode(&bytes, 3, values.len());
        assert_eq!(decoded, values);
    }

    #[test]
    fn known_encoding_num_bits_3() {
        // Values 0..=7 at 3 bits each: bit stream is
        // 000 001 010 011 100 101 110 111, LSB-first within each byte.
        // Byte 0 (bits 0..8): value0=000, value1=001, value2's low 2 bits=10
        //   -> bit0=0 bit1=0 bit2=0 | bit3=1 bit4=0 bit5=0 | bit6=0 bit7=1
        //   -> 0b1000_1000 = 0x88
        let values = vec![0u32, 1, 2, 3, 4, 5, 6, 7];
        let bytes = oracle_encode(&values, 3);
        assert_eq!(bytes[0], 0x88);
    }

    #[test]
    fn empty_input() {
        assert_eq!(oracle_encode(&[], 5), Vec::<u8>::new());
        assert_eq!(oracle_decode(&[], 5, 0), Vec::<u32>::new());
    }
}
