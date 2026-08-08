//! `cargo xtask bench-native` (called internally by `bench-all`) — the native `criterion`
//! sanity baseline, `plans/05-benchmarking-strategy.md` §1 row 1. **Not** the headline claim
//! — see `bench_wasm` for that. Criterion's own statistical output (median, confidence
//! intervals, outlier detection) is captured verbatim rather than re-parsed, since this bench
//! is explicitly secondary and criterion's own report is already trustworthy on its own.

use std::process::Command;

/// Runs the native criterion benches and returns their raw stdout (criterion's own
/// human-readable statistical report, printed as-is into the results doc).
///
/// # Panics
///
/// Panics if `cargo bench` fails to spawn or exits unsuccessfully.
pub fn run() -> String {
    println!("==> running native criterion benches (cargo bench --bench decode_native)");
    let output = Command::new("cargo")
        .args([
            "bench",
            "-p",
            "wasm-bitpack",
            "--bench",
            "decode_native",
            "--features",
            "bench-support",
        ])
        .output()
        .unwrap_or_else(|err| panic!("failed to spawn `cargo bench --bench decode_native`: {err}"));

    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        panic!("`cargo bench --bench decode_native` failed");
    }

    String::from_utf8_lossy(&output.stdout).into_owned()
}
