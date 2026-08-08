//! `cargo xtask` — repo automation: `ci` (runs the same checks as `.github/workflows/ci.yml`
//! locally so CI and local dev can't silently drift), and `bench-wasm`/`bench-all` (the
//! benchmark harness, see `plans/05-benchmarking-strategy.md` and `plans/01-architecture.md`
//! §7). `msrv` is added in a later phase that needs it.

mod bench_native;
mod bench_wasm;
mod cargo_build;
mod results;
mod toolchain;

use std::env;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("ci") => run_ci(),
        Some("bench-wasm") => run_bench_wasm(),
        Some("bench-all") => run_bench_all(),
        Some(other) => {
            eprintln!("unknown xtask subcommand: `{other}`");
            eprintln!("available subcommands: ci, bench-wasm, bench-all");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: cargo xtask <subcommand>");
            eprintln!("available subcommands: ci, bench-wasm, bench-all");
            ExitCode::FAILURE
        }
    }
}

/// `cargo xtask bench-wasm [competitor]` — builds and runs the `wasm32-wasip1` benchmark
/// matrix, printing every measurement as JSON to stdout (one line per (competitor, bit-width,
/// pattern, size) point). Standalone entry point for the real comparison
/// (`plans/05-benchmarking-strategy.md` §2) without also running the native baseline or
/// writing a results file — see `bench-all` for the full pipeline.
///
/// The optional `competitor` argument (one of `bench_wasm::COMPETITORS`) restricts the run to
/// a single competitor — splitting the ~13-minute full matrix into four independently
/// resumable ~3-minute chunks is convenient in constrained environments (a CI job timeout, an
/// interactive session with a wall-clock cap); `bench-all` always runs the unrestricted
/// full set.
fn run_bench_wasm() -> ExitCode {
    let only: Vec<&str> = match env::args().nth(2) {
        Some(name) => {
            assert!(
                bench_wasm::COMPETITORS.contains(&name.as_str()),
                "unknown competitor `{name}` — expected one of {:?}",
                bench_wasm::COMPETITORS
            );
            vec![Box::leak(name.into_boxed_str())]
        }
        None => bench_wasm::COMPETITORS.to_vec(),
    };
    let results = bench_wasm::run(&only);
    for result in &results {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "competitor": result.competitor,
                "num_bits": result.num_bits,
                "pattern": result.pattern,
                "size": result.size,
                "median_ns": result.median_ns,
                "elements_per_sec": result.elements_per_sec,
            }))
            .expect("json serialization of plain scalar fields never fails")
        );
    }
    ExitCode::SUCCESS
}

/// `cargo xtask bench-all` — runs the native `criterion` baseline and the full
/// `wasm32-wasip1` matrix, then writes a fresh dated snapshot to `plans/results/`
/// (`plans/05-benchmarking-strategy.md` §6-7). The single command anyone (including a
/// skeptical interviewer) runs to independently verify the published numbers.
fn run_bench_all() -> ExitCode {
    let versions = toolchain::versions();
    let machine = toolchain::machine_info();
    println!("==> rustc: {}", versions.rustc);
    println!("==> cargo: {}", versions.cargo);
    println!("==> wasmtime: {}", versions.wasmtime);
    println!("==> machine: {machine}");

    let native_output = bench_native::run();
    let wasm_results = bench_wasm::run(&bench_wasm::COMPETITORS);

    let report = results::ReportInputs {
        native_output,
        wasm_results,
        versions,
        machine,
    };
    let path = results::write_report(&report);
    println!("==> wrote {}", path.display());

    ExitCode::SUCCESS
}

fn run_ci() -> ExitCode {
    let steps: &[(&str, &[&str])] = &[
        (
            "cargo fmt --all -- --check",
            &["fmt", "--all", "--", "--check"],
        ),
        (
            "cargo clippy --workspace --all-targets --features bench-support -- -D warnings",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--features",
                "bench-support",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("cargo test --workspace", &["test", "--workspace"]),
    ];

    for (label, step_args) in steps {
        println!("==> {label}");
        let status = Command::new("cargo")
            .args(*step_args)
            .status()
            .unwrap_or_else(|err| panic!("failed to spawn `{label}`: {err}"));
        if !status.success() {
            eprintln!("xtask ci: `{label}` failed");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}
