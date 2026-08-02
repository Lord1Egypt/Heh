//! Differential test for integer arithmetic: Heh against CPython, whose ints
//! are also arbitrary-precision and whose `//` and `%` sign rules SPEC §6.1
//! adopts by name.
//!
//! This exists because `int` is the language's headline feature and the
//! implementation carries a machine-word fast path that promotes to a bignum.
//! Every boundary around that promotion is where a bug would hide, so the
//! operand pool below is deliberately stacked with values just under, at, and
//! just over the i64 and u32-limb edges.

use std::process::Command;

/// Operands chosen to sit on every representation boundary.
fn operands() -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for base in [
        "0",
        "1",
        "-1",
        "2",
        "-2",
        "7",
        "-7",
        "10",
        "255",
        "256",
        // u32 limb edges
        "4294967295",
        "4294967296",
        "4294967297",
        "-4294967295",
        "-4294967296",
        // i64 edges — where the fast path must give up
        "9223372036854775806",
        "9223372036854775807",
        "-9223372036854775807",
        "-9223372036854775808",
        // comfortably beyond any machine word
        "9223372036854775808",
        "18446744073709551616",
        "123456789012345678901234567890",
        "-123456789012345678901234567890",
    ] {
        v.push(base.to_string());
    }
    v
}

/// Joined output with line endings normalized. On Windows CPython's `print`
/// emits CRLF while Heh emits LF, which is a difference in the harness, not in
/// arithmetic — without this the comparison fails on Windows for no reason.
fn normalize(stdout: &[u8], stderr: &[u8]) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    )
    .replace("\r\n", "\n")
}

fn heh_bin() -> &'static str {
    env!("CARGO_BIN_EXE_heh")
}

fn run_heh(src: &str) -> String {
    let dir = std::env::temp_dir().join(format!("heh-bignum-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("prog.heh");
    std::fs::write(&file, src).unwrap();
    let out = Command::new(heh_bin())
        .arg("run")
        .arg(&file)
        .output()
        .expect("failed to run heh");
    let _ = std::fs::remove_dir_all(&dir);
    normalize(&out.stdout, &out.stderr)
}

/// Written to a file rather than passed with `-c`: the generated program is
/// far larger than a command line can carry.
fn run_python(src: &str) -> Option<String> {
    let dir = std::env::temp_dir().join(format!("heh-bignum-py-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    let file = dir.join("prog.py");
    std::fs::write(&file, src).ok()?;
    let out = Command::new("python3").arg(&file).output().ok()?;
    let _ = std::fs::remove_dir_all(&dir);
    Some(normalize(&out.stdout, &out.stderr))
}

/// Compare every binary op over every operand pair. Division and modulo by
/// zero are faults in Heh and exceptions in Python, so those pairs are skipped
/// here — `tests/corpus/errors` covers them.
#[test]
fn arithmetic_matches_cpython() {
    if run_python("print(1)").map(|s| s.trim().to_string()) != Some("1".to_string()) {
        eprintln!("python3 unavailable — skipping the differential test");
        return;
    }

    let ops = operands();
    let mut heh_src = String::new();
    let mut py_src = String::new();

    for a in &ops {
        for b in &ops {
            for op in ["+", "-", "*"] {
                heh_src.push_str(&format!("sys.print({a} {op} {b})\n"));
                py_src.push_str(&format!("print({a} {op} {b})\n"));
            }
            // Only defined for a non-zero divisor.
            if b != "0" {
                for op in ["//", "%"] {
                    heh_src.push_str(&format!("sys.print({a} {op} {b})\n"));
                    py_src.push_str(&format!("print({a} {op} {b})\n"));
                }
            }
            for op in ["==", "!=", "<", "<=", ">", ">="] {
                heh_src.push_str(&format!("sys.print({a} {op} {b})\n"));
                // Heh prints booleans lowercase.
                py_src.push_str(&format!("print(str({a} {op} {b}).lower())\n"));
            }
        }
    }

    // Exponentiation separately: keep the exponents small enough to stay quick.
    for a in &ops {
        for e in ["0", "1", "2", "3", "7", "64"] {
            heh_src.push_str(&format!("sys.print(({a}) ** {e})\n"));
            py_src.push_str(&format!("print(({a}) ** {e})\n"));
        }
    }

    let heh_out = run_heh(&heh_src);
    let py_out = run_python(&py_src).unwrap();

    if heh_out != py_out {
        let h: Vec<&str> = heh_out.lines().collect();
        let p: Vec<&str> = py_out.lines().collect();
        let mut first = String::new();
        for (i, (a, b)) in h.iter().zip(p.iter()).enumerate() {
            if a != b {
                let expr = heh_src.lines().nth(i).unwrap_or("?");
                first = format!("line {i}: {expr}\n  heh    : {a}\n  cpython: {b}");
                break;
            }
        }
        if first.is_empty() {
            first = format!(
                "output lengths differ: heh {} lines, cpython {} lines",
                h.len(),
                p.len()
            );
        }
        panic!("integer arithmetic diverges from CPython\n{first}");
    }
}

/// `str()` of an integer must round-trip through the parser unchanged, at
/// every magnitude — the promotion boundary included.
#[test]
fn int_display_round_trips() {
    let mut src = String::new();
    for a in operands() {
        src.push_str(&format!("sys.print(int_of(str({a})) == ok({a}))\n"));
    }
    let out = run_heh(&src);
    for (i, line) in out.lines().enumerate() {
        assert_eq!(line, "true", "round-trip failed for operand {i}: {out}");
    }
}
