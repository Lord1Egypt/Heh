//! P10 tooling: `heh test` discovery, execution, and exit codes.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn heh() -> &'static str {
    env!("CARGO_BIN_EXE_heh")
}

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!("heh_tooling_{}_{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn test_runner_passes_and_ignores_non_tests() {
    let dir = fresh_dir("pass");
    fs::write(
        dir.join("a_test.heh"),
        "use std/debug\n\
         fn test_math()\n    debug.assert(2 + 2 == 4, \"math\")\n\
         fn test_str()\n    debug.assert(\"ab\".upper() == \"AB\", \"upper\")\n\
         fn helper()\n    debug.assert(true, \"ignored\")\n",
    )
    .unwrap();

    let out = Command::new(heh())
        .args(["test", dir.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "expected success, got: {stdout}");
    assert!(
        stdout.contains("2 passed, 0 failed"),
        "summary wrong: {stdout}"
    );
    assert!(
        !stdout.contains("helper"),
        "non-test fn should be ignored: {stdout}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_runner_reports_failures_and_exits_nonzero() {
    let dir = fresh_dir("fail");
    fs::write(
        dir.join("b_test.heh"),
        "use std/debug\n\
         fn test_good()\n    debug.assert(true, \"good\")\n\
         fn test_bad()\n    debug.assert(1 == 2, \"one is two\")\n",
    )
    .unwrap();

    let out = Command::new(heh())
        .args(["test", dir.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success(), "expected failure exit code");
    assert!(
        stdout.contains("FAIL test_bad"),
        "should report failing test: {stdout}"
    );
    assert!(
        stdout.contains("one is two"),
        "should show assert message: {stdout}"
    );
    assert!(
        stdout.contains("1 passed, 1 failed"),
        "summary wrong: {stdout}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_runner_handles_no_tests() {
    let dir = fresh_dir("empty");
    let out = Command::new(heh())
        .args(["test", dir.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("no *_test.heh"));
    let _ = fs::remove_dir_all(&dir);
}
