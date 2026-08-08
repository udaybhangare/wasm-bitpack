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
