//! Writes `plans/results/YYYY-MM-DD-<description>.md` (per
//! `plans/05-benchmarking-strategy.md` §6: full input matrix, raw numbers, machine specs,
//! toolchain versions, and the ≥3x claim's actual measured multiplier — honestly reported,
//! never adjusted after the fact to manufacture a passing number,
//! `plans/00-overview.md` §6 / `plans/05-benchmarking-strategy.md` §6's hard rule) and the
//! root `BENCHMARKS.md` summary.
//!
//! `plans/` is git-ignored entirely (local-only working notes — see
//! `plans/decisions/0009-plans-dir-not-pushed.md` and the root `CLAUDE.md`), so
//! `plans/results/*.md` is never something a GitHub visitor can click into. `BENCHMARKS.md`,
//! by contrast, **is** committed (`plans/01-architecture.md` §1 lists it at repo root
//! alongside `README.md`), so it must be fully self-contained — the actual numbers inlined,
//! never a link into `plans/results/` that would 404 for anyone browsing GitHub or cloning
//! fresh.

use crate::bench_wasm::BenchResult;
use crate::toolchain::ToolchainVersions;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const STEADY_STATE_SIZE: usize = 10_000_000;
/// Purely for readability in prose/headings — `{STEADY_STATE_SIZE}` on its own formats
/// without thousands separators.
const STEADY_STATE_SIZE_FMT: &str = "10,000,000";

pub struct ReportInputs {
    pub native_output: String,
    pub wasm_results: Vec<BenchResult>,
    pub versions: ToolchainVersions,
    pub machine: String,
}

/// The headline-claim computation, shared by `plans/results/*.md` and `BENCHMARKS.md` so the
/// two documents can never quote different numbers for the same run.
struct Summary {
    rows: String,
    ratios_count: usize,
    overall_multiplier: Option<f64>,
    min_ratio: f64,
    max_ratio: f64,
}

impl Summary {
    fn compute(results: &[BenchResult]) -> Self {
        let wasm128 = ratio_at_steady_state(results, "wasm_bitpack_wasm128");
        let bitpacking = ratio_at_steady_state(results, "bitpacking_bitpacker4x");
        let scalar = ratio_at_steady_state(results, "wasm_bitpack_scalar");

        let mut ratios = Vec::new();
        let mut rows = String::new();
        for &(pattern, num_bits, w128) in &wasm128 {
            let bp = bitpacking
                .iter()
                .find(|(p, n, _)| *p == pattern && *n == num_bits)
                .map(|(_, _, v)| *v);
            let sc = scalar
                .iter()
                .find(|(p, n, _)| *p == pattern && *n == num_bits)
                .map(|(_, _, v)| *v);
            let ratio_bp = bp.map(|bp| w128 / bp);
            let ratio_sc = sc.map(|sc| w128 / sc);
            if let Some(r) = ratio_bp {
                ratios.push(r);
            }
            rows.push_str(&format!(
                "| {pattern} | {num_bits} | {:.1} | {} | {} | {} | {} |\n",
                w128 / 1.0e6,
                fmt_rate(bp),
                fmt_rate(sc),
                fmt_ratio(ratio_bp),
                fmt_ratio(ratio_sc),
            ));
        }

        let overall_multiplier = (!ratios.is_empty()).then(|| median(ratios.clone()));
        let min_ratio = ratios.iter().copied().fold(f64::INFINITY, f64::min);
        let max_ratio = ratios.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        Self {
            rows,
            ratios_count: ratios.len(),
            overall_multiplier,
            min_ratio,
            max_ratio,
        }
    }

    fn verdict(&self) -> String {
        match self.overall_multiplier {
            Some(m) if m >= 3.0 => format!(
                "**CONFIRMED** — median {m:.2}x decode throughput vs `bitpacking::BitPacker4x` \
                 at steady state ({STEADY_STATE_SIZE_FMT} elements), across all {} (bit-width, \
                 pattern) points measured (range {:.2}x-{:.2}x).",
                self.ratios_count, self.min_ratio, self.max_ratio
            ),
            Some(m) => format!(
                "**NOT MET** — median {m:.2}x decode throughput vs `bitpacking::BitPacker4x` at \
                 steady state ({STEADY_STATE_SIZE_FMT} elements), across all {} (bit-width, \
                 pattern) points measured (range {:.2}x-{:.2}x). This is the honest measured \
                 number; methodology was not adjusted to manufacture a passing figure \
                 (`plans/05-benchmarking-strategy.md` §6).",
                self.ratios_count, self.min_ratio, self.max_ratio
            ),
            None => "**INCONCLUSIVE** — no matching wasm_bitpack_wasm128/bitpacking_bitpacker4x \
                      steady-state measurements were found."
                .to_string(),
        }
    }
}

/// Formats a raw `elements_per_sec` value (or its absence) as millions/sec, matching every
/// other rate column in these reports.
fn fmt_rate(v: Option<f64>) -> String {
    v.map_or_else(|| "-".to_string(), |v| format!("{:.1}", v / 1.0e6))
}

fn fmt_ratio(v: Option<f64>) -> String {
    v.map_or_else(|| "-".to_string(), |v| format!("{v:.2}x"))
}

/// Writes the dated results markdown + raw JSON into `plans/results/`, and regenerates the
/// root `BENCHMARKS.md`. Returns the `plans/results/` markdown file's path.
///
/// # Panics
///
/// Panics if `plans/results/` can't be created or the files can't be written.
pub fn write_report(inputs: &ReportInputs) -> PathBuf {
    let date = today();
    let summary = Summary::compute(&inputs.wasm_results);

    let dir = PathBuf::from("plans/results");
    fs::create_dir_all(&dir).expect("failed to create plans/results/");

    let raw_json_name = format!("{date}-decode-throughput.raw.json");
    let raw_json_path = dir.join(&raw_json_name);
    let raw_json = serde_json::to_string_pretty(
        &inputs
            .wasm_results
            .iter()
            .map(BenchResultJson::from)
            .collect::<Vec<_>>(),
    )
    .expect("BenchResult is always serializable");
    fs::write(&raw_json_path, raw_json).expect("failed to write raw results JSON");

    let md_path = dir.join(format!("{date}-decode-throughput.md"));
    let md = render_results_markdown(inputs, &summary, &date, &raw_json_name);
    fs::write(&md_path, md).expect("failed to write results markdown");

    let benchmarks_md = render_benchmarks_md(inputs, &summary, &date);
    fs::write("BENCHMARKS.md", benchmarks_md).expect("failed to write BENCHMARKS.md");

    md_path
}

fn today() -> String {
    // No chrono dependency for one date string: days-since-epoch -> proleptic Gregorian civil
    // date, the standard branchless algorithm (Howard Hinnant's `civil_from_days`).
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after 1970")
        .as_secs();
    let days = (secs / 86400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn ratio_at_steady_state<'a>(
    results: &'a [BenchResult],
    competitor: &str,
) -> Vec<(&'a str, u8, f64)> {
    results
        .iter()
        .filter(|r| r.competitor == competitor && r.size == STEADY_STATE_SIZE)
        .map(|r| (r.pattern.as_str(), r.num_bits, r.elements_per_sec))
        .collect()
}

fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).expect("elements_per_sec is never NaN"));
    xs[xs.len() / 2]
}

fn render_results_markdown(
    inputs: &ReportInputs,
    summary: &Summary,
    date: &str,
    raw_json_name: &str,
) -> String {
    let size_scaling = render_size_scaling_table(&inputs.wasm_results, 16, "random");

    format!(
        "# {date} — Decode Throughput Benchmark Results\n\n\
         ## 1. Verdict\n\n\
         The ≥3x decode throughput claim (`plans/00-overview.md` §6): {}\n\n\
         ## 2. Exact versions\n\n\
         | Tool | Version |\n|---|---|\n\
         | `rustc` | {} |\n\
         | `cargo` | {} |\n\
         | `wasmtime` | {} |\n\
         | Machine | {} |\n\n\
         ## 3. Build configuration\n\n\
         - Target: `wasm32-wasip1`, `RUSTFLAGS=\"-C target-feature=+simd128\"` (set via \
           `.cargo/config.toml`'s `[target.wasm32-wasip1] rustflags`, applied identically to \
           this crate and every competitor — `plans/05-benchmarking-strategy.md` §5's \
           anti-cheating rule against selectively hobbling a competitor). Every JSON result \
           line's `target_feature_simd128` field is checked `true` before being included \
           here (see `xtask/src/bench_wasm.rs`) — never silently absent.\n\
         - Profile: `release` (`cargo build --release`).\n\
         - Runner: `wasmtime run --wasm simd -S inherit-network=false`.\n\
         - Iterations: 100 timed calls per (competitor, bit-width, pattern, size) point, \
           after 10 untimed warmup calls; median + stddev reported (raw data: \
           `{raw_json_name}`).\n\
         - RNG seed: `0x2545_F491` (`bench_support::BENCH_SEED`) — see \
           `src/testing/bench_gen.rs`.\n\n\
         ## 4. Headline comparison — steady state ({STEADY_STATE_SIZE_FMT} elements)\n\n\
         Millions of `u32` elements decoded per second (`elements_per_sec / 1e6`), by \
         bit-width and pattern:\n\n\
         | Pattern | Bits | `wasm_bitpack::Wasm128` (M/s) | `bitpacking::BitPacker4x` (M/s) | \
         `wasm_bitpack::Scalar` (M/s) | Wasm128 / BitPacker4x | Wasm128 / Scalar |\n\
         |---|---:|---:|---:|---:|---:|---:|\n{}\n\
         ## 5. Throughput vs. size ({STEADY_STATE_SIZE_FMT}-element sweep, `bits=16`, `random`)\n\n\
         Shows the overhead-dominated-small vs. steady-state-large regimes \
         (`plans/05-benchmarking-strategy.md` §4):\n\n{size_scaling}\n\
         ## 6. Native sanity baseline (not the headline claim)\n\n\
         `cargo bench --bench decode_native` — `Scalar`/`bitpacking`/`stream-vbyte` on native \
         x86_64 only; `Wasm128` doesn't exist on this target (`cfg`-gated to \
         `wasm32`+`simd128`). Raw `criterion` output:\n\n\
         ```text\n{}\n```\n\n\
         ## 7. Reproduction\n\n\
         ```sh\n\
         cargo xtask bench-all\n\
         ```\n\n\
         Regenerates this file (and its sibling `{raw_json_name}`, and the root \
         `BENCHMARKS.md`) from scratch. Two consecutive runs' steady-state numbers should \
         agree within normal wall-clock benchmarking noise (a few percent).\n",
        summary.verdict(),
        format_toolchain_line(&inputs.versions.rustc),
        format_toolchain_line(&inputs.versions.cargo),
        format_toolchain_line(&inputs.versions.wasmtime),
        inputs.machine,
        summary.rows,
        inputs.native_output.trim(),
    )
}

/// Renders the root `BENCHMARKS.md` — fully self-contained (see this module's doc comment
/// for why it can't just link into the git-ignored `plans/results/`).
fn render_benchmarks_md(inputs: &ReportInputs, summary: &Summary, date: &str) -> String {
    format!(
        "# Benchmarks\n\n\
         `wasm-bitpack`'s core claim: **≥3x decode throughput vs. `bitpacking::BitPacker4x`, \
         both compiled to `wasm32-wasip1` + `simd128` and run under identical conditions** \
         (`plans/00-overview.md` §6). This file is regenerated by `cargo xtask bench-all` — \
         never hand-edited, so it can't drift from what the reproduction script actually \
         measures.\n\n\
         **Last measured: {date}.** {}\n\n\
         ## Steady-state throughput ({STEADY_STATE_SIZE_FMT} elements)\n\n\
         Millions of `u32` elements decoded per second, by bit-width and pattern:\n\n\
         | Pattern | Bits | `wasm_bitpack::Wasm128` (M/s) | `bitpacking::BitPacker4x` (M/s) | \
         `wasm_bitpack::Scalar` (M/s) | Wasm128 / BitPacker4x | Wasm128 / Scalar |\n\
         |---|---:|---:|---:|---:|---:|---:|\n{}\n\
         ## Methodology\n\n\
         - Bit-widths: 1, 2, 4, 8, 11, 16, 20, 24, 32. Patterns: random, sorted-ascending, \
           delta-friendly (bounded random walk). Sizes: log-scaled from 1 block (128 \
           elements) up to 10,000,000 elements. Fixed RNG seed \
           (`bench_support::BENCH_SEED = 0x2545_F491`) — every run generates byte-identical \
           input.\n\
         - `RUSTFLAGS=\"-C target-feature=+simd128\"` applied identically to this crate and \
           every competitor (`plans/05-benchmarking-strategy.md` §5) — `bitpacking`'s \
           LLVM-autovectorized scalar fallback gets the same `simd128` opt-in this crate's \
           own hand-written decoder needs, so the comparison is against the *strongest* \
           baseline `bitpacking` can produce on this target, not a weakened one (see the \
           Phase 0 premise-validation finding this project's `plans/` record).\n\
         - 100 timed iterations per (competitor, bit-width, pattern, size) point, median + \
           stddev, after warmup. Full methodology: `plans/05-benchmarking-strategy.md`.\n\
         - Toolchain: `rustc` {}, `wasmtime` {}.\n\n\
         ## Reproduce this yourself\n\n\
         ```sh\n\
         cargo xtask bench-all\n\
         ```\n\n\
         Requires `wasmtime` on `PATH`. Writes a fresh dated snapshot (full matrix, raw JSON, \
         machine specs) to `plans/results/` — that directory is this project's local-only \
         working notes and isn't part of the public repo (see \
         `plans/decisions/0009-plans-dir-not-pushed.md`), so the numbers above are the \
         complete, self-contained public record; the local snapshot is for anyone who clones \
         the repo and runs the reproduction script themselves.\n",
        summary.verdict(),
        summary.rows,
        format_toolchain_line(&inputs.versions.rustc),
        format_toolchain_line(&inputs.versions.wasmtime),
    )
}

fn format_toolchain_line(s: &str) -> String {
    s.replace('|', "\\|")
}

fn render_size_scaling_table(results: &[BenchResult], num_bits: u8, pattern: &str) -> String {
    let mut sizes: Vec<_> = results
        .iter()
        .filter(|r| r.num_bits == num_bits && r.pattern == pattern)
        .map(|r| r.size)
        .collect();
    sizes.sort_unstable();
    sizes.dedup();

    let mut out = String::from("| Size | Wasm128 (M/s) | BitPacker4x (M/s) | Scalar (M/s) | stream-vbyte (M/s) |\n|---:|---:|---:|---:|---:|\n");
    for size in sizes {
        let get = |competitor: &str| {
            results
                .iter()
                .find(|r| {
                    r.competitor == competitor
                        && r.num_bits == num_bits
                        && r.pattern == pattern
                        && r.size == size
                })
                .map(|r| format!("{:.1}", r.elements_per_sec / 1.0e6))
                .unwrap_or_else(|| "-".to_string())
        };
        out.push_str(&format!(
            "| {size} | {} | {} | {} | {} |\n",
            get("wasm_bitpack_wasm128"),
            get("bitpacking_bitpacker4x"),
            get("wasm_bitpack_scalar"),
            get("stream_vbyte_scalar"),
        ));
    }
    out
}

/// Serializable mirror of [`BenchResult`] (which derives `Deserialize` but not `Serialize` —
/// it's parsed FROM the wasm binaries' JSON, not normally re-emitted).
#[derive(serde::Serialize)]
struct BenchResultJson {
    competitor: String,
    num_bits: u8,
    pattern: String,
    size: usize,
    median_ns: f64,
    elements_per_sec: f64,
}

impl From<&BenchResult> for BenchResultJson {
    fn from(r: &BenchResult) -> Self {
        Self {
            competitor: r.competitor.clone(),
            num_bits: r.num_bits,
            pattern: r.pattern.clone(),
            size: r.size,
            median_ns: r.median_ns,
            elements_per_sec: r.elements_per_sec,
        }
    }
}
