//! `cargo xtask bench-wasm` — the real comparison (`plans/05-benchmarking-strategy.md` §2):
//! builds the four standalone `wasm32-wasip1` timing binaries
//! (`crates/wasm-bitpack/benches/wasm/*.rs`) and runs each of them under `wasmtime` across
//! the full bit-width × pattern matrix, parsing their structured JSON stdout output.

use crate::cargo_build::build_bench;
use serde::Deserialize;
use std::io::{self, Write as _};
use std::process::Command;

/// One (competitor, bit-width, pattern, size) measurement, deserialized directly from a
/// `wasm_*` binary's JSON stdout line (see `benches/wasm/common.rs::report`).
#[derive(Debug, Clone, Deserialize)]
pub struct BenchResult {
    pub competitor: String,
    pub num_bits: u8,
    pub pattern: String,
    pub size: usize,
    #[allow(dead_code)]
    pub iterations: usize,
    pub median_ns: f64,
    #[allow(dead_code)]
    pub stddev_ns: f64,
    pub elements_per_sec: f64,
    #[allow(dead_code)]
    pub bytes_per_sec: f64,
    pub target_feature_simd128: bool,
}

/// Every `[[bench]]` target name under `crates/wasm-bitpack/benches/wasm/`.
pub const COMPETITORS: [&str; 4] = [
    "wasm_scalar",
    "wasm_wasm128",
    "wasm_bitpacking",
    "wasm_streamvbyte",
];

/// Bit-width spot checks — mirrors `bench_support::BENCH_BIT_WIDTHS`. Kept as a literal here
/// (rather than depending on the library crate from xtask) since xtask is a separate,
/// dependency-light workspace member per `plans/01-architecture.md` §7.
const BIT_WIDTHS: [u8; 9] = [1, 2, 4, 8, 11, 16, 20, 24, 32];
/// Mirrors `bench_support::BenchPattern::ALL`'s `.name()` values.
const PATTERNS: [&str; 3] = ["random", "sorted_ascending", "delta_friendly"];

const WASM_TARGET: &str = "wasm32-wasip1";

/// Builds and runs the full `wasm32-wasip1` benchmark matrix, returning every measurement.
/// `only` restricts to a subset of [`COMPETITORS`] (used to split a full run across multiple
/// invocations — e.g. one per competitor — when a single invocation's wall-clock time is
/// inconvenient; `bench-all` always passes the full [`COMPETITORS`] list).
///
/// # Panics
///
/// Panics if `cargo build` or `wasmtime run` fails, or if a `wasm_*` binary's stdout doesn't
/// parse as the expected JSON result lines.
pub fn run(only: &[&str]) -> Vec<BenchResult> {
    println!("==> building wasm32-wasip1 benchmark binaries");
    println!(
        "    (RUSTFLAGS=\"-C target-feature=+simd128\" via .cargo/config.toml's \
         [target.{WASM_TARGET}] rustflags — see plans/05-benchmarking-strategy.md §5)"
    );
    let executables: Vec<_> = COMPETITORS
        .iter()
        .filter(|name| only.contains(name))
        .map(|&name| (name, build_bench(name, Some(WASM_TARGET))))
        .collect();

    let total = executables.len() * BIT_WIDTHS.len() * PATTERNS.len();
    let mut done = 0usize;
    let mut all_results = Vec::with_capacity(total * 4); // 4 sizes reported per invocation

    for (name, exe) in &executables {
        for &num_bits in &BIT_WIDTHS {
            for &pattern in &PATTERNS {
                done += 1;
                print!(
                    "\r==> wasmtime run [{done}/{total}]: {name} num_bits={num_bits:<2} pattern={pattern:<16}"
                );
                io::stdout().flush().ok();

                let output = Command::new("wasmtime")
                    .args(["run", "--wasm", "simd", "-S", "inherit-network=false"])
                    .arg(exe)
                    .arg(num_bits.to_string())
                    .arg(pattern)
                    .output()
                    .unwrap_or_else(|err| {
                        panic!("failed to spawn `wasmtime run {}`: {err} (is wasmtime installed and on PATH?)", exe.display())
                    });
                if !output.status.success() {
                    eprintln!("\n{}", String::from_utf8_lossy(&output.stderr));
                    panic!(
                        "`wasmtime run {}` failed (num_bits={num_bits}, pattern={pattern})",
                        exe.display()
                    );
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut lines_parsed = 0usize;
                for line in stdout.lines() {
                    match serde_json::from_str::<BenchResult>(line) {
                        Ok(result) => {
                            assert!(
                                result.target_feature_simd128,
                                "simd128 was not enabled for {} — refusing to report a \
                                 silently-scalar-fallback number (plans/05-benchmarking-strategy.md §5)",
                                exe.display()
                            );
                            all_results.push(result);
                            lines_parsed += 1;
                        }
                        Err(_) => eprintln!("\nunparsed line from {}: {line}", exe.display()),
                    }
                }
                assert!(
                    lines_parsed > 0,
                    "`wasmtime run {}` produced no parseable JSON result lines",
                    exe.display()
                );
            }
        }
    }
    println!();

    all_results
}
