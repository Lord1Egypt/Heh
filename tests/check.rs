//! Static-checker regressions: errors promised as compile-time diagnostics
//! must be emitted before either execution engine runs.

use heh::check::Checker;
use heh::lexer::lex;
use heh::parser::Parser;

fn diagnostic_codes(source: &str) -> Vec<&'static str> {
    let tokens = lex(source).expect("test source should lex");
    let file = Parser::new(&tokens)
        .parse_file()
        .expect("test source should parse");
    let mut checker = Checker::new();
    checker.check_file(&file);
    checker.diags.iter().map(|diag| diag.code).collect()
}

#[test]
fn rejects_non_exhaustive_enum_match_statically() {
    let source = "type Choice = yes or no\nfn pick(x: Choice)\n    match x\n        yes\n            return\n";
    assert!(diagnostic_codes(source).contains(&"E0020"));
}

#[test]
fn accepts_exhaustive_enum_match_statically() {
    let source = "type Choice = yes or no\nfn pick(x: Choice)\n    match x\n        yes\n            return\n        no\n            return\n";
    assert!(!diagnostic_codes(source).contains(&"E0020"));
}

#[test]
fn rejects_wrong_function_arity_and_argument_type() {
    let wrong_arity = "fn add(a: int, b: int) -> int\n    a + b\nsys.print(add(1))\n";
    assert!(diagnostic_codes(wrong_arity).contains(&"E0109"));

    let wrong_type = "fn add(a: int, b: int) -> int\n    a + b\nsys.print(add(1, \"two\"))\n";
    assert!(diagnostic_codes(wrong_type).contains(&"E0040"));
}
