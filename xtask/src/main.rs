//! `cargo xtask` — repo automation. Currently implements only `ci`, which runs the same
//! checks as `.github/workflows/ci.yml` locally so CI and local dev can't silently drift.
//! Further subcommands (`bench-wasm`, `bench-all`, `msrv`) are added in the phases that
//! need them (see `plans/01-architecture.md` §7).

use std::env;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("ci") => run_ci(),
        Some(other) => {
            eprintln!("unknown xtask subcommand: `{other}`");
            eprintln!("available subcommands: ci");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: cargo xtask <subcommand>");
            eprintln!("available subcommands: ci");
            ExitCode::FAILURE
        }
    }
}

fn run_ci() -> ExitCode {
    let steps: &[(&str, &[&str])] = &[
        (
            "cargo fmt --all -- --check",
            &["fmt", "--all", "--", "--check"],
        ),
        (
            "cargo clippy --workspace --all-targets -- -D warnings",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
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
