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
    assert!(
        lock.contains(&format!("# source: {url}")),
        "lock missing source URL: {lock}"
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

#[test]
fn lock_rejects_unpinned_files_and_missing_lock() {
    if !curl_available() {
        eprintln!("skipping vendor test: curl not available");
        return;
    }

    let dir = fresh_dir("completeness");
    let lib = dir.join("dep.heh");
    fs::write(&lib, "let answer = 42\n").unwrap();
    fs::write(dir.join("app.heh"), "sys.print(\"ok\")\n").unwrap();
    let url = file_url(&lib);
    let fetched = Command::new(heh())
        .current_dir(&dir)
        .args(["get", &url])
        .output()
        .unwrap();
    assert!(fetched.status.success());

    fs::write(dir.join("vendor/unpinned.heh"), "let hidden = true\n").unwrap();
    let out = Command::new(heh())
        .current_dir(&dir)
        .args(["run", "app.heh"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unpinned file"));

    fs::remove_file(dir.join("vendor/unpinned.heh")).unwrap();
    fs::remove_file(dir.join("heh.lock")).unwrap();
    let out = Command::new(heh())
        .current_dir(&dir)
        .args(["run", "app.heh"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("heh.lock is missing"));

    let _ = fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn lock_rejects_vendor_symlinks() {
    use std::os::unix::fs::symlink;

    let dir = fresh_dir("symlink");
    fs::create_dir(dir.join("vendor")).unwrap();
    fs::write(dir.join("outside.heh"), "let escaped = true\n").unwrap();
    symlink(dir.join("outside.heh"), dir.join("vendor/escaped.heh")).unwrap();
    fs::write(dir.join("app.heh"), "sys.print(\"ok\")\n").unwrap();
    fs::write(
        dir.join("heh.lock"),
        "# deliberately empty: the symlink must still be rejected\n",
    )
    .unwrap();

    let out = Command::new(heh())
        .current_dir(&dir)
        .args(["run", "app.heh"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("refusing symlink"));
    let _ = fs::remove_dir_all(&dir);
}
