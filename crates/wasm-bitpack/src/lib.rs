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

mod dispatch;
mod pack;
mod scalar;
mod traits;
mod unpack;
mod wasm128;

#[cfg(test)]
mod testing;

pub use dispatch::best_available;
pub use pack::pack;
pub use scalar::Scalar;
pub use traits::BitPacker;
pub use unpack::unpack;
pub use wasm128::Wasm128;

#[cfg(test)]
mod scaffold_smoke_test {
    // Phase -1 validation test: confirms `cargo test --target wasm32-wasip1` genuinely
    // executes under wasmtime via the `.cargo/config.toml` runner, not a silent no-op.
    #[test]
    fn arithmetic_sanity() {
        assert_eq!(2 + 2, 4);
    }
}
