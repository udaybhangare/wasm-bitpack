//! [`compress_sorted`]/[`compress_strictly_sorted`] and their decode-side counterparts —
//! delta-encoded convenience layer for monotonic sequences (sorted IDs, timestamps), which
//! typically pack to a much smaller effective bit width once delta-encoded first.
//!
//! # Wire format: a raw base value, then packed consecutive deltas
//!
//! A naive scheme would delta-encode `decompressed[0]` itself (as "the delta from nothing")
//! and pack it alongside every other delta at one shared `num_bits`. That defeats the whole
//! point for the realistic case this module targets: a sequence of large absolute values
//! (Unix timestamps, database IDs) with small *steps* between them — `decompressed[0]` alone
//! would force `num_bits` up to whatever the largest value needs (often 30+ bits), even
//! though every real delta might fit in 2 or 3.
//!
//! So the two encode functions instead write `decompressed[0]` as a raw 4-byte
//! little-endian header, then delta-encode and pack only `decompressed[1..]` relative to
//! their predecessor. `num_bits_sorted`/`num_bits_strictly_sorted` size against exactly that
//! same consecutive-pair range, so `num_bits` only has to cover genuine step sizes.
//!
//! # Why decode reuses [`unpack`] as-is
//!
//! [`decompress_sorted`]/[`decompress_strictly_sorted`] delegate to [`unpack`] for the
//! delta-unpacking step. On `wasm32` + `simd128` that transparently runs the
//! [`Wasm128`](crate::Wasm128) SIMD decode fast path — a delta value unpacks exactly like any
//! other fixed-width `u32`, since the SIMD kernel has no notion of what the packed values
//! *mean*. Only the final prefix-sum reconstruction (adding each delta back onto a running
//! total) is scalar: it's an inherently sequential dependency chain, each output depending on
//! the one before it, which doesn't parallelize the way independent per-value unpacking does.
//! Encode is scalar-only end to end regardless (it delegates to [`pack`], which delegates to
//! `Scalar::compress` even on `wasm32` + `simd128` — see `plans/00-overview.md` §4: no SIMD
//! encode path exists in v0.1 for any variant, this one included).

use crate::pack::pack;
use crate::scalar::{bits_for_max_value, check_num_bits};
use crate::unpack::unpack;

/// Chunk size used to delta-encode without a heap allocation — matches
/// [`BitPacker::BLOCK_LEN`](crate::BitPacker::BLOCK_LEN) (`128` for every v0.1
/// implementation, per `plans/01-architecture.md` §3). `128` is divisible by `8`, so every
/// full chunk packs to a whole number of bytes (`16 * num_bits`, no leftover bits carried to
/// the next chunk) — packing the post-header delta run this way, one `CHUNK`-sized stack
/// buffer at a time, produces byte-for-byte the same output as delta-encoding the whole
/// slice up front and packing it in a single [`pack`] call.
const CHUNK: usize = 128;

/// `delta = v - prev`, the non-strict ("sorted", duplicates allowed) scheme.
///
/// # Panics
///
/// Panics if `v < prev`.
fn delta_step_sorted(prev: u32, v: u32) -> u32 {
    assert!(
        v >= prev,
        "compress_sorted/num_bits_sorted require non-decreasing input: {v} follows {prev}"
    );
    v - prev
}

/// `delta = v - prev - 1`, the strict ("strictly sorted", no duplicates) scheme — the `- 1`
/// exploits the strict-increase guarantee (`v - prev >= 1`) so a run of consecutive integers
/// packs to `0` bits, one narrower than [`delta_step_sorted`] would need for the same data.
///
/// # Panics
///
/// Panics if `v <= prev`.
fn delta_step_strictly_sorted(prev: u32, v: u32) -> u32 {
    assert!(
        v > prev,
        "compress_strictly_sorted/num_bits_strictly_sorted require strictly increasing \
         input: {v} follows {prev}"
    );
    v - prev - 1
}

/// `v = prev + delta`, the inverse of [`delta_step_sorted`].
fn reconstruct_step_sorted(prev: u32, delta: u32) -> u32 {
    prev.wrapping_add(delta)
}

/// `v = prev + delta + 1`, the inverse of [`delta_step_strictly_sorted`].
fn reconstruct_step_strictly_sorted(prev: u32, delta: u32) -> u32 {
    prev.wrapping_add(delta).wrapping_add(1)
}

/// Shared minimum-bit-width scan over every consecutive pair in `decompressed`, via
/// `delta_step` — no allocation.
fn num_bits_delta_encoded(decompressed: &[u32], delta_step: fn(u32, u32) -> u32) -> u8 {
    let mut max_delta = 0u32;
    for w in decompressed.windows(2) {
        max_delta = max_delta.max(delta_step(w[0], w[1]));
    }
    bits_for_max_value(max_delta)
}

/// Shared header-then-pack encode core: writes `decompressed[0]` as a raw little-endian `u32`
/// header, then delta-encodes and packs `decompressed[1..]` via `delta_step` in `CHUNK`-sized
/// stack buffers (no heap allocation).
fn compress_delta_encoded(
    decompressed: &[u32],
    compressed: &mut [u8],
    num_bits: u8,
    delta_step: fn(u32, u32) -> u32,
) -> usize {
    check_num_bits(num_bits);
    let (&first, rest) = match decompressed.split_first() {
        Some(pair) => pair,
        None => return 0,
    };
    assert!(
        compressed.len() >= 4,
        "compressed buffer too small: need at least 4 bytes for the base-value header, got {}",
        compressed.len()
    );
    compressed[0..4].copy_from_slice(&first.to_le_bytes());

    let mut buf = [0u32; CHUNK];
    let mut prev = first;
    let mut out_pos = 4usize;
    for chunk in rest.chunks(CHUNK) {
        for (slot, &v) in buf.iter_mut().zip(chunk) {
            *slot = delta_step(prev, v);
            prev = v;
        }
        out_pos += pack(&buf[..chunk.len()], &mut compressed[out_pos..], num_bits);
    }
    out_pos
}

/// Shared header-then-unpack decode core: reads the raw little-endian `u32` header back into
/// `decompressed[0]`, then unpacks `decompressed[1..]`'s deltas via [`unpack`] and reconstructs
/// them in place via `reconstruct_step`.
fn decompress_delta_encoded(
    compressed: &[u8],
    decompressed: &mut [u32],
    num_bits: u8,
    reconstruct_step: fn(u32, u32) -> u32,
) -> usize {
    check_num_bits(num_bits);
    let (first_slot, rest) = match decompressed.split_first_mut() {
        Some(pair) => pair,
        None => return 0,
    };
    assert!(
        compressed.len() >= 4,
        "compressed buffer too small: need at least 4 bytes for the base-value header, got {}",
        compressed.len()
    );
    let mut header = [0u8; 4];
    header.copy_from_slice(&compressed[0..4]);
    let first = u32::from_le_bytes(header);
    *first_slot = first;

    let read = unpack(&compressed[4..], rest, num_bits);
    let mut prev = first;
    for slot in rest.iter_mut() {
        let v = reconstruct_step(prev, *slot);
        *slot = v;
        prev = v;
    }
    4 + read
}

/// Returns the minimum number of bits needed to delta-encode `decompressed` for
/// [`compress_sorted`] — the widest consecutive difference in a non-decreasing sequence.
/// `decompressed[0]` itself is excluded (it's stored as a raw header, not delta-encoded — see
/// the module docs), so this reflects genuine step sizes, not `decompressed[0]`'s own
/// magnitude.
///
/// # Panics
///
/// Panics if `decompressed` is not sorted in non-decreasing order (some `decompressed[i] <
/// decompressed[i - 1]`).
pub fn num_bits_sorted(decompressed: &[u32]) -> u8 {
    num_bits_delta_encoded(decompressed, delta_step_sorted)
}

/// Returns the minimum number of bits needed to delta-encode `decompressed` for
/// [`compress_strictly_sorted`] — the widest `consecutive difference - 1` in a strictly
/// increasing sequence. `decompressed[0]` itself is excluded, same as [`num_bits_sorted`].
///
/// # Panics
///
/// Panics if `decompressed` is not strictly increasing (some `decompressed[i] <=
/// decompressed[i - 1]`).
pub fn num_bits_strictly_sorted(decompressed: &[u32]) -> u8 {
    num_bits_delta_encoded(decompressed, delta_step_strictly_sorted)
}

/// Delta-encodes a non-decreasing (duplicates allowed) sequence and packs it into
/// `compressed`, returning the number of bytes written.
///
/// `decompressed[0]` is written as a raw 4-byte little-endian header; `decompressed[1..]` is
/// delta-encoded (`deltas[i] = decompressed[i] - decompressed[i - 1]`) and packed at
/// `num_bits` bits each via [`pack`] — encode stays scalar-only, same as `pack` itself (see
/// the module docs). Use [`num_bits_sorted`] to compute the smallest valid `num_bits`.
///
/// # Panics
///
/// Panics if `decompressed` is not sorted in non-decreasing order, if `compressed` is too
/// small (at least 4 header bytes, plus room for `decompressed.len() - 1` deltas packed at
/// `num_bits` bits each), or if `num_bits` is `0` or greater than `32`.
///
/// # Examples
///
/// ```
/// use wasm_bitpack::{compress_sorted, decompress_sorted, num_bits_sorted};
///
/// // Large absolute values, small steps — exactly the case this module is for.
/// let values = vec![1_700_000_000u32, 1_700_000_012, 1_700_000_012, 1_700_000_015];
/// let num_bits = num_bits_sorted(&values);
/// assert_eq!(num_bits, 4); // widest step is 15, decompressed[0]'s own magnitude is irrelevant
///
/// let mut compressed = vec![0u8; values.len() * 4 + 8];
/// let written = compress_sorted(&values, &mut compressed, num_bits);
///
/// let mut decompressed = vec![0u32; values.len()];
/// let read = decompress_sorted(&compressed[..written], &mut decompressed, num_bits);
/// assert_eq!(read, written);
/// assert_eq!(decompressed, values);
/// ```
pub fn compress_sorted(decompressed: &[u32], compressed: &mut [u8], num_bits: u8) -> usize {
    compress_delta_encoded(decompressed, compressed, num_bits, delta_step_sorted)
}

/// Delta-encodes a strictly increasing sequence and packs it into `compressed`, returning the
/// number of bytes written.
///
/// `decompressed[0]` is written as a raw 4-byte little-endian header; `decompressed[1..]` is
/// delta-encoded (`deltas[i] = decompressed[i] - decompressed[i - 1] - 1`, exploiting the
/// strict-increase guarantee so a run of consecutive integers packs to `0` bits) and packed at
/// `num_bits` bits each via [`pack`]. Use [`num_bits_strictly_sorted`] to compute the smallest
/// valid `num_bits`.
///
/// # Panics
///
/// Panics if `decompressed` is not strictly increasing, if `compressed` is too small (at
/// least 4 header bytes, plus room for `decompressed.len() - 1` deltas packed at `num_bits`
/// bits each), or if `num_bits` is `0` or greater than `32`.
///
/// # Examples
///
/// ```
/// use wasm_bitpack::{
///     compress_strictly_sorted, decompress_strictly_sorted, num_bits_strictly_sorted,
/// };
///
/// let values = vec![1_700_000_000u32, 1_700_000_012, 1_700_000_015, 1_700_000_100];
/// let num_bits = num_bits_strictly_sorted(&values);
///
/// let mut compressed = vec![0u8; values.len() * 4 + 8];
/// let written = compress_strictly_sorted(&values, &mut compressed, num_bits);
///
/// let mut decompressed = vec![0u32; values.len()];
/// let read = decompress_strictly_sorted(&compressed[..written], &mut decompressed, num_bits);
/// assert_eq!(read, written);
/// assert_eq!(decompressed, values);
/// ```
pub fn compress_strictly_sorted(
    decompressed: &[u32],
    compressed: &mut [u8],
    num_bits: u8,
) -> usize {
    compress_delta_encoded(
        decompressed,
        compressed,
        num_bits,
        delta_step_strictly_sorted,
    )
}

/// Unpacks `compressed` (as produced by [`compress_sorted`]) into `decompressed`, returning
/// the number of bytes read.
///
/// Reads the raw 4-byte little-endian header back into `decompressed[0]`, then delegates to
/// [`unpack`] for `decompressed[1..]`'s deltas — on `wasm32` + `simd128` this transparently
/// uses the [`Wasm128`](crate::Wasm128) SIMD decode fast path (see the module docs) — and
/// reconstructs the original values in place with a scalar running-sum pass.
///
/// Does not itself verify that `compressed` was actually produced by [`compress_sorted`]:
/// given arbitrary/corrupted input, the running-sum reconstruction may wrap around `u32`
/// silently rather than panic — well-defined, not undefined behavior, consistent with
/// `unpack`'s own contract (`plans/04-testing-strategy.md` §7).
///
/// # Panics
///
/// Panics if `compressed` is too small (at least 4 header bytes, plus room for
/// `decompressed.len() - 1` deltas packed at `num_bits` bits each), or if `num_bits` is `0`
/// or greater than `32`.
pub fn decompress_sorted(compressed: &[u8], decompressed: &mut [u32], num_bits: u8) -> usize {
    decompress_delta_encoded(compressed, decompressed, num_bits, reconstruct_step_sorted)
}

/// Unpacks `compressed` (as produced by [`compress_strictly_sorted`]) into `decompressed`,
/// returning the number of bytes read.
///
/// Same header-then-[`unpack`] structure as [`decompress_sorted`]; the running-sum
/// reconstruction adds back the `+ 1` the encode side subtracted (`decompressed[i] =
/// decompressed[i - 1] + delta + 1`).
///
/// # Panics
///
/// Panics if `compressed` is too small (at least 4 header bytes, plus room for
/// `decompressed.len() - 1` deltas packed at `num_bits` bits each), or if `num_bits` is `0`
/// or greater than `32`.
pub fn decompress_strictly_sorted(
    compressed: &[u8],
    decompressed: &mut [u32],
    num_bits: u8,
) -> usize {
    decompress_delta_encoded(
        compressed,
        decompressed,
        num_bits,
        reconstruct_step_strictly_sorted,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::gen;
    use proptest::prelude::*;

    #[test]
    fn num_bits_sorted_ignores_base_magnitude() {
        // Large base, tiny steps: this is the whole point of the header split.
        assert_eq!(
            num_bits_sorted(&[1_700_000_000, 1_700_000_003, 1_700_000_007]),
            3
        );
        assert_eq!(num_bits_sorted(&[5]), 0); // single element: no deltas at all
        assert_eq!(num_bits_sorted(&[]), 0);
        assert_eq!(num_bits_sorted(&[7, 7, 7]), 0); // duplicates allowed, all deltas 0
    }

    #[test]
    fn num_bits_strictly_sorted_saves_one_bit_on_consecutive_runs() {
        // 10, 11, 12, 13: every real delta is exactly 1 -> delta-1 == 0 -> 0 bits needed.
        assert_eq!(num_bits_strictly_sorted(&[10, 11, 12, 13]), 0);
        assert_eq!(num_bits_sorted(&[10, 11, 12, 13]), 1); // same data, non-strict scheme needs 1 bit
    }

    #[test]
    fn compress_sorted_empty_and_single_element() {
        let mut compressed = vec![0u8; 16];
        assert_eq!(compress_sorted(&[], &mut compressed, 4), 0);

        let written = compress_sorted(&[42], &mut compressed, 4);
        assert_eq!(written, 4); // just the header, no delta payload

        let mut decompressed = vec![0u32; 1];
        let read = decompress_sorted(&compressed[..written], &mut decompressed, 4);
        assert_eq!(read, written);
        assert_eq!(decompressed, vec![42]);
    }

    #[test]
    fn compress_strictly_sorted_empty_and_single_element() {
        let mut compressed = vec![0u8; 16];
        assert_eq!(compress_strictly_sorted(&[], &mut compressed, 4), 0);

        let written = compress_strictly_sorted(&[42], &mut compressed, 4);
        assert_eq!(written, 4);

        let mut decompressed = vec![0u32; 1];
        let read = decompress_strictly_sorted(&compressed[..written], &mut decompressed, 4);
        assert_eq!(read, written);
        assert_eq!(decompressed, vec![42]);
    }

    #[test]
    fn round_trip_bit_width_boundaries() {
        for num_bits in [1u8, 32] {
            let values: Vec<u32> = (0..50u32).collect();
            let mut compressed = vec![0u8; values.len() * 4 + 8];
            let written = compress_sorted(&values, &mut compressed, num_bits);
            let mut decompressed = vec![0u32; values.len()];
            let read = decompress_sorted(&compressed[..written], &mut decompressed, num_bits);
            assert_eq!(read, written, "num_bits={num_bits}");
            assert_eq!(decompressed, values, "num_bits={num_bits}");
        }
    }

    #[test]
    #[should_panic(expected = "require non-decreasing input")]
    fn compress_sorted_panics_on_unsorted_input() {
        let mut compressed = vec![0u8; 32];
        compress_sorted(&[1, 5, 3], &mut compressed, 4);
    }

    #[test]
    #[should_panic(expected = "require strictly increasing input")]
    fn compress_strictly_sorted_panics_on_duplicate() {
        let mut compressed = vec![0u8; 32];
        compress_strictly_sorted(&[1, 3, 3], &mut compressed, 4);
    }

    #[test]
    #[should_panic(expected = "num_bits must be in 1..=32")]
    fn compress_sorted_panics_on_num_bits_zero() {
        let mut compressed = vec![0u8; 32];
        compress_sorted(&[1, 2, 3], &mut compressed, 0);
    }

    #[test]
    #[should_panic(expected = "num_bits must be in 1..=32")]
    fn compress_sorted_panics_on_num_bits_zero_even_for_single_element() {
        // Regression guard: a single-element input never reaches `pack` (there are no
        // deltas to pack), so `num_bits` validation must happen unconditionally up front,
        // not only when there's actually a delta payload to encode.
        let mut compressed = vec![0u8; 32];
        compress_sorted(&[7], &mut compressed, 0);
    }

    #[test]
    #[should_panic(expected = "need at least 4 bytes for the base-value header")]
    fn compress_sorted_panics_on_undersized_header_buffer() {
        let mut compressed = vec![0u8; 2];
        compress_sorted(&[1, 2, 3], &mut compressed, 4);
    }

    #[test]
    #[should_panic(expected = "compressed buffer too small")]
    fn compress_sorted_panics_on_undersized_payload_buffer() {
        let mut compressed = vec![0u8; 4];
        compress_sorted(&[1, 2, 3], &mut compressed, 32);
    }

    proptest! {
        #![proptest_config(gen::proptest_config(500))]

        #[test]
        fn prop_round_trip_sorted((num_bits, values) in gen::sorted_num_bits_and_values()) {
            let mut compressed = vec![0u8; values.len() * 4 + 8];
            let written = compress_sorted(&values, &mut compressed, num_bits);

            let mut decompressed = vec![0u32; values.len()];
            let read = decompress_sorted(&compressed[..written], &mut decompressed, num_bits);

            prop_assert_eq!(read, written);
            prop_assert_eq!(decompressed, values);
        }

        #[test]
        fn prop_round_trip_strictly_sorted(
            (num_bits, values) in gen::strictly_sorted_num_bits_and_values()
        ) {
            let mut compressed = vec![0u8; values.len() * 4 + 8];
            let written = compress_strictly_sorted(&values, &mut compressed, num_bits);

            let mut decompressed = vec![0u32; values.len()];
            let read =
                decompress_strictly_sorted(&compressed[..written], &mut decompressed, num_bits);

            prop_assert_eq!(read, written);
            prop_assert_eq!(decompressed, values);
        }
    }
}
