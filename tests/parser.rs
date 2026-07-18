use heh::ast;
use heh::lexer;
use heh::parser::Parser;
use std::fs;

fn check_golden(name: &str) {
    let source_path = format!("examples/{}.heh", name);
    let golden_path = format!("tests/golden/parser/{}.ast", name);

    let source = fs::read_to_string(&source_path).unwrap();
    let tokens = lexer::lex(&source).unwrap();
    let mut parser = Parser::new(&tokens);
    let ast = parser.parse_file().unwrap();
    let actual = ast::dump_file(&ast);

    if let Ok(expected) = fs::read_to_string(&golden_path) {
        if actual != expected {
            // Write actual to a temp file for easy diffing if needed, but here just assert.
            // Normally we'd use pretty_assertions or just panic.
            if std::env::var("UPDATE_GOLDEN").is_ok() {
                fs::write(&golden_path, actual).unwrap();
            } else {
                assert_eq!(actual, expected, "Golden mismatch for {}", name);
            }
        }
    } else {
        // If file doesn't exist, create it (useful for first run)
        fs::write(&golden_path, actual).unwrap();
    }
}

#[test]
fn golden_example_dumps() {
    check_golden("hello");
    check_golden("fizzbuzz");
    check_golden("infinity");
    check_golden("shapes");
    check_golden("errors");
    check_golden("caps");
}
