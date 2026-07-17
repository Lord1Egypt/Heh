//! heh — the Heh language toolchain (single binary, zero dependencies).
//!
//! P0 baseline: version + usage. Subcommands land phase by phase
//! (see docs/agent/TASK_MENU.md): tokens, ast, run, check, fmt, test, get.

use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
heh — the immortal programming language 𓁨

Usage:
  heh --version          print the toolchain version
  heh --help             print this help

Subcommands arrive phase by phase (P1+): tokens, ast, run, check, fmt, test, get.
Spec: SPEC.md · Plan: docs/agent/TASK_MENU.md";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => {
            println!("heh {VERSION}");
            ExitCode::SUCCESS
        }
        Some("--help") | Some("-h") | None => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("heh: unknown command '{other}' (this is the P0 baseline; see --help)");
            ExitCode::from(2)
        }
    }
}
