//! Static-checker regressions: errors promised as compile-time diagnostics
//! must be emitted before either execution engine runs.

use heh::check::Checker;
use heh::lexer::lex;
use heh::parser::Parser;
use std::path::Path;

fn diagnostic_codes(source: &str) -> Vec<&'static str> {
    let tokens = lex(source).expect("test source should lex");
    let file = Parser::new(&tokens)
        .parse_file()
        .expect("test source should parse");
    let mut checker = Checker::new();
    checker.check_file(&file);
    checker.diags.iter().map(|diag| diag.code).collect()
}

fn diagnostic_codes_at(source: &str, path: &Path) -> Vec<&'static str> {
    let tokens = lex(source).expect("test source should lex");
    let file = Parser::new(&tokens)
        .parse_file()
        .expect("test source should parse");
    let mut checker = Checker::new();
    checker.check_file_at(&file, path);
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

#[test]
fn infers_polymorphic_builtin_results_and_rejects_bad_conversions() {
    let valid = "let parsed = int_of(\"42\")\nlet maybe = some(1)\nlet copied = list([1, 2])\nlet chars = list(\"heh\")\nlet ks = list({\"a\": 1})\nlet n = len(copied)\n";
    assert!(diagnostic_codes(valid).is_empty());

    assert!(diagnostic_codes("let x = int(\"42\")\n").contains(&"E0040"));
    assert!(diagnostic_codes("let x = float(true)\n").contains(&"E0040"));
    assert!(diagnostic_codes("let x = int_of(42)\n").contains(&"E0040"));
    assert!(diagnostic_codes("let x = list(42)\n").contains(&"E0040"));
    assert!(diagnostic_codes("let x = some()\n").contains(&"E0109"));
}

#[test]
fn checks_calls_through_local_module_interfaces() {
    let root = std::env::temp_dir().join(format!("heh-check-module-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("create module fixture directory");
    std::fs::write(
        root.join("maths.heh"),
        "type Point\n    x: int\n    y: int\ntype Shape = circle(r: float) or dot\nlet answer = 42\nfn add(a: int, b: int) -> int\n    a + b\n",
    )
    .expect("write module fixture");
    let main_path = root.join("main.heh");

    let valid = "use \"./maths.heh\"\nlet answer = maths.add(20, 22) + maths.answer\nlet p = maths.Point(x: 1, y: 2)\nlet shape = maths.circle(2.0)\n";
    assert!(diagnostic_codes_at(valid, &main_path).is_empty());

    let wrong_type = "use \"./maths.heh\"\nlet answer = maths.add(20, \"22\")\n";
    assert!(diagnostic_codes_at(wrong_type, &main_path).contains(&"E0040"));

    let wrong_arity = "use \"./maths.heh\"\nlet answer = maths.add(20)\n";
    assert!(diagnostic_codes_at(wrong_arity, &main_path).contains(&"E0109"));

    let wrong_field = "use \"./maths.heh\"\nlet p = maths.Point(x: 1, z: 2)\n";
    assert!(diagnostic_codes_at(wrong_field, &main_path).contains(&"E0109"));
    let wrong_payload = "use \"./maths.heh\"\nlet shape = maths.circle(\"large\")\n";
    assert!(diagnostic_codes_at(wrong_payload, &main_path).contains(&"E0040"));

    std::fs::write(
        root.join("cycle_a.heh"),
        "use \"./cycle_b.heh\"\nfn a() -> int\n    1\n",
    )
    .expect("write first cycle fixture");
    std::fs::write(
        root.join("cycle_b.heh"),
        "use \"./cycle_a.heh\"\nfn b() -> int\n    2\n",
    )
    .expect("write second cycle fixture");
    let cycle = "use \"./cycle_a.heh\"\nlet answer = cycle_a.a()\n";
    assert!(diagnostic_codes_at(cycle, &main_path).contains(&"E0030"));

    std::fs::write(
        root.join("broken.heh"),
        "fn answer() -> int\n    return \"not an int\"\n",
    )
    .expect("write invalid module fixture");
    let broken = "use \"./broken.heh\"\nlet answer = broken.answer()\n";
    assert!(diagnostic_codes_at(broken, &main_path).contains(&"E0033"));
}

#[test]
fn checks_record_and_enum_constructor_shapes() {
    let prefix = "type Point\n    x: int\n    y: int\ntype Shape = circle(r: float) or dot\n";
    assert!(diagnostic_codes(&format!(
        "{prefix}let p = Point(x: 1, y: 2)\nlet s = circle(2.0)\nlet d = dot\n"
    ))
    .is_empty());
    assert!(diagnostic_codes(&format!("{prefix}let p = Point(x: 1)\n")).contains(&"E0109"));
    assert!(diagnostic_codes(&format!("{prefix}let p = Point(x: 1, x: 2)\n")).contains(&"E0109"));
    assert!(
        diagnostic_codes(&format!("{prefix}let p = Point(x: 1, y: \"two\")\n")).contains(&"E0040")
    );
    assert!(diagnostic_codes(&format!("{prefix}let s = circle()\n")).contains(&"E0109"));
}
