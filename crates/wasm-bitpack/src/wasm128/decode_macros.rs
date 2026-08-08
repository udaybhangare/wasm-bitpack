//! The `macro_rules!` template that emits all 32 per-bit-width decode routines, plus the
//! table of invocations (see `plans/decisions/0007-simd-codegen-strategy.md` for the
//! rationale and `plans/03-coding-standards.md` §4 for the exact conventions macro
//! invocations and any hand-written exceptions must follow).
//!
//! ## Algorithm
//!
//! [`Scalar`](crate::Scalar) packs `BLOCK_LEN` (128) values into a single LSB-first
//! bitstream: value `i` occupies bits `[i * num_bits, (i + 1) * num_bits)` of the packed
//! byte buffer, counting from the least-significant bit of byte 0. Each generated
//! `decode_{n}bit` routine reproduces that exact layout, two values at a time, using a
//! 2-lane `i64x2` register:
//!
//! 1. For a pair of values `(i, i + 1)`, `v128_load64_zero`/`v128_load64_lane` read an
//!    8-byte little-endian window starting at each value's byte offset
//!    (`floor(i * num_bits / 8)`) into the two lanes of one `v128`. A value's bits can span
//!    at most 5 bytes (`num_bits` up to 32, plus up to 7 bits of intra-byte offset), so an
//!    8-byte window always contains the whole field.
//! 2. Left-shift each lane *independently* so both values' bit windows land at the same
//!    position (`[64 - num_bits, 64)`), via a per-lane multiply by a compile-time-computed
//!    power of two (`i64x2_mul`) — `core::arch::wasm32` has no per-lane-variable-shift
//!    instruction, so multiplying by `2^k` stands in for `<< k`, which is exact under
//!    wrapping arithmetic (multiplying by a power of two never produces carries).
//! 3. Shift both lanes right by the same amount (`64 - num_bits`, `u64x2_shr`) to bring the
//!    extracted field down to `[0, num_bits)`, zero-filling above it.
//! 4. Mask with `v128_and` (redundant given step 3's zero-fill, kept as cheap defensive
//!    insurance) and extract both lanes.
//!
//! Every load reads from a local, zero-padded stack copy of the input block — never
//! directly from the caller's `compressed` slice — so that reading 8 bytes starting at the
//! *last* value's byte offset can never read past a buffer boundary. See the `# Safety` doc
//! on each generated function for the exact bound.
//!
//! ## Where the `unsafe` actually is
//!
//! `core::arch::wasm32`'s pure register arithmetic (`i64x2`, `i64x2_mul`, `u64x2_shr`,
//! `v128_and`, `i64x2_extract_lane`) is `#[target_feature(enable = "simd128")]`-gated but
//! *not* `unsafe fn` — with `simd128` enabled unconditionally for this whole compilation
//! (see `.cargo/config.toml`'s `rustflags`), the compiler treats calling them as ordinary
//! safe function calls (confirmed by `unused_unsafe` firing if they're wrapped in `unsafe {
//! }`). The two loads (`v128_load64_zero`, `v128_load64_lane`) are different: they're
//! declared `unsafe fn` directly, because they dereference a raw pointer, which is unsafe
//! regardless of target-feature configuration. Those two calls are this module's only real
//! unsafety, and are exactly what each generated function's `# Safety` doc is about.

use core::arch::wasm32::{
    i64x2, i64x2_extract_lane, i64x2_mul, u64x2_shr, v128_and, v128_load64_lane, v128_load64_zero,
};

/// Number of `u32` values a single block holds. Must match
/// [`BitPacker::BLOCK_LEN`](crate::traits::BitPacker::BLOCK_LEN) for [`Wasm128`](crate::Wasm128);
/// kept as a local constant so this module has no dependency on the trait.
const BLOCK_LEN: usize = 128;

/// Emits one `pub(crate) unsafe fn decode_{n}bit` following the algorithm documented at the
/// top of this file, specialized (monomorphized, not runtime-branching) for the literal
/// bit-width `$bits`.
macro_rules! decode_bitwidth {
    ($name:ident, $bits:literal) => {
        #[doc = concat!(
                    "Decodes `BLOCK_LEN` (128) values packed at ",
                    stringify!($bits),
                    " bit(s) each, matching [`Scalar`](crate::Scalar)'s LSB-first bit layout ",
                    "exactly. See the [module-level algorithm docs](self) for how."
                )]
        ///
        /// # Safety
        ///
        /// This function's only unsafe operations are two `v128_load64_zero`/
        /// `v128_load64_lane` calls per pair of decoded values, each reading 8 bytes from a
        /// local, fixed-size, zero-padded buffer (`buf`) that this function allocates and
        /// fills itself. Every such read is proven in-bounds by construction — the highest
        /// offset ever read is `floor(127 * bits / 8)`, which is strictly less than
        /// `buf`'s `BLOCK_LEN * bits / 8` live-data length, so reading 8 further bytes
        /// always lands inside `buf`'s extra 8 bytes of zero padding (see the module docs)
        /// — regardless of what `compressed`/`decompressed` actually contain. This function
        /// is marked `unsafe` because of those raw-pointer reads, not because of an
        /// additional obligation on the caller: passing a `compressed` shorter than
        /// `BLOCK_LEN * bits / 8` bytes, or a `decompressed` shorter than `BLOCK_LEN`,
        /// causes an ordinary bounds-checked panic (from the initial copy, or from indexing
        /// into `decompressed`), not undefined behavior.
        #[allow(
            clippy::cast_possible_wrap,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_ptr_alignment
        )] // the `*const u8` -> `*const u64` casts are sound despite the alignment increase:
           // `v128_load64_zero`/`v128_load64_lane` are documented to perform a 1-aligned
           // (i.e. unaligned) load internally, so an under-aligned `*const u64` is exactly
           // what they expect, not a bug. Every other cast below reinterprets bits already
           // proven to fit (NUM_BITS <= 32, an extracted field is always < 2^NUM_BITS <=
           // 2^32) or converts between the signed `i64` lanes `core::arch::wasm32`'s API
           // requires and this module's unsigned
           // arithmetic — never a truncating or sign-changing loss of information.
        pub(crate) unsafe fn $name(compressed: &[u8], decompressed: &mut [u32]) {
            const NUM_BITS: usize = $bits;
            const PACKED_LEN: usize = BLOCK_LEN * NUM_BITS / 8;
            const MASK: u64 = (1u64 << NUM_BITS) - 1;

            // Local zero-padded copy: every window read below is 8 bytes starting at most
            // `floor(127 * NUM_BITS / 8)` bytes in, which is < PACKED_LEN, so
            // `PACKED_LEN + 8` bytes always keeps every read in-bounds (see module docs).
            let mut buf = [0u8; PACKED_LEN + 8];
            buf[..PACKED_LEN].copy_from_slice(&compressed[..PACKED_LEN]);

            let mask_v = i64x2(MASK as i64, MASK as i64);

            let mut i = 0usize;
            while i < BLOCK_LEN {
                let bit_a = i * NUM_BITS;
                let bit_b = (i + 1) * NUM_BITS;
                let byte_off_a = bit_a / 8;
                let byte_off_b = bit_b / 8;
                let shift_a = (bit_a % 8) as i64;
                let shift_b = (bit_b % 8) as i64;

                // SAFETY: `byte_off_a + 8 <= buf.len()` — see this function's `# Safety`.
                // `buf[byte_off_a..]`'s pointer needs no alignment (`v128_load64_zero`
                // performs a 1-aligned/unaligned load).
                let v0 = unsafe { v128_load64_zero(buf[byte_off_a..].as_ptr().cast::<u64>()) };
                // SAFETY: `byte_off_b + 8 <= buf.len()` — see this function's `# Safety`.
                // Same no-alignment-required load, placed into lane 1 of `v0`.
                let v =
                    unsafe { v128_load64_lane::<1>(v0, buf[byte_off_b..].as_ptr().cast::<u64>()) };

                // Left-shift amount that moves each lane's `num_bits`-wide field to land at
                // `[64 - NUM_BITS, 64)`: `shift + left == 64 - NUM_BITS` for both lanes.
                // Always in `1..64` since `shift` is in `0..8` and `NUM_BITS` in `1..=32`,
                // so `1i64 << left` is always well-defined (never a shift-amount overflow).
                let left_a = 64 - NUM_BITS as i64 - shift_a;
                let left_b = 64 - NUM_BITS as i64 - shift_b;
                let mul_v = i64x2(1i64 << left_a, 1i64 << left_b);

                // Per-lane wrapping multiply by a power of two is exactly a per-lane left
                // shift with high-bit truncation — this is what moves each lane's target
                // field to a common position without a per-lane variable-shift instruction
                // (`core::arch::wasm32` doesn't have one).
                let shifted = i64x2_mul(v, mul_v);
                // A single right-shift amount is valid for both lanes because the multiply
                // above aligned both fields to the same `[64 - NUM_BITS, 64)` window.
                let aligned = u64x2_shr(shifted, (64 - NUM_BITS) as u32);
                // Defensive mask, redundant with the shift above's zero-fill but cheap
                // insurance against an alignment mistake in the reasoning above.
                let masked = v128_and(aligned, mask_v);

                decompressed[i] = i64x2_extract_lane::<0>(masked) as u32;
                decompressed[i + 1] = i64x2_extract_lane::<1>(masked) as u32;

                i += 2;
            }
        }
    };
}

// Every bit-width 1..=32, in one place so the full range is visually auditable at a glance
// (per `plans/03-coding-standards.md` §4). None currently need a hand-written exception.
decode_bitwidth!(decode_1bit, 1);
decode_bitwidth!(decode_2bit, 2);
decode_bitwidth!(decode_3bit, 3);
decode_bitwidth!(decode_4bit, 4);
decode_bitwidth!(decode_5bit, 5);
decode_bitwidth!(decode_6bit, 6);
decode_bitwidth!(decode_7bit, 7);
decode_bitwidth!(decode_8bit, 8);
decode_bitwidth!(decode_9bit, 9);
decode_bitwidth!(decode_10bit, 10);
decode_bitwidth!(decode_11bit, 11);
decode_bitwidth!(decode_12bit, 12);
decode_bitwidth!(decode_13bit, 13);
decode_bitwidth!(decode_14bit, 14);
decode_bitwidth!(decode_15bit, 15);
decode_bitwidth!(decode_16bit, 16);
decode_bitwidth!(decode_17bit, 17);
decode_bitwidth!(decode_18bit, 18);
decode_bitwidth!(decode_19bit, 19);
decode_bitwidth!(decode_20bit, 20);
decode_bitwidth!(decode_21bit, 21);
decode_bitwidth!(decode_22bit, 22);
decode_bitwidth!(decode_23bit, 23);
decode_bitwidth!(decode_24bit, 24);
decode_bitwidth!(decode_25bit, 25);
decode_bitwidth!(decode_26bit, 26);
decode_bitwidth!(decode_27bit, 27);
decode_bitwidth!(decode_28bit, 28);
decode_bitwidth!(decode_29bit, 29);
decode_bitwidth!(decode_30bit, 30);
decode_bitwidth!(decode_31bit, 31);
decode_bitwidth!(decode_32bit, 32);

/// Dispatch table from a runtime `num_bits` (already validated to be in `1..=32`) to the
/// matching monomorphized decode routine. The only place any of these 32 functions are
/// called from — never exposed outside this module.
///
/// # Safety
///
/// Same precondition as the selected `decode_{n}bit` (see its `# Safety` doc) — none of
/// this function's own code is unsafe, it only forwards to one of the 32 above.
pub(crate) unsafe fn decode(num_bits: u8, compressed: &[u8], decompressed: &mut [u32]) {
    // SAFETY: each arm forwards this function's own precondition unchanged to the matching
    // `decode_{n}bit`, whose required bit-width exactly matches the arm's `num_bits` value.
    unsafe {
        match num_bits {
            1 => decode_1bit(compressed, decompressed),
            2 => decode_2bit(compressed, decompressed),
            3 => decode_3bit(compressed, decompressed),
            4 => decode_4bit(compressed, decompressed),
            5 => decode_5bit(compressed, decompressed),
            6 => decode_6bit(compressed, decompressed),
            7 => decode_7bit(compressed, decompressed),
            8 => decode_8bit(compressed, decompressed),
            9 => decode_9bit(compressed, decompressed),
            10 => decode_10bit(compressed, decompressed),
            11 => decode_11bit(compressed, decompressed),
            12 => decode_12bit(compressed, decompressed),
            13 => decode_13bit(compressed, decompressed),
            14 => decode_14bit(compressed, decompressed),
            15 => decode_15bit(compressed, decompressed),
            16 => decode_16bit(compressed, decompressed),
            17 => decode_17bit(compressed, decompressed),
            18 => decode_18bit(compressed, decompressed),
            19 => decode_19bit(compressed, decompressed),
            20 => decode_20bit(compressed, decompressed),
            21 => decode_21bit(compressed, decompressed),
            22 => decode_22bit(compressed, decompressed),
            23 => decode_23bit(compressed, decompressed),
            24 => decode_24bit(compressed, decompressed),
            25 => decode_25bit(compressed, decompressed),
            26 => decode_26bit(compressed, decompressed),
            27 => decode_27bit(compressed, decompressed),
            28 => decode_28bit(compressed, decompressed),
            29 => decode_29bit(compressed, decompressed),
            30 => decode_30bit(compressed, decompressed),
            31 => decode_31bit(compressed, decompressed),
            32 => decode_32bit(compressed, decompressed),
            _ => unreachable!("num_bits must be in 1..=32, checked by the caller"),
        }
    }
}
