//! Crate-private, `#[cfg(test)]`-only testing infrastructure: a naive oracle and shared
//! input generators used by both the proptest suite and the benches.

pub(crate) mod gen;
pub(crate) mod oracle;
