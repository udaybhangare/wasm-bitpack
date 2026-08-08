//! Builds a `[[bench]]` target for a given compile target and returns the path to the
//! compiled executable, read from cargo's own `--message-format=json` stream rather than
//! guessing the hashed filename under `target/<...>/deps/`.

use std::path::PathBuf;
use std::process::Command;

/// Runs `cargo build --release -p wasm-bitpack --bench <bench_name> --features bench-support`
/// (optionally cross-compiled to `target`) and returns the resulting executable's path.
///
/// # Panics
///
/// Panics if `cargo` fails to spawn, the build fails, or cargo's JSON output doesn't contain
/// a `compiler-artifact` message with an `executable` field for this bench target.
pub fn build_bench(bench_name: &str, target: Option<&str>) -> PathBuf {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "build",
        "--release",
        "-p",
        "wasm-bitpack",
        "--bench",
        bench_name,
        "--features",
        "bench-support",
        "--message-format=json-render-diagnostics",
    ]);
    if let Some(target) = target {
        cmd.args(["--target", target]);
    }

    let output = cmd
        .output()
        .unwrap_or_else(|err| panic!("failed to spawn `cargo build --bench {bench_name}`: {err}"));
    if !output.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        panic!("`cargo build --bench {bench_name}` failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut executable = None;
    for line in stdout.lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if msg.get("reason").and_then(serde_json::Value::as_str) == Some("compiler-artifact") {
            if let Some(exe) = msg.get("executable").and_then(serde_json::Value::as_str) {
                executable = Some(PathBuf::from(exe));
            }
        }
    }

    executable.unwrap_or_else(|| {
        panic!("`cargo build --bench {bench_name}` produced no executable artifact")
    })
}
