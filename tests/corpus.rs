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
                
                let output = cmd
                    .output()
                    .expect("failed to execute heh run");

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
                assert!(output.status.success(), "Program {} failed with exit code: {:?}", name, output.status);
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
                
                // We just run AST for now, or run. Since Evaluator I only covers part of language,
                // and some errors are syntax errors, we can just use `heh run` which does both parsing and eval.
                let output = Command::new(env!("CARGO_BIN_EXE_heh"))
                    .arg("run")
                    .arg(&path)
                    .output()
                    .expect("failed to execute heh run");

                let actual_err = String::from_utf8_lossy(&output.stderr);
                
                if let Ok(expected_err_code) = fs::read_to_string(&err_path) {
                    let expected_code = expected_err_code.trim();
                    assert!(actual_err.contains(expected_code), 
                        "Error corpus {} did not contain expected diagnostic {}.\nActual stderr:\n{}", 
                        name, expected_code, actual_err);
                } else {
                    panic!("Missing golden error code for program: {}", name);
                }
                assert!(!output.status.success(), "Error corpus {} was expected to fail, but succeeded.", name);
            }
        }
    }
}
