use std::fs;
use std::process::Command;

const CHECKER_DIAGNOSTICS: &[(&str, &str)] = &[
    ("E0010", "check_e0010"),
    ("E0011", "check_e0011"),
    ("E0020", "match_non_exhaustive"),
    ("E0021", "check_e0021"),
    ("E0030", "import_cycle"),
    ("E0032", "check_e0032"),
    ("E0033", "check_e0033"),
    ("E0040", "check_e0040"),
    ("E0041", "check_e0041"),
    ("E0042", "check_e0042"),
    ("E0043", "check_e0043"),
    ("E0044", "check_e0044"),
    ("E0045", "check_e0045"),
    ("E0050", "check_e0050"),
    ("E0051", "check_e0051"),
    ("E0052", "check_e0052"),
    ("E0053", "check_e0053"),
    ("E0054", "check_e0054"),
    ("E0055", "check_e0055"),
    ("E0056", "check_e0056"),
    ("E0057", "check_e0057"),
    ("E0058", "check_e0058"),
    ("E0059", "check_e0059"),
    ("E0109", "check_e0109"),
    ("E0110", "check_e0110"),
    ("E0114", "try_outside_fn"),
];

#[test]
fn corpus_programs() {
    let programs_dir = "tests/corpus/programs";
    if let Ok(entries) = fs::read_dir(programs_dir) {
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("heh") {
                let name = path.file_stem().unwrap().to_str().unwrap();
                let out_path = format!("{}/{}.out", programs_dir, name);
                let args_path = format!("{}/{}.args", programs_dir, name);
                let mut cmd = Command::new(env!("CARGO_BIN_EXE_heh"));
                cmd.arg("run").arg(&path);

                if let Ok(args_content) = fs::read_to_string(&args_path) {
                    for arg in args_content.split_whitespace() {
                        cmd.arg(arg);
                    }
                }

                let output = cmd.output().expect("failed to execute heh run");

                let actual_out = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");

                if let Ok(expected_out) = fs::read_to_string(&out_path) {
                    let expected_out = expected_out.replace("\r\n", "\n");
                    assert_eq!(actual_out, expected_out, "Program {} output mismatch", name);
                } else {
                    if std::env::var("UPDATE_GOLDEN").is_ok() {
                        fs::write(&out_path, actual_out.as_bytes()).unwrap();
                    } else {
                        panic!("Missing golden output for program: {}", name);
                    }
                }
                assert!(
                    output.status.success(),
                    "Program {} failed with exit code: {:?}",
                    name,
                    output.status
                );
            }
        }
    }
}

#[test]
fn corpus_errors() {
    let errors_dir = "tests/corpus/errors";
    if let Ok(entries) = fs::read_dir(errors_dir) {
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("heh") {
                let name = path.file_stem().unwrap().to_str().unwrap();
                let err_path = format!("{}/{}.err", errors_dir, name);

                let expected = fs::read_to_string(&err_path)
                    .unwrap_or_else(|_| panic!("Missing golden stderr for program: {name}"))
                    .replace("\r\n", "\n");
                let phase_path = format!("{errors_dir}/{name}.phase");
                let phase = fs::read_to_string(phase_path).unwrap_or_else(|_| "compile".into());

                let check = Command::new(env!("CARGO_BIN_EXE_heh"))
                    .arg("check")
                    .arg(&path)
                    .output()
                    .expect("failed to execute heh check");
                let run = Command::new(env!("CARGO_BIN_EXE_heh"))
                    .arg("run")
                    .arg(&path)
                    .output()
                    .expect("failed to execute heh run");

                let assert_failure = |command: &str, output: &std::process::Output| {
                    assert_eq!(output.status.code(), Some(1), "{name}: {command} exit code");
                    assert!(output.stdout.is_empty(), "{name}: {command} wrote stdout");
                    assert_eq!(
                        String::from_utf8_lossy(&output.stderr)
                            .replace("\r\n", "\n")
                            .replace('\\', "/"),
                        expected,
                        "{name}: {command} stderr"
                    );
                };

                match phase.trim() {
                    "compile" => {
                        assert_failure("check", &check);
                        assert_failure("run", &run);
                    }
                    "runtime" => {
                        assert!(
                            check.status.success(),
                            "{name}: check must accept runtime case"
                        );
                        assert!(check.stdout.is_empty() && check.stderr.is_empty());
                        assert_failure("run", &run);
                    }
                    other => panic!("{name}: unknown diagnostic phase {other:?}"),
                }
            }
        }
    }
}

#[test]
fn checker_diagnostic_corpus_is_complete() {
    let checker = fs::read_to_string("src/check.rs").expect("read checker source");
    let emitted: std::collections::BTreeSet<_> = checker
        .lines()
        .filter_map(|line| line.trim().strip_prefix("code: \"")?.strip_suffix("\","))
        .collect();
    let covered: std::collections::BTreeSet<_> =
        CHECKER_DIAGNOSTICS.iter().map(|(code, _)| *code).collect();
    assert_eq!(
        covered, emitted,
        "checker diagnostic corpus coverage drifted"
    );

    for (code, name) in CHECKER_DIAGNOSTICS {
        let source = format!("tests/corpus/errors/{name}.heh");
        let stderr = format!("tests/corpus/errors/{name}.err");
        assert!(
            std::path::Path::new(&source).is_file(),
            "missing {code} source"
        );
        let snapshot =
            fs::read_to_string(stderr).unwrap_or_else(|_| panic!("missing {code} stderr"));
        assert!(
            snapshot.starts_with(&format!("error[{code}]:")),
            "{name} does not snapshot {code}"
        );
    }
}
