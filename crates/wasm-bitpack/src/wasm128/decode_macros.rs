//! The `macro_rules!` template that emits all 32 per-bit-width decode routines, plus the
//! table of invocations (see `plans/decisions/0007-simd-codegen-strategy.md` for the
//! rationale and `plans/03-coding-standards.md` §4 for the exact conventions macro
//! invocations and any hand-written exceptions must follow).
//!
//! ## Algorithm
//!
//! [`Scalar`](crate::Scalar) packs a `BLOCK_LEN`-exact (128-value) block using the 4-way
//! interleaved BP128-style layout documented in
//! `plans/decisions/0010-bp128-style-packed-format.md`: the block is split into 4 stride-4
//! sub-streams (lane `j` holds values `j, j+4, ..., j+124`, 32 values each), each lane is
//! bit-packed independently at `num_bits` bits per value, and the 4 lanes' packed 32-bit
//! words are interleaved round-robin — `compressed`'s first 16 bytes are lane 0..3's word 0,
//! the next 16 bytes are lane 0..3's word 1, and so on, for `num_bits` words total
//! (`16 * num_bits` bytes, i.e. `packed_len_bytes(BLOCK_LEN, num_bits)`).
//!
//! Because all 4 lanes are always at the *same* relative bit position within their own
//! sub-stream at any given "row" (decoded value index within a lane), a single `v128_load`
//! of one 16-byte word gives all 4 lanes' bits for that row at once, and every lane needs the
//! *same* shift amount to extract its field — no per-lane-variable shift, and therefore no
//! multiply-as-shift trick, is ever needed (unlike the crate's first-generation kernel, which
//! paired *adjacent* values into one register and paid for an `i64x2_mul` every iteration to
//! fake a per-lane-divergent shift). Each generated `decode_{n}bit` routine:
//!
//! 1. Loads the first 16-byte word (`row 0`'s bits for all 4 lanes) into a working register.
//! 2. For each `row` in `1..32`: right-shifts the working register by `(row * num_bits) % 32`
//!    bits (the same shift for all 4 lanes), masks to `num_bits` bits, and stores the result —
//!    this is `row`'s decoded value for all 4 lanes at once, i.e. `decompressed[4*row..4*row+4]`.
//! 3. Whenever the current word's remaining capacity is exhausted (`inner_capacity <=
//!    num_bits`), loads the next 16-byte word before computing `row`'s output; if `row`'s
//!    field actually straddles the two words (`inner_capacity < num_bits`), left-shifts the
//!    newly loaded word by `inner_capacity` and ORs it into the output.
//!
//! This is a direct WASM SIMD128 port of the classic Lemire BP128 scheme
//! `bitpacking::BitPacker4x` itself uses (see ADR 0010) — ported from `bitpacking`'s own
//! `macros.rs` `unpack` routine, substituting WASM's *native* runtime-operand shifts
//! (`u32x4_shr`/`u32x4_shl` — WASM's shift instructions take a stack operand, not an encoded
//! immediate like x86/ARM, so no compile-time unrolling is required just to get a real shift
//! instruction) for x86's `_mm_srli_epi32::<N>`/`_mm_slli_epi32::<N>`.
//!
//! Every load and store reads/writes directly against the caller's `compressed`/`decompressed`
//! slices — no zero-padded scratch buffer is needed (unlike the first-generation kernel): the
//! highest byte ever read is `16 * (num_bits - 1)`, and reading 16 bytes from there lands at
//! exactly `16 * num_bits`, so every load is in-bounds given `compressed.len() >= 16 *
//! num_bits`. See the `# Safety` doc on each generated function for the exact bound.
//!
//! ## Where the `unsafe` actually is
//!
//! `core::arch::wasm32`'s pure register arithmetic (`u32x4_shr`, `u32x4_shl`, `v128_and`,
//! `v128_or`, `u32x4_splat`) is `#[target_feature(enable = "simd128")]`-gated but *not*
//! `unsafe fn` — with `simd128` enabled unconditionally for this whole compilation (see
//! `.cargo/config.toml`'s `rustflags`), the compiler treats calling them as ordinary safe
//! function calls (confirmed by `unused_unsafe` firing if they're wrapped in `unsafe { }`).
//! `v128_load`/`v128_store` are different: they're declared `unsafe fn` directly, because they
//! dereference a raw pointer, which is unsafe regardless of target-feature configuration.
//! Those calls are this module's only real unsafety, and are exactly what each generated
//! function's `# Safety` doc is about.

use core::arch::wasm32::{
    u32x4_shl, u32x4_shr, u32x4_splat, v128, v128_and, v128_load, v128_or, v128_store,
};

/// Number of `u32` values a single block holds. Must match
/// [`BitPacker::BLOCK_LEN`](crate::traits::BitPacker::BLOCK_LEN) for [`Wasm128`](crate::Wasm128);
/// kept as a local constant so this module has no dependency on the trait.
const BLOCK_LEN: usize = 128;

/// Number of interleaved lanes the packed format splits a block into — must match
/// `scalar::LANES` (kept as its own local constant for the same reason as `BLOCK_LEN`: no
/// cross-module dependency for a value this module only uses to derive [`ROWS`]).
const LANES: usize = 4;

/// Number of decoded values per lane, i.e. per-block loop iterations: one `v128` register
/// holds one row (one value from each of the 4 lanes) at a time.
const ROWS: usize = BLOCK_LEN / LANES;

/// Emits one `pub(crate) unsafe fn decode_{n}bit` following the algorithm documented at the
/// top of this file, specialized (monomorphized, not runtime-branching) for the literal
/// bit-width `$bits`. Covers `1..=31`; `32` is hand-written separately below since it needs
/// no shift/mask/straddle handling at all (see [`decode_32bit`]).
macro_rules! decode_bitwidth {
    ($name:ident, $bits:literal) => {
        #[doc = concat!(
                            "Decodes `BLOCK_LEN` (128) values packed at ",
                            stringify!($bits),
                            " bit(s) each, matching [`Scalar`](crate::Scalar)'s 4-way interleaved ",
                            "layout exactly. See the [module-level algorithm docs](self) for how."
                        )]
        ///
        /// # Safety
        ///
        /// This function's only unsafe operations are `v128_load`/`v128_store` calls, each
        /// reading or writing 16 bytes directly against `compressed`/`decompressed` — no
        /// padding or scratch buffer is used. Every load's highest byte offset is
        /// `16 * (NUM_BITS - 1)`, and reading 16 bytes from there lands at exactly
        /// `16 * NUM_BITS` (`= packed_len_bytes(BLOCK_LEN, NUM_BITS)`), so every load is
        /// in-bounds given `compressed.len() >= 16 * NUM_BITS`. Every store writes to
        /// `decompressed[4 * row..4 * row + 4]` for `row` in `0..32`, so every store is
        /// in-bounds given `decompressed.len() == BLOCK_LEN` (128). This function is marked
        /// `unsafe` because of those raw-pointer reads/writes, not because of an additional
        /// obligation beyond the two length checks above — both already asserted by the
        /// caller ([`Wasm128::decompress`](crate::wasm128::Wasm128)) before this is called.
        #[allow(clippy::cast_possible_truncation, clippy::cast_ptr_alignment)]
        // `row`/`inner_cursor`/`inner_capacity` are always < 32 (bounded by `% 32`/`32 - x`
        // arithmetic), so casting them to `u32` for the shift intrinsics never truncates; the
        // `u64 -> u32` mask cast is exact because `NUM_BITS <= 32`. The `*const u8 -> *const
        // v128` casts are sound despite the alignment increase: `v128_load`/`v128_store` are
        // documented to perform 1-aligned (i.e. unaligned) loads/stores internally, so an
        // under-aligned pointer is exactly what they expect, not a bug.
        pub(crate) unsafe fn $name(compressed: &[u8], decompressed: &mut [u32]) {
            const NUM_BITS: usize = $bits;
            const MASK: u32 = ((1u64 << NUM_BITS) - 1) as u32;

            let mask_v = u32x4_splat(MASK);

            // SAFETY: `16 * NUM_BITS <= compressed.len()` — see this function's `# Safety`.
            // This is word 0, the lowest (and always in-bounds) 16-byte window.
            let mut in_register = unsafe { v128_load(compressed.as_ptr().cast::<v128>()) };
            let mut next_word = 1usize;

            // SAFETY: `decompressed.len() == BLOCK_LEN` — see this function's `# Safety`.
            // `decompressed[0..4]` is row 0, always in-bounds.
            unsafe {
                v128_store(
                    decompressed[0..4].as_mut_ptr().cast::<v128>(),
                    v128_and(in_register, mask_v),
                );
            }

            for row in 1..ROWS {
                let inner_cursor = (row * NUM_BITS) % 32;
                let inner_capacity = 32 - inner_cursor;

                let shifted = if inner_cursor == 0 {
                    in_register
                } else {
                    u32x4_shr(in_register, inner_cursor as u32)
                };
                let mut out = v128_and(shifted, mask_v);

                // We consumed the current word entirely. Read the next one.
                if inner_capacity <= NUM_BITS && row != ROWS - 1 {
                    // SAFETY: `next_word < NUM_BITS` whenever this branch runs (the reload
                    // trigger `inner_capacity <= NUM_BITS` together with `row != 31` never
                    // fires for the word past the last one — see this function's `# Safety`
                    // and the module-level algorithm docs), so `16 * next_word + 16 <=
                    // 16 * NUM_BITS <= compressed.len()`.
                    in_register =
                        unsafe { v128_load(compressed[16 * next_word..].as_ptr().cast::<v128>()) };
                    next_word += 1;

                    // This row's field actually straddles the two words — fold in the low
                    // bits of the newly loaded word.
                    if inner_capacity < NUM_BITS {
                        let filled =
                            v128_and(u32x4_shl(in_register, inner_capacity as u32), mask_v);
                        out = v128_or(out, filled);
                    }
                }

                // SAFETY: `decompressed.len() == BLOCK_LEN` (128) and `4 * row + 4 <= 128` for
                // every `row` in `1..32` — see this function's `# Safety`.
                unsafe {
                    v128_store(
                        decompressed[4 * row..4 * row + 4]
                            .as_mut_ptr()
                            .cast::<v128>(),
                        out,
                    );
                }
            }
        }
    };
}

// Every bit-width 1..=31, in one place so the full range is visually auditable at a glance
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

/// Decodes `BLOCK_LEN` (128) values packed at 32 bits each. Hand-written rather than emitted
/// by [`decode_bitwidth!`] because at `num_bits == 32` every lane's word *is* its value — no
/// shifting, masking, or straddle-handling needed at all, just 32 loads and 32 stores. See the
/// [module-level algorithm docs](self).
///
/// # Safety
///
/// Reads `compressed[16 * row..16 * row + 16]` and writes `decompressed[4 * row..4 * row + 4]`
/// for every `row` in `0..ROWS` (32) — in-bounds given `compressed.len() >= 512`
/// (`packed_len_bytes(BLOCK_LEN, 32)`) and `decompressed.len() == BLOCK_LEN` (128), both
/// already asserted by the caller ([`Wasm128::decompress`](crate::wasm128::Wasm128)) before
/// this is called.
#[allow(clippy::cast_ptr_alignment)]
// `v128_load`/`v128_store` perform 1-aligned (unaligned) loads/stores internally, so casting
// an under-aligned `*const u8`/`*mut u32` to `*const v128`/`*mut v128` is exactly what they
// expect, not a bug.
pub(crate) unsafe fn decode_32bit(compressed: &[u8], decompressed: &mut [u32]) {
    for row in 0..ROWS {
        // SAFETY: see this function's `# Safety`.
        let v = unsafe { v128_load(compressed[16 * row..].as_ptr().cast::<v128>()) };
        // SAFETY: see this function's `# Safety`.
        unsafe {
            v128_store(
                decompressed[4 * row..4 * row + 4]
                    .as_mut_ptr()
                    .cast::<v128>(),
                v,
            );
        }
    }
}

/// Dispatch table from a runtime `num_bits` (already validated to be in `1..=32`) to the
/// matching monomorphized decode routine. The only place any of these 32 functions are
/// called from — never exposed outside this module.
///
/// # Safety
///
/// Same precondition as the selected `decode_{n}bit`/`decode_32bit` (see its `# Safety` doc)
/// — none of this function's own code is unsafe, it only forwards to one of the 32 above.
pub(crate) unsafe fn decode(num_bits: u8, compressed: &[u8], decompressed: &mut [u32]) {
    // SAFETY: each arm forwards this function's own precondition unchanged to the matching
    // `decode_{n}bit`/`decode_32bit`, whose required bit-width exactly matches the arm's
    // `num_bits` value.
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
