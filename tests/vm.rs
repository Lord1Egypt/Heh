//! P11 gate: the bytecode VM must produce byte-identical output to the
//! tree-walking evaluator across the entire corpus and examples.

use std::fs;
use std::path::Path;
use std::process::Command;

fn heh() -> &'static str {
    env!("CARGO_BIN_EXE_heh")
}

fn run(path: &Path, vm: bool, args: &[String]) -> (String, String, Option<i32>) {
    let mut cmd = Command::new(heh());
    cmd.arg("run");
    if vm {
        cmd.arg("--vm");
    }
    cmd.arg(path);
    for a in args {
        cmd.arg(a);
    }
    let out = cmd.output().expect("run heh");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code(),
    )
}

fn differential(path: &Path) {
    let stem = path.file_stem().unwrap().to_str().unwrap();
    let dir = path.parent().unwrap();
    let args_path = dir.join(format!("{stem}.args"));
    let args: Vec<String> = fs::read_to_string(&args_path)
        .ok()
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_default();

    let (tw_out, _tw_err, tw_code) = run(path, false, &args);
    let (vm_out, _vm_err, vm_code) = run(path, true, &args);

    assert_eq!(tw_out, vm_out, "stdout differs for {}", path.display());
    assert_eq!(tw_code, vm_code, "exit code differs for {}", path.display());
}

fn walk(dir: &str) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("heh"))
        .collect();
    paths.sort();
    for p in paths {
        differential(&p);
    }
}

#[test]
fn vm_matches_tree_walker_on_corpus() {
    walk("tests/corpus/programs");
    walk("examples");
}
