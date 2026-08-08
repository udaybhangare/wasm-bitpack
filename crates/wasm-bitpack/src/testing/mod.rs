//! Crate-private testing infrastructure: a naive oracle and shared input generators used by
//! the proptest suite, the equivalence matrix, and the benches.
//!
//! [`bench_gen`] is compiled unconditionally (dependency-free, no `proptest`) so it can back
//! the `bench-support` feature without pulling a dev-dependency into a normal build; `gen`
//! and `oracle` stay `#[cfg(test)]`-only, same as before.

pub(crate) mod bench_gen;
#[cfg(test)]
pub(crate) mod gen;
#[cfg(test)]
pub(crate) mod oracle;
