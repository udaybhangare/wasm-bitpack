//! Captures exact toolchain versions and best-effort machine specs for the results doc —
//! `plans/05-benchmarking-strategy.md` §5's anti-cheating rule: "every published number
//! states exact `rustc`, `wasmtime`... versions, plus exact compile flags and profile."

use std::process::Command;

/// `rustc --version`, `cargo --version`, and `wasmtime --version`, each trimmed. Falls back
/// to `"<unavailable: ...>"` rather than panicking — a missing version string shouldn't abort
/// an otherwise-successful benchmark run, it should just be visible as a gap in the report.
pub struct ToolchainVersions {
    pub rustc: String,
    pub cargo: String,
    pub wasmtime: String,
}

pub fn versions() -> ToolchainVersions {
    ToolchainVersions {
        rustc: capture("rustc", &["--version"]),
        cargo: capture("cargo", &["--version"]),
        wasmtime: capture("wasmtime", &["--version"]),
    }
}

/// Best-effort OS/CPU description. Never panics — this is metadata for a report header, not
/// something correctness depends on.
pub fn machine_info() -> String {
    let os = format!("{} ({})", std::env::consts::OS, std::env::consts::ARCH);
    let cpu = cpu_model().unwrap_or_else(|| "unknown".to_string());
    format!("{cpu}, {os}")
}

#[cfg(windows)]
fn cpu_model() -> Option<String> {
    let out = capture(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_Processor).Name",
        ],
    );
    (!out.starts_with("<unavailable")).then_some(out)
}

#[cfg(not(windows))]
fn cpu_model() -> Option<String> {
    let out = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    out.lines()
        .find(|line| line.starts_with("model name"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string())
}

fn capture(program: &str, args: &[&str]) -> String {
    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        Ok(output) => format!("<unavailable: `{program}` exited with {}>", output.status),
        Err(err) => format!("<unavailable: failed to spawn `{program}`: {err}>"),
    }
}
