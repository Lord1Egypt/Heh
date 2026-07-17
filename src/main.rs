//! heh — the Heh language toolchain (single binary, zero dependencies).
//!
//! Subcommands land phase by phase (see docs/agent/TASK_MENU.md):
//! P1 tokens ✓ · then ast, run, check, fmt, test, get.

use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
heh — the immortal programming language 𓁨

Usage:
  heh tokens <file.heh>  dump lexer output, one token per line
  heh --version          print the toolchain version
  heh --help             print this help

More subcommands arrive phase by phase (P2+): ast, run, check, fmt, test, get.
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
        Some("tokens") => {
            let Some(path) = args.get(1) else {
                eprintln!("heh: usage: heh tokens <file.heh>");
                return ExitCode::from(2);
            };
            cmd_tokens(path)
        }
        Some(other) => {
            eprintln!("heh: unknown command '{other}' (see --help)");
            ExitCode::from(2)
        }
    }
}

fn cmd_tokens(path: &str) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("heh: cannot read '{path}': {e}");
            return ExitCode::FAILURE;
        }
    };
    match heh::lexer::lex(&source) {
        Ok(tokens) => {
            print!("{}", heh::lexer::dump(&tokens));
            ExitCode::SUCCESS
        }
        Err(d) => {
            eprintln!("{}", d.render(path, &source));
            ExitCode::FAILURE
        }
    }
}
