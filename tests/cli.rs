//! P0 smoke tests: the toolchain binary exists, runs, and behaves.

use std::process::Command;

fn heh(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_heh"))
        .args(args)
        .output()
        .expect("failed to run heh binary")
}

#[test]
fn version_prints_and_exits_zero() {
    let out = heh(&["--version"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), format!("heh {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_prints_usage() {
    let out = heh(&["--help"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("immortal programming language"));
}

#[test]
fn unknown_command_fails_with_code_2() {
    let out = heh(&["frobnicate"]);
    assert_eq!(out.status.code(), Some(2));
}
