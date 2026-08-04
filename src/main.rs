//! heh — the Heh language toolchain (single binary, zero dependencies).
//!
//! One binary carries every subcommand: run, check, test, fmt, get, and the
//! `tokens`/`ast` dumps used by the conformance tests.

use std::path::PathBuf;
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
heh — the immortal programming language 𓁨

Usage:
  heh run <file.heh> [args]   run a program (--tree-walk for the reference evaluator; pass --deny-fs/-net/-env/-clock/-rand)
  heh check <file.heh>        type-check without running
  heh test [dir]              run every fn test_*() in *_test.heh files
  heh fmt [--check] <path>    format a file or directory tree in place
  heh get <url>               vendor a dependency into vendor/ and pin it in heh.lock
  heh ast <file.heh>          dump the parsed AST
  heh tokens <file.heh>       dump lexer output, one token per line
  heh --version               print the toolchain version
  heh --help                  print this help

Spec: SPEC.md · Docs: https://github.com/Lord1Egypt/Heh";

/// Both engines recurse on the native stack, so Heh programs get a dedicated
/// thread with a large one. `MAX_CALL_DEPTH` then faults on runaway recursion
/// well before this runs out — a clean diagnostic instead of a core dump.
const INTERPRETER_STACK: usize = 256 * 1024 * 1024;

fn main() -> ExitCode {
    match std::thread::Builder::new()
        .stack_size(INTERPRETER_STACK)
        .spawn(run_cli)
    {
        Ok(handle) => handle.join().unwrap_or(ExitCode::FAILURE),
        // No thread available: run inline rather than refusing to start.
        Err(_) => run_cli(),
    }
}

fn run_cli() -> ExitCode {
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
            let mut use_vm = true;
            for arg in iter {
                if arg == "--vm" {
                    use_vm = true;
                } else if arg == "--tree-walk" {
                    use_vm = false;
                } else if arg.starts_with("--deny-") {
                    run_args.push(arg.clone());
                } else if path.is_none() {
                    path = Some(arg.clone());
                } else {
                    run_args.push(arg.clone());
                }
            }
            if let Some(p) = path {
                cmd_run(&p, run_args, use_vm)
            } else {
                eprintln!("heh: usage: heh run [--vm|--tree-walk] <file.heh> [args...]");
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
        Some("fmt") => {
            let mut check_mode = false;
            let mut path = None;
            for arg in args.iter().skip(1) {
                if arg == "--check" {
                    check_mode = true;
                } else if path.is_none() {
                    path = Some(arg.clone());
                }
            }
            let Some(path) = path else {
                eprintln!("heh: usage: heh fmt [--check] <file.heh>");
                return ExitCode::from(2);
            };
            cmd_fmt(&path, check_mode)
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

fn cmd_run(path: &str, run_args: Vec<String>, use_vm: bool) -> ExitCode {
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
    checker.check_file_at(&ast, std::path::Path::new(path));
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

    if use_vm {
        let mut eval = heh::eval::Evaluator::with_base_dir(base_dir);
        if let Err(d) = eval.prepare(&ast, run_args) {
            eprintln!("{}", d.render(path, &source));
            return ExitCode::FAILURE;
        }
        let program = heh::compile::compile(&ast);
        let mut vm = heh::vm::Vm::new(eval);
        return match vm.run(&program) {
            Ok(_) => ExitCode::SUCCESS,
            Err(d) => {
                eprintln!("{}", d.render(path, &source));
                ExitCode::FAILURE
            }
        };
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
    checker.check_file_at(&ast, std::path::Path::new(path));
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
    if url.contains(['\r', '\n']) {
        eprintln!("heh get: URL contains a forbidden newline");
        return ExitCode::FAILURE;
    }
    let vendor = std::path::Path::new("vendor");
    if let Err(e) = std::fs::create_dir_all(vendor) {
        eprintln!("heh get: cannot create vendor/: {e}");
        return ExitCode::FAILURE;
    }

    let fetch = if url.ends_with(".git") {
        let name = safe_vendor_name(url, "dep", true);
        let dest = vendor.join(&name);
        let _ = std::fs::remove_dir_all(&dest);
        run_tool(
            "git",
            &["clone", "--depth", "1", url, dest.to_str().unwrap_or("")],
        )
    } else {
        let name = safe_vendor_name(url, "dep.heh", false);
        let dest = vendor.join(&name);
        run_tool(
            "curl",
            &[
                "-sSL",
                "--fail",
                "--proto",
                "=http,https,file",
                "--proto-redir",
                "=http,https",
                "-o",
                dest.to_str().unwrap_or(""),
                url,
            ],
        )
    };
    if let Err(e) = fetch {
        eprintln!("heh get: {e}");
        return ExitCode::FAILURE;
    }

    match write_lock(std::path::Path::new("."), Some(url)) {
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

fn safe_vendor_name(url: &str, fallback: &str, strip_git_suffix: bool) -> String {
    let raw = url
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("");
    let candidate = if strip_git_suffix {
        raw.strip_suffix(".git").unwrap_or(raw)
    } else {
        raw
    };
    if candidate.is_empty()
        || candidate == "."
        || candidate == ".."
        || candidate.contains(['/', '\\'])
    {
        fallback.to_string()
    } else {
        candidate.to_string()
    }
}

fn run_tool(program: &str, args: &[&str]) -> Result<(), String> {
    match std::process::Command::new(program).args(args).output() {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(_) => Err(format!(
            "'{program}' not found (required to fetch this dependency)"
        )),
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
    let mut children: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.file_name());
    for entry in children {
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("refusing symlink in vendor tree: '{}'", path.display()),
            ));
        }
        if kind.is_dir() {
            // Skip git metadata — it is not source and changes constantly.
            if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                continue;
            }
            collect_files(&path, root, out)?;
        } else if kind.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = std::fs::read(&path)?;
            out.push((rel, heh::modules::sha256_hex(&bytes)));
        }
    }
    Ok(())
}

/// Write `<root>/heh.lock` from the current vendor tree. Returns the file count.
fn write_lock(root: &std::path::Path, source_url: Option<&str>) -> std::io::Result<usize> {
    let entries = hash_vendor_tree(root)?;
    let mut body =
        String::from("# heh.lock — SHA-256 of every vendored file. Do not edit by hand.\n");
    let lock_path = root.join("heh.lock");
    let mut sources: Vec<String> = std::fs::read_to_string(&lock_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.strip_prefix("# source: ").map(str::to_string))
        .collect();
    if let Some(url) = source_url {
        if !sources.iter().any(|known| known == url) {
            sources.push(url.to_string());
        }
    }
    sources.sort();
    sources.dedup();
    for source in sources {
        body.push_str(&format!("# source: {source}\n"));
    }
    for (rel, hash) in &entries {
        body.push_str(&format!("{hash}  {rel}\n"));
    }
    std::fs::write(lock_path, body)?;
    Ok(entries.len())
}

/// Verify `<base_dir>/heh.lock` against the vendor tree. Ok(()) when there is
/// no lockfile or every pinned hash matches; Err on any mismatch or missing
/// file (fail closed).
fn verify_lock(base_dir: &std::path::Path) -> Result<(), String> {
    let lock_path = base_dir.join("heh.lock");
    let contents = match std::fs::read_to_string(&lock_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let vendor = base_dir.join("vendor");
            let has_vendor_files = vendor
                .read_dir()
                .map(|mut entries| entries.next().is_some())
                .unwrap_or(false);
            return if has_vendor_files {
                Err("lock verification failed: vendor/ exists but heh.lock is missing".into())
            } else {
                Ok(())
            };
        }
        Err(e) => return Err(format!("cannot read '{}': {e}", lock_path.display())),
    };
    let mut expected = std::collections::BTreeMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (hash, rel) = line
            .split_once("  ")
            .ok_or_else(|| format!("malformed heh.lock line: '{line}'"))?;
        let rel_path = std::path::Path::new(rel);
        if !rel.starts_with("vendor/")
            || rel_path.is_absolute()
            || rel_path
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(format!("invalid path in heh.lock: '{rel}'"));
        }
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("invalid SHA-256 in heh.lock for '{rel}'"));
        }
        if expected.insert(rel.to_string(), hash.to_string()).is_some() {
            return Err(format!("duplicate path in heh.lock: '{rel}'"));
        }
    }

    let actual: std::collections::BTreeMap<_, _> = hash_vendor_tree(base_dir)
        .map_err(|e| format!("lock verification failed: {e}"))?
        .into_iter()
        .collect();
    for rel in expected.keys() {
        if !actual.contains_key(rel) {
            return Err(format!("lock verification failed: '{rel}' is missing"));
        }
    }
    for rel in actual.keys() {
        if !expected.contains_key(rel) {
            return Err(format!("lock verification failed: unpinned file '{rel}'"));
        }
    }
    for (rel, hash) in expected {
        if actual.get(&rel) != Some(&hash) {
            return Err(format!(
                "lock verification failed: '{rel}' has been modified (hash mismatch)"
            ));
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
        checker.check_file_at(&ast, file);
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

        let base_dir = file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
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
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                continue;
            }
            find_test_files(&path, out)?;
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with("_test.heh"))
        {
            out.push(path);
        }
    }
    Ok(())
}

// --------------------------------------------------------------------------
// `heh fmt` — canonical formatter (rewrites in place, or --check)
// --------------------------------------------------------------------------

/// `heh fmt [--check] <path>` — a directory formats every `.heh` file under it
/// (SPEC §13 writes the argument as a path, not a file).
fn cmd_fmt(path: &str, check_mode: bool) -> ExitCode {
    let target = std::path::Path::new(path);
    if target.is_dir() {
        let mut files = Vec::new();
        if let Err(e) = find_heh_files(target, &mut files) {
            eprintln!("heh fmt: cannot scan '{path}': {e}");
            return ExitCode::FAILURE;
        }
        files.sort();
        // Tracked as a bool because `ExitCode` is not comparable on the
        // oldest Rust this toolchain supports.
        let mut all_ok = true;
        for file in files {
            all_ok &= cmd_fmt_file(&file.to_string_lossy(), check_mode);
        }
        return if all_ok {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }
    if cmd_fmt_file(path, check_mode) {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn find_heh_files(dir: &std::path::Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                continue;
            }
            find_heh_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("heh") {
            out.push(path);
        }
    }
    Ok(())
}

/// Format one file. Returns whether it succeeded, so the directory walk can
/// accumulate a result without comparing `ExitCode`s.
fn cmd_fmt_file(path: &str, check_mode: bool) -> bool {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("heh: cannot read '{path}': {e}");
            return false;
        }
    };
    // Comments live outside the AST, so formatting needs them alongside it.
    let (tokens, comments) = match heh::lexer::lex_with_comments(&source) {
        Ok(t) => t,
        Err(d) => {
            eprintln!("{}", d.render(path, &source));
            return false;
        }
    };
    let ast = match heh::parser::Parser::new(&tokens).parse_file() {
        Ok(a) => a,
        Err(d) => {
            eprintln!("{}", d.render(path, &source));
            return false;
        }
    };

    let formatted = heh::fmt::format_file_with_comments(&ast, comments);

    if check_mode {
        if formatted == source {
            true
        } else {
            eprintln!("{path}: not formatted (run `heh fmt {path}`)");
            false
        }
    } else if formatted == source {
        true
    } else {
        match std::fs::write(path, &formatted) {
            Ok(_) => {
                println!("formatted {path}");
                true
            }
            Err(e) => {
                eprintln!("heh: cannot write '{path}': {e}");
                false
            }
        }
    }
}
