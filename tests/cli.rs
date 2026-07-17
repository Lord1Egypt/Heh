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

#[test]
fn tokens_subcommand_dumps_hello() {
    let path = format!("{}/examples/hello.heh", env!("CARGO_MANIFEST_DIR"));
    let out = heh(&["tokens", &path]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("STR(\"Heh lives forever 𓁨\")"));
    assert!(stdout.trim_end().ends_with("EOF"));
}

#[test]
fn tokens_missing_file_fails_with_code_1() {
    let out = heh(&["tokens", "no/such/file.heh"]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn tokens_lex_error_renders_diagnostic() {
    let dir = std::env::temp_dir().join("heh-cli-test");
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("bad.heh");
    std::fs::write(&bad, "if x\n\ty\n").unwrap();
    let out = heh(&["tokens", bad.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("error[E0001]"), "got: {stderr}");
    assert!(stderr.contains("bad.heh:2:1"), "got: {stderr}");
}
