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

#[test]
fn checks_capability_members_arity_and_argument_types() {
    let valid = "fn main(sys: Sys)\n    let p = sys.args\n    let now = sys.clock.now()\n    let text = sys.fs.read(\"notes.txt\")\n    sys.print(p, now, text)\n";
    assert!(diagnostic_codes(valid).is_empty());

    let wrong_arity = "fn main(sys: Sys)\n    sys.clock.now(1)\n";
    assert!(diagnostic_codes(wrong_arity).contains(&"E0109"));

    let wrong_type = "fn main(sys: Sys)\n    sys.fs.read(42)\n";
    assert!(diagnostic_codes(wrong_type).contains(&"E0040"));

    let unknown = "fn main(sys: Sys)\n    sys.fs.teleport(\"somewhere\")\n";
    assert!(diagnostic_codes(unknown).contains(&"E0053"));
}

#[test]
fn checks_std_modules_and_builtin_methods() {
    let valid = "use std/math\nuse std/hash\nlet words = \"a,b\".split(\",\")\nlet digest = hash.sha256(words.join(\"\"))\nlet root = math.sqrt(4.0)\n";
    assert!(diagnostic_codes(valid).is_empty());

    let wrong_module_arg = "use std/math\nlet x = math.sqrt(4)\n";
    assert!(diagnostic_codes(wrong_module_arg).contains(&"E0040"));

    let wrong_method_arg = "let xs = [1, 2]\nxs.push(\"three\")\n";
    assert!(diagnostic_codes(wrong_method_arg).contains(&"E0040"));

    let unknown_module_member = "use std/hash\nlet x = hash.md5(\"x\")\n";
    assert!(diagnostic_codes(unknown_module_member).contains(&"E0053"));
}
