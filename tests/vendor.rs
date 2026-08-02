//! P9 vendoring: `heh get` writes heh.lock, and `heh run` verifies it,
//! faulting (fail closed) when a vendored file has been tampered with.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn heh() -> &'static str {
    env!("CARGO_BIN_EXE_heh")
}

fn curl_available() -> bool {
    Command::new("curl")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A `file://` URL curl accepts on every platform. A Windows path is
/// `C:\dir\file`, which needs forward slashes and the third slash before the
/// drive letter; a Unix path already starts with one.
fn file_url(p: &std::path::Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    if s.starts_with('/') {
        format!("file://{s}")
    } else {
        format!("file:///{s}")
    }
}

fn fresh_dir(tag: &str) -> PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!("heh_vendor_{}_{}", tag, std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn vendor_get_lock_and_tamper() {
    if !curl_available() {
        eprintln!("skipping vendor test: curl not available");
        return;
    }

    let dir = fresh_dir("lock");
    let lib = dir.join("greetlib.heh");
    fs::write(&lib, "fn shout(s: str) -> str\n    \"{s.upper()}!\"\n").unwrap();
    fs::write(
        dir.join("app.heh"),
        "use vendor/greetlib\nsys.print(greetlib.shout(\"heh\"))\n",
    )
    .unwrap();

    // heh get <file:// url> -> vendors the file and writes heh.lock
    let url = file_url(&lib);
    let out = Command::new(heh())
        .current_dir(&dir)
        .args(["get", &url])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "heh get failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(dir.join("heh.lock").exists(), "heh.lock was not created");
    assert!(
        dir.join("vendor/greetlib.heh").exists(),
        "file was not vendored"
    );

    let lock = fs::read_to_string(dir.join("heh.lock")).unwrap();
    assert!(
        lock.contains("vendor/greetlib.heh"),
        "lock missing entry: {lock}"
    );

    // run with a valid lock -> success
    let out = Command::new(heh())
        .current_dir(&dir)
        .args(["run", "app.heh"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "HEH!\n");

    // tamper the vendored file -> run must fault with a hash mismatch
    fs::write(
        dir.join("vendor/greetlib.heh"),
        "fn shout(s: str) -> str\n    \"{s.upper()}?\"\n",
    )
    .unwrap();
    let out = Command::new(heh())
        .current_dir(&dir)
        .args(["run", "app.heh"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "tampered run should have failed");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("hash mismatch"),
        "expected hash mismatch, got: {stderr}"
    );

    let _ = fs::remove_dir_all(&dir);
}
