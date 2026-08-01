//! P10 gate for `heh fmt`: formatting every parseable corpus/example program
//! is semantics-preserving (re-parsing yields an equal AST) and idempotent.

use heh::{ast, fmt, lexer, parser::Parser};
use std::fs;
use std::path::Path;

fn parse(src: &str) -> Option<ast::File> {
    let toks = lexer::lex(src).ok()?;
    Parser::new(&toks).parse_file().ok()
}

/// Remove `line:col` tokens from an AST dump. The formatter legitimately
/// changes line numbers (it drops comments and reflows), so structural
/// equality must ignore spans.
fn strip_spans(dump: &str) -> String {
    let chars: Vec<char> = dump.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        // Match a run of digits ':' digits and drop it.
        let start = i;
        let mut j = i;
        while j < chars.len() && chars[j].is_ascii_digit() {
            j += 1;
        }
        if j > i && j < chars.len() && chars[j] == ':' {
            let mut k = j + 1;
            while k < chars.len() && chars[k].is_ascii_digit() {
                k += 1;
            }
            if k > j + 1 {
                i = k; // skip the whole span token
                continue;
            }
        }
        out.push(chars[start]);
        i = start + 1;
    }
    out
}

fn check_one(path: &Path) {
    let src = fs::read_to_string(path).unwrap();
    let name = path.display();

    let Some(ast1) = parse(&src) else { return }; // skip intentional parse-error fixtures

    let f1 = fmt::format_file(&ast1);
    let ast2 = parse(&f1).unwrap_or_else(|| panic!("formatted {name} does not re-parse:\n{f1}"));

    assert_eq!(
        strip_spans(&ast::dump_file(&ast1)),
        strip_spans(&ast::dump_file(&ast2)),
        "fmt changed the AST of {name}\n--- formatted ---\n{f1}"
    );

    let f2 = fmt::format_file(&ast2);
    assert_eq!(f1, f2, "fmt is not idempotent for {name}\n--- pass1 ---\n{f1}\n--- pass2 ---\n{f2}");
}

fn walk(dir: &str) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(path.to_str().unwrap());
        } else if path.extension().and_then(|e| e.to_str()) == Some("heh") {
            check_one(&path);
        }
    }
}

#[test]
fn fmt_is_stable_across_corpus_and_examples() {
    walk("tests/corpus/programs");
    walk("tests/corpus/errors");
    walk("examples");
}
