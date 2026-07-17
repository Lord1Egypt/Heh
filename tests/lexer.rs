//! P1 gate: unit tests for every token class and layout edge case, plus
//! golden token dumps for every example program.

use heh::lexer::{dump, lex, Kw, StrPart, Token, TokenKind};

fn kinds(src: &str) -> Vec<TokenKind> {
    lex(src)
        .unwrap_or_else(|d| panic!("lex failed: [{}] {} at {}:{}", d.code, d.msg, d.line, d.col))
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

fn code_of(src: &str) -> &'static str {
    lex(src).expect_err("expected a lex error").code
}

fn op(s: &'static str) -> TokenKind {
    TokenKind::Op(s)
}
fn ident(s: &str) -> TokenKind {
    TokenKind::Ident(s.to_string())
}
fn int(s: &str) -> TokenKind {
    TokenKind::Int(s.to_string())
}
fn float(s: &str) -> TokenKind {
    TokenKind::Float(s.to_string())
}

// --- keywords, identifiers, reserved literals -------------------------------

#[test]
fn all_19_keywords_lex_as_keywords() {
    let src = "and break continue elif else fn for if in let match mut not or return try type use while\n";
    let toks = kinds(src);
    let kws: Vec<&TokenKind> = toks
        .iter()
        .filter(|k| matches!(k, TokenKind::Kw(_)))
        .collect();
    assert_eq!(kws.len(), 19);
    assert_eq!(toks[0], TokenKind::Kw(Kw::And));
    assert_eq!(toks[18], TokenKind::Kw(Kw::While));
}

#[test]
fn reserved_literals_and_idents() {
    assert_eq!(
        kinds("true false none ok err some _ _hidden x9\n"),
        vec![
            TokenKind::Lit("true"),
            TokenKind::Lit("false"),
            TokenKind::Lit("none"),
            ident("ok"),
            ident("err"),
            ident("some"),
            ident("_"),
            ident("_hidden"),
            ident("x9"),
            TokenKind::Newline,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn keyword_prefix_is_still_an_ident() {
    assert_eq!(
        kinds("iffy formal lettuce\n")[..3].to_vec(),
        vec![ident("iffy"), ident("formal"), ident("lettuce")]
    );
}

// --- numbers -----------------------------------------------------------------

#[test]
fn int_literals_all_bases() {
    assert_eq!(
        kinds("0 42 1_000_000 0xFF 0xdead_beef 0b1010 0o755\n")[..7].to_vec(),
        vec![
            int("0"),
            int("42"),
            int("1_000_000"),
            int("0xFF"),
            int("0xdead_beef"),
            int("0b1010"),
            int("0o755"),
        ]
    );
}

#[test]
fn float_literals() {
    assert_eq!(
        kinds("1.5 2.0 6.02e23 1e5 1.5e-3 1.5e+3 1_000.5\n")[..7].to_vec(),
        vec![
            float("1.5"),
            float("2.0"),
            float("6.02e23"),
            float("1e5"),
            float("1.5e-3"),
            float("1.5e+3"),
            float("1_000.5"),
        ]
    );
}

#[test]
fn range_is_not_a_float() {
    assert_eq!(
        kinds("1..5\n")[..3].to_vec(),
        vec![int("1"), op(".."), int("5")]
    );
    assert_eq!(
        kinds("1..=5\n")[..3].to_vec(),
        vec![int("1"), op("..="), int("5")]
    );
    assert_eq!(
        kinds("1.5..2.5\n")[..3].to_vec(),
        vec![float("1.5"), op(".."), float("2.5")]
    );
}

#[test]
fn bad_numbers_are_e0005() {
    for src in [
        "1_\n", "1__0\n", "0x_F\n", "0x\n", "0b12\n", "123abc\n", "1.5e\n",
    ] {
        assert_eq!(code_of(src), "E0005", "for {src:?}");
    }
}

#[test]
fn bare_e_suffix_is_not_an_exponent() {
    // `1.5e` (no digits) errors, but `1.5 end`-style idents after a space are fine
    assert_eq!(
        kinds("x = 1.5\n")[..3].to_vec(),
        vec![ident("x"), op("="), float("1.5")]
    );
}

// --- strings and interpolation ------------------------------------------------

fn parts_of(k: &TokenKind) -> &Vec<StrPart> {
    match k {
        TokenKind::Str(p) => p,
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn plain_string_with_unicode_and_escapes() {
    let toks = kinds("\"heh 𓁨 \\n \\t \\\\ \\\" \\{ \\u{1F40D}\"\n");
    let parts = parts_of(&toks[0]);
    assert_eq!(
        parts,
        &vec![StrPart::Text("heh 𓁨 \n \t \\ \" { 🐍".to_string())]
    );
}

#[test]
fn interpolation_lexes_to_parts() {
    let toks = kinds("\"sum is {a + b}!\"\n");
    let parts = parts_of(&toks[0]);
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], StrPart::Text("sum is ".to_string()));
    match &parts[1] {
        StrPart::Interp(inner) => {
            let inner_kinds: Vec<&TokenKind> = inner.iter().map(|t| &t.kind).collect();
            assert_eq!(inner_kinds, vec![&ident("a"), &op("+"), &ident("b")]);
        }
        other => panic!("expected Interp, got {other:?}"),
    }
    assert_eq!(parts[2], StrPart::Text("!".to_string()));
}

#[test]
fn nested_string_inside_interpolation() {
    // the caps.heh case: "{text.split("\n").len()} lines"
    let toks = kinds("\"{s.split(\"\\n\").len()} lines\"\n");
    let parts = parts_of(&toks[0]);
    match &parts[0] {
        StrPart::Interp(inner) => {
            let nested = inner
                .iter()
                .find(|t| matches!(t.kind, TokenKind::Str(_)))
                .expect("nested string token");
            assert_eq!(
                parts_of(&nested.kind),
                &vec![StrPart::Text("\n".to_string())]
            );
        }
        other => panic!("expected Interp, got {other:?}"),
    }
    assert_eq!(parts[1], StrPart::Text(" lines".to_string()));
}

#[test]
fn braces_nest_inside_interpolation() {
    let toks = kinds("\"{ {\"a\": 1}.len() }\"\n");
    let parts = parts_of(&toks[0]);
    assert_eq!(parts.len(), 1);
    assert!(matches!(parts[0], StrPart::Interp(_)));
}

#[test]
fn string_errors() {
    assert_eq!(code_of("\"unclosed\n"), "E0003");
    assert_eq!(code_of("\"unclosed"), "E0003");
    assert_eq!(code_of("\"{x\n"), "E0003");
    assert_eq!(code_of("\"bad \\q escape\"\n"), "E0004");
    assert_eq!(code_of("\"{}\"\n"), "E0004"); // empty interpolation
    assert_eq!(code_of("\"\\u{}\"\n"), "E0004");
    assert_eq!(code_of("\"\\u{110000}\"\n"), "E0004"); // beyond unicode
}

#[test]
fn empty_string_is_one_empty_text_part() {
    let toks = kinds("\"\"\n");
    assert_eq!(parts_of(&toks[0]), &vec![StrPart::Text(String::new())]);
}

// --- layout -------------------------------------------------------------------

#[test]
fn basic_indent_dedent() {
    let src = "if x\n    y\nz\n";
    assert_eq!(
        kinds(src),
        vec![
            TokenKind::Kw(Kw::If),
            ident("x"),
            TokenKind::Newline,
            TokenKind::Indent,
            ident("y"),
            TokenKind::Newline,
            TokenKind::Dedent,
            ident("z"),
            TokenKind::Newline,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn eof_closes_all_blocks() {
    let src = "if a\n    if b\n        c";
    let toks = kinds(src);
    let tail: Vec<&TokenKind> = toks.iter().rev().take(4).collect();
    // reversed: EOF, DEDENT, DEDENT, NEWLINE
    assert_eq!(tail[0], &TokenKind::Eof);
    assert_eq!(tail[1], &TokenKind::Dedent);
    assert_eq!(tail[2], &TokenKind::Dedent);
    assert_eq!(tail[3], &TokenKind::Newline);
}

#[test]
fn blank_and_comment_lines_emit_nothing() {
    let with_noise = "if x\n\n    # a comment\n\n    y\n";
    let without = "if x\n    y\n";
    assert_eq!(kinds(with_noise), kinds(without));
}

#[test]
fn tab_in_indentation_is_e0001() {
    assert_eq!(code_of("if x\n\ty\n"), "E0001");
    let d = lex("if x\n\ty\n").unwrap_err();
    assert_eq!((d.line, d.col), (2, 1));
}

#[test]
fn over_indent_is_e0002() {
    assert_eq!(code_of("if x\n        y\n"), "E0002"); // 8 spaces, expected 4
    assert_eq!(code_of("if x\n  y\n"), "E0002"); // 2 spaces
}

#[test]
fn bad_dedent_is_e0002() {
    assert_eq!(code_of("if x\n    y\n  z\n"), "E0002");
}

#[test]
fn mid_line_tabs_are_plain_whitespace() {
    assert_eq!(
        kinds("let\tx\t=\t1\n")[..4].to_vec(),
        vec![TokenKind::Kw(Kw::Let), ident("x"), op("="), int("1")]
    );
}

// --- bracket continuation -------------------------------------------------------

#[test]
fn brackets_join_lines_without_layout() {
    let src = "let xs = [1,\n    2,\n  3]\n";
    let toks = kinds(src);
    assert!(
        !toks.contains(&TokenKind::Indent),
        "no INDENT inside brackets"
    );
    assert_eq!(
        toks.iter().filter(|k| **k == TokenKind::Newline).count(),
        1,
        "one logical line"
    );
}

#[test]
fn comment_inside_continuation_is_skipped() {
    let src = "let xs = [1,  # first\n    2]\n";
    let toks = kinds(src);
    assert_eq!(toks.iter().filter(|k| **k == TokenKind::Newline).count(), 1);
}

#[test]
fn delimiter_errors_are_e0006() {
    assert_eq!(code_of("let xs = [1, 2\n"), "E0006"); // unclosed at EOF
    assert_eq!(code_of("let x = (1]\n"), "E0006"); // mismatched
    assert_eq!(code_of("let x = 1)\n"), "E0006"); // unmatched close
}

// --- misc ----------------------------------------------------------------------

#[test]
fn crlf_is_normalized() {
    assert_eq!(kinds("if x\r\n    y\r\n"), kinds("if x\n    y\n"));
}

#[test]
fn missing_trailing_newline_still_ends_the_line() {
    assert_eq!(kinds("let x = 1"), kinds("let x = 1\n"));
}

#[test]
fn all_operators_lex() {
    let src = "a ..= b -> ** // == != <= >= += -= *= /= .. + - * / % < > = , : . ?\n";
    let toks = kinds(src);
    let ops: Vec<&TokenKind> = toks
        .iter()
        .filter(|k| matches!(k, TokenKind::Op(_)))
        .collect();
    assert_eq!(ops.len(), 25);
    assert_eq!(*ops[0], op("..="));
    assert_eq!(*ops[12], op(".."));
}

#[test]
fn positions_are_line_col_of_token_start() {
    let toks = lex("let x = \"hi\"\n").unwrap();
    let find = |pred: fn(&Token) -> bool| toks.iter().find(|t| pred(t)).unwrap();
    let x = find(|t| t.kind == TokenKind::Ident("x".to_string()));
    assert_eq!((x.line, x.col), (1, 5));
    let s = find(|t| matches!(t.kind, TokenKind::Str(_)));
    assert_eq!((s.line, s.col), (1, 9));
}

#[test]
fn unicode_columns_count_chars() {
    // 𓁨 is one char: the token after it starts 4 cols after the quote closes
    let toks = lex("\"𓁨\" + x\n").unwrap();
    let plus = toks.iter().find(|t| t.kind == TokenKind::Op("+")).unwrap();
    assert_eq!((plus.line, plus.col), (1, 5));
}

#[test]
fn unexpected_character_is_e0004() {
    assert_eq!(code_of("let x = @\n"), "E0004");
    assert_eq!(code_of("let x = ;\n"), "E0004");
}

// --- golden dumps for every example (the P1 conformance seed) --------------------

#[test]
fn golden_example_dumps() {
    let root = env!("CARGO_MANIFEST_DIR");
    for name in ["hello", "infinity", "fizzbuzz", "shapes", "errors", "caps"] {
        let src = std::fs::read_to_string(format!("{root}/examples/{name}.heh")).unwrap();
        let toks = lex(&src)
            .unwrap_or_else(|d| panic!("{name}.heh failed to lex:\n{}", d.render(name, &src)));
        let got = dump(&toks);
        let want = std::fs::read_to_string(format!("{root}/tests/golden/lexer/{name}.tokens"))
            .unwrap_or_else(|e| panic!("missing golden for {name}: {e}"));
        assert_eq!(
            got, want,
            "golden mismatch for {name}.heh — if the change is intended, regenerate \
             with `cargo run -- tokens examples/{name}.heh`, REVIEW the diff by eye, \
             and justify it in the PR"
        );
    }
}
