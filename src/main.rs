//! heh — the Heh language toolchain (single binary, zero dependencies).
//!
//! Subcommands land phase by phase (see docs/agent/TASK_MENU.md):
//! P1 tokens ✓ · then ast, run, check, fmt, test, get.

use std::path::PathBuf;
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
heh — the immortal programming language 𓁨

Usage:
  heh run <file.heh> [args]   run a program (pass --deny-fs/-net/-env/-clock/-rand)
  heh check <file.heh>        type-check without running
  heh get <url>               vendor a dependency into vendor/ and pin it in heh.lock
  heh ast <file.heh>          dump the parsed AST
  heh tokens <file.heh>       dump lexer output, one token per line
  heh --version               print the toolchain version
  heh --help                  print this help

More subcommands arrive phase by phase: fmt, test.
Spec: SPEC.md · Plan: docs/agent/TASK_MENU.md";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => {
            println!("heh {VERSION}");
            ExitCode::SUCCESS
        }
        Some("--help") | Some("-h") | None => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("tokens") => {
            let Some(path) = args.get(1) else {
                eprintln!("heh: usage: heh tokens <file.heh>");
                return ExitCode::from(2);
            };
            cmd_tokens(path)
        }
        Some("ast") => {
            let Some(path) = args.get(1) else {
                eprintln!("heh: usage: heh ast <file.heh>");
                return ExitCode::from(2);
            };
            cmd_ast(path)
        }
        Some("check") => {
            let Some(path) = args.get(1) else {
                eprintln!("heh: usage: heh check <file.heh>");
                return ExitCode::from(2);
            };
            cmd_check(path)
        }
        Some("run") => {
            let iter = args.iter().skip(1);
            let mut path = None;
            let mut run_args = Vec::new();
            for arg in iter {
                if arg.starts_with("--deny-") {
                    run_args.push(arg.clone());
                } else if path.is_none() {
                    path = Some(arg.clone());
                } else {
                    run_args.push(arg.clone());
                }
            }
            if let Some(p) = path {
                cmd_run(&p, run_args)
            } else {
                eprintln!("heh: usage: heh run <file.heh> [args...]");
                ExitCode::from(2)
            }
        }
        Some("get") => {
            let Some(url) = args.get(1) else {
                eprintln!("heh: usage: heh get <url>");
                return ExitCode::from(2);
            };
            cmd_get(url)
        }
        Some("test") => {
            let dir = args.get(1).map(String::as_str).unwrap_or(".");
            cmd_test(dir)
        }
        Some(other) => {
            eprintln!("heh: unknown command '{other}' (see --help)");
            ExitCode::from(2)
        }
    }
}

fn cmd_tokens(path: &str) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("heh: cannot read '{path}': {e}");
            return ExitCode::FAILURE;
        }
    };
    match heh::lexer::lex(&source) {
        Ok(tokens) => {
            print!("{}", heh::lexer::dump(&tokens));
            ExitCode::SUCCESS
        }
        Err(d) => {
            eprintln!("{}", d.render(path, &source));
            ExitCode::FAILURE
        }
    }
}

fn cmd_ast(path: &str) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("heh: cannot read '{path}': {e}");
            return ExitCode::FAILURE;
        }
    };

    let tokens = match heh::lexer::lex(&source) {
        Ok(t) => t,
        Err(d) => {
            eprintln!("{}", d.render(path, &source));
            return ExitCode::FAILURE;
        }
    };

    let mut parser = heh::parser::Parser::new(&tokens);
    match parser.parse_file() {
        Ok(ast) => {
            print!("{}", heh::ast::dump_file(&ast));
            ExitCode::SUCCESS
        }
        Err(d) => {
            eprintln!("{}", d.render(path, &source));
            ExitCode::FAILURE
        }
    }
}

fn cmd_run(path: &str, run_args: Vec<String>) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("heh: cannot read '{path}': {e}");
            return ExitCode::FAILURE;
        }
    };

    let tokens = match heh::lexer::lex(&source) {
        Ok(t) => t,
        Err(d) => {
            eprintln!("{}", d.render(path, &source));
            return ExitCode::FAILURE;
        }
    };

    let mut parser = heh::parser::Parser::new(&tokens);
    let ast = match parser.parse_file() {
        Ok(a) => a,
        Err(d) => {
            eprintln!("{}", d.render(path, &source));
            return ExitCode::FAILURE;
        }
    };

    let mut checker = heh::check::Checker::new();
    checker.check_file(&ast);
    if !checker.diags.is_empty() {
        for d in checker.diags {
            eprintln!("{}", d.render(path, &source));
        }
        return ExitCode::FAILURE;
    }

    let base_dir = std::path::Path::new(path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    // Verify the vendor lockfile before running: a tampered vendored file is a
    // fault (fail closed, never execute mismatched code).
    if let Err(e) = verify_lock(&base_dir) {
        eprintln!("fault: {e}");
        return ExitCode::FAILURE;
    }

    let mut eval = heh::eval::Evaluator::with_base_dir(base_dir);
    match eval.eval_file(&ast, run_args) {
        Ok(_) => ExitCode::SUCCESS,
        Err(d) => {
            eprintln!("{}", d.render(path, &source));
            ExitCode::FAILURE
        }
    }
}

fn cmd_check(path: &str) -> ExitCode {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("heh: cannot read '{path}': {e}");
            return ExitCode::FAILURE;
        }
    };

    let tokens = match heh::lexer::lex(&source) {
        Ok(t) => t,
        Err(d) => {
            eprintln!("{}", d.render(path, &source));
            return ExitCode::FAILURE;
        }
    };

    let mut parser = heh::parser::Parser::new(&tokens);
    let ast = match parser.parse_file() {
        Ok(a) => a,
        Err(d) => {
            eprintln!("{}", d.render(path, &source));
            return ExitCode::FAILURE;
        }
    };

    let mut checker = heh::check::Checker::new();
    checker.check_file(&ast);
    if !checker.diags.is_empty() {
        for d in checker.diags {
            eprintln!("{}", d.render(path, &source));
        }
        return ExitCode::FAILURE;
    }
    
    ExitCode::SUCCESS
}

// --------------------------------------------------------------------------
// Vendoring: `heh get <url>` + heh.lock (SHA-256 of every vendored file)
// --------------------------------------------------------------------------

/// `heh get <url>` — vendor a dependency into `./vendor/` and refresh
/// `./heh.lock`. A `.git` URL is cloned with git; anything else is fetched
/// with curl. Both run as arg-lists (never a shell string).
fn cmd_get(url: &str) -> ExitCode {
    let vendor = std::path::Path::new("vendor");
    if let Err(e) = std::fs::create_dir_all(vendor) {
        eprintln!("heh get: cannot create vendor/: {e}");
        return ExitCode::FAILURE;
    }

    let fetch = if url.ends_with(".git") {
        let name = url.rsplit('/').next().unwrap_or("dep").trim_end_matches(".git");
        let dest = vendor.join(name);
        let _ = std::fs::remove_dir_all(&dest);
        run_tool("git", &["clone", "--depth", "1", url, dest.to_str().unwrap_or("")])
    } else {
        let name = url.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or("dep.heh");
        let dest = vendor.join(name);
        run_tool("curl", &["-sSL", "--fail", "-o", dest.to_str().unwrap_or(""), url])
    };
    if let Err(e) = fetch {
        eprintln!("heh get: {e}");
        return ExitCode::FAILURE;
    }

    match write_lock(std::path::Path::new(".")) {
        Ok(n) => {
            println!("vendored '{url}' — heh.lock now pins {n} file(s)");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("heh get: cannot write heh.lock: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_tool(program: &str, args: &[&str]) -> Result<(), String> {
    match std::process::Command::new(program).args(args).output() {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(format!("{program} failed: {}", String::from_utf8_lossy(&out.stderr).trim())),
        Err(_) => Err(format!("'{program}' not found (required to fetch this dependency)")),
    }
}

/// Collect (relative-path, sha256-hex) for every file under `<root>/vendor/`,
/// sorted by path for a stable lockfile.
fn hash_vendor_tree(root: &std::path::Path) -> std::io::Result<Vec<(String, String)>> {
    let vendor = root.join("vendor");
    let mut entries = Vec::new();
    if vendor.is_dir() {
        collect_files(&vendor, root, &mut entries)?;
    }
    entries.sort();
    Ok(entries)
}

fn collect_files(
    dir: &std::path::Path,
    root: &std::path::Path,
    out: &mut Vec<(String, String)>,
) -> std::io::Result<()> {
    let mut children: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    children.sort();
    for path in children {
        if path.is_dir() {
            // Skip git metadata — it is not source and changes constantly.
            if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                continue;
            }
            collect_files(&path, root, out)?;
        } else if path.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            let bytes = std::fs::read(&path)?;
            out.push((rel, heh::modules::sha256_hex(&bytes)));
        }
    }
    Ok(())
}

/// Write `<root>/heh.lock` from the current vendor tree. Returns the file count.
fn write_lock(root: &std::path::Path) -> std::io::Result<usize> {
    let entries = hash_vendor_tree(root)?;
    let mut body = String::from("# heh.lock — SHA-256 of every vendored file. Do not edit by hand.\n");
    for (rel, hash) in &entries {
        body.push_str(&format!("{hash}  {rel}\n"));
    }
    std::fs::write(root.join("heh.lock"), body)?;
    Ok(entries.len())
}

/// Verify `<base_dir>/heh.lock` against the vendor tree. Ok(()) when there is
/// no lockfile or every pinned hash matches; Err on any mismatch or missing
/// file (fail closed).
fn verify_lock(base_dir: &std::path::Path) -> Result<(), String> {
    let lock_path = base_dir.join("heh.lock");
    let contents = match std::fs::read_to_string(&lock_path) {
        Ok(c) => c,
        Err(_) => return Ok(()), // no lockfile: nothing to verify
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (hash, rel) = line.split_once("  ").ok_or_else(|| format!("malformed heh.lock line: '{line}'"))?;
        let file = base_dir.join(rel);
        let bytes = std::fs::read(&file).map_err(|_| format!("lock verification failed: '{rel}' is missing"))?;
        let actual = heh::modules::sha256_hex(&bytes);
        if actual != hash {
            return Err(format!("lock verification failed: '{rel}' has been modified (hash mismatch)"));
        }
    }
    Ok(())
}

// --------------------------------------------------------------------------
// `heh test` — discover *_test.heh, run pure `fn test_*()`, report results
// --------------------------------------------------------------------------

fn cmd_test(dir: &str) -> ExitCode {
    let mut files = Vec::new();
    if let Err(e) = find_test_files(std::path::Path::new(dir), &mut files) {
        eprintln!("heh test: cannot scan '{dir}': {e}");
        return ExitCode::FAILURE;
    }
    files.sort();
    if files.is_empty() {
        println!("no *_test.heh files found under '{dir}'");
        return ExitCode::SUCCESS;
    }

    let mut total = 0usize;
    let mut passed = 0usize;
    for file in &files {
        let rel = file.display();
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{rel}: cannot read: {e}");
                return ExitCode::FAILURE;
            }
        };
        let tokens = match heh::lexer::lex(&source) {
            Ok(t) => t,
            Err(d) => {
                eprintln!("{}", d.render(&rel.to_string(), &source));
                return ExitCode::FAILURE;
            }
        };
        let ast = match heh::parser::Parser::new(&tokens).parse_file() {
            Ok(a) => a,
            Err(d) => {
                eprintln!("{}", d.render(&rel.to_string(), &source));
                return ExitCode::FAILURE;
            }
        };
        let mut checker = heh::check::Checker::new();
        checker.check_file(&ast);
        if !checker.diags.is_empty() {
            for d in &checker.diags {
                eprintln!("{}", d.render(&rel.to_string(), &source));
            }
            return ExitCode::FAILURE;
        }

        let test_names: Vec<String> = ast
            .items
            .iter()
            .filter_map(|item| match item {
                heh::ast::TopItem::Fn(f) if f.name.starts_with("test_") => Some(f.name.clone()),
                _ => None,
            })
            .collect();
        if test_names.is_empty() {
            continue;
        }

        let base_dir = file.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from("."));
        let mut eval = heh::eval::Evaluator::with_base_dir(base_dir);
        if let Err(d) = eval.load_defs(&ast) {
            eprintln!("{}", d.render(&rel.to_string(), &source));
            return ExitCode::FAILURE;
        }

        println!("{rel}:");
        for name in test_names {
            total += 1;
            match eval.call_zero_arg_fn(&name) {
                Ok(_) => {
                    passed += 1;
                    println!("  ok   {name}");
                }
                Err(d) => {
                    println!("  FAIL {name} — {}", d.msg);
                }
            }
        }
    }

    let failed = total - passed;
    println!("\n{passed} passed, {failed} failed ({total} total)");
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn find_test_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                continue;
            }
            find_test_files(&path, out)?;
        } else if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with("_test.heh")) {
            out.push(path);
        }
    }
    Ok(())
}
