//! `wasm-bitpack` is a SIMD-accelerated integer bit-packing and unpacking library with a
//! hand-written WebAssembly SIMD128 decode fast path.
//!
//! Every popular Rust integer-compression crate gets its speed from x86 AVX2/SSE
//! intrinsics and silently falls back to scalar code when compiled to `wasm32`. This crate
//! implements the missing `wasm32` + `simd128` decode fast path instead.
//!
//! See the [project README](https://github.com/udaybhangare/wasm-bitpack) for the full
//! pitch, benchmark methodology, and usage examples.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![deny(missing_docs)]
#![deny(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
// `test` is exempted so `#[cfg(test)]` modules (which use `std`/`Vec` throughout) keep
// compiling unmodified under `--no-default-features`; doctests are unaffected either way
// since they're always compiled as a separate, ordinary std-linked crate.
#![cfg_attr(not(any(feature = "std", test)), no_std)]
// `doc_cfg` is nightly-only; docs.rs builds with a pinned nightly and passes `--cfg docsrs`
// automatically, so this has no effect on a normal stable build.
#![cfg_attr(docsrs, feature(doc_cfg))]

mod dispatch;
mod pack;
mod scalar;
mod sorted;
mod testing;
mod traits;
mod unpack;
mod wasm128;

pub use dispatch::best_available;
pub use pack::pack;
pub use scalar::Scalar;
pub use sorted::{
    compress_sorted, compress_strictly_sorted, decompress_sorted, decompress_strictly_sorted,
    num_bits_sorted, num_bits_strictly_sorted,
};
pub use traits::BitPacker;
pub use unpack::unpack;
pub use wasm128::Wasm128;

/// Benchmark input generation, mirroring exactly what the correctness suite proves against —
/// see `plans/04-testing-strategy.md` §9. **Not part of the locked v0.1 public API**
/// (`plans/02-api-design-rules.md` §2) and not covered by semver: this exists solely so this
/// crate's own `benches/` targets (a separate compilation unit that can only see `pub` items)
/// can reuse the exact generation logic the correctness suite is checked against, instead of
/// a second, potentially-drifting copy. Gated behind the `bench-support` feature, which is
/// off by default and is not meant to be enabled outside this crate's own benchmark targets.
#[cfg(feature = "bench-support")]
#[doc(hidden)]
pub mod bench_support {
    pub use crate::testing::bench_gen::{
        max_value_for_num_bits, random_values, sorted_ascending_values, BenchPattern,
        BENCH_BIT_WIDTHS, BENCH_SEED, BENCH_SIZES,
    };
}

#[cfg(test)]
mod scaffold_smoke_test {
    // Phase -1 validation test: confirms `cargo test --target wasm32-wasip1` genuinely
    // executes under wasmtime via the `.cargo/config.toml` runner, not a silent no-op.
    #[test]
    fn arithmetic_sanity() {
        assert_eq!(2 + 2, 4);
    }
}
