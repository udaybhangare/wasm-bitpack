//! Shared proptest/bench input-generation strategies, used by both the correctness test
//! suite and the criterion/xtask benches so both draw from the same input methodology.

#![allow(dead_code)]

use proptest::prelude::*;

/// A `num_bits` strategy, uniform over the full valid range `1..=32`.
pub(crate) fn num_bits_strategy() -> impl Strategy<Value = u8> {
    1u8..=32
}
