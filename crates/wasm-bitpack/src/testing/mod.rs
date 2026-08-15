//! Crate-private testing infrastructure: a naive oracle and shared input generators used by
//! the proptest suite, the equivalence matrix, and the benches.
//!
//! [`bench_gen`] is dependency-free (no `proptest`) so it can back the `bench-support`
//! feature without pulling a dev-dependency into a normal build; it's compiled whenever
//! `std` is actually available (always true under `cfg(test)`, or via `bench-support`,
//! which implies `std`) rather than unconditionally, since its `pub` functions return
//! `Vec<u32>` and would otherwise break a genuine `no_std` build. `gen` and `oracle` stay
//! `#[cfg(test)]`-only, same as before.

#[cfg(any(test, feature = "bench-support"))]
pub(crate) mod bench_gen;
#[cfg(test)]
pub(crate) mod gen;
#[cfg(test)]
pub(crate) mod oracle;
