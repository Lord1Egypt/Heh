use std::fs;
use std::process::Command;

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
