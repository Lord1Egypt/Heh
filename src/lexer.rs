//! Lexer for Heh (SPEC §2–§5): the full token set plus the layout algorithm
//! of SPEC §3 (INDENT/DEDENT/NEWLINE, exactly 4 spaces per level, tabs in
//! indentation are E0001, bracket continuation). String literals lex their
//! interpolation segments into nested token streams (SPEC §5.3).

use crate::diag::Diag;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kw {
    And,
    Break,
    Continue,
    Elif,
    Else,
    Fn,
    For,
    If,
    In,
    Let,
    Match,
    Mut,
    Not,
    Or,
    Return,
    Try,
    Type,
    Use,
    While,
}

impl Kw {
    pub fn as_str(self) -> &'static str {
        match self {
            Kw::And => "and",
            Kw::Break => "break",
            Kw::Continue => "continue",
            Kw::Elif => "elif",
            Kw::Else => "else",
            Kw::Fn => "fn",
            Kw::For => "for",
            Kw::If => "if",
            Kw::In => "in",
            Kw::Let => "let",
            Kw::Match => "match",
            Kw::Mut => "mut",
            Kw::Not => "not",
            Kw::Or => "or",
            Kw::Return => "return",
            Kw::Try => "try",
            Kw::Type => "type",
            Kw::Use => "use",
            Kw::While => "while",
        }
    }

    fn from_ident(s: &str) -> Option<Kw> {
        Some(match s {
            "and" => Kw::And,
            "break" => Kw::Break,
            "continue" => Kw::Continue,
            "elif" => Kw::Elif,
            "else" => Kw::Else,
            "fn" => Kw::Fn,
            "for" => Kw::For,
            "if" => Kw::If,
            "in" => Kw::In,
            "let" => Kw::Let,
            "match" => Kw::Match,
            "mut" => Kw::Mut,
            "not" => Kw::Not,
            "or" => Kw::Or,
            "return" => Kw::Return,
            "try" => Kw::Try,
            "type" => Kw::Type,
            "use" => Kw::Use,
            "while" => Kw::While,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StrPart {
    Text(String),
    Interp(Vec<Token>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Kw(Kw),
    Ident(String),
    /// Integer literal, raw as written (`0xFF`, `1_000_000`) — parsed in P3.
    Int(String),
    /// Float literal, raw as written (`1.5`, `6.02e23`) — parsed in P3.
    Float(String),
    Str(Vec<StrPart>),
    /// `true` | `false` | `none` (reserved literals, SPEC §4).
    Lit(&'static str),
    Op(&'static str),
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    /// 1-based; columns count Unicode scalar values.
    pub line: u32,
    pub col: u32,
}

const OPS3: [&str; 1] = ["..="];
const OPS2: [&str; 12] = [
    "->", "**", "//", "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "..",
];
const OPS1: [&str; 18] = [
    "+", "-", "*", "/", "%", "<", ">", "=", "(", ")", "[", "]", "{", "}", ",", ":", ".", "?",
];

/// A `#` comment, kept aside so `heh fmt` can put it back. Comments are not
/// tokens — nothing downstream of the lexer sees them except the formatter.
#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    pub line: u32,
    pub col: u32,
    pub text: String,
    /// True when the comment is alone on its line (no code before it).
    pub own_line: bool,
}

pub fn lex(source: &str) -> Result<Vec<Token>, Diag> {
    lex_with_comments(source).map(|(tokens, _)| tokens)
}

/// Lex, also returning every comment in source order (for `heh fmt`).
pub fn lex_with_comments(source: &str) -> Result<(Vec<Token>, Vec<Comment>), Diag> {
    // SPEC §2: \r\n is normalized to \n. A leading BOM is tolerated.
    let source = source.replace("\r\n", "\n");
    let source = source.strip_prefix('\u{feff}').unwrap_or(&source);
    let mut lexer = Lexer::new(source);
    let tokens = lexer.run()?;
    Ok((tokens, std::mem::take(&mut lexer.comments)))
}

/// Stable dump format used by `heh tokens` and the golden tests:
/// one `line:col<TAB>KIND` per token; interpolation dumps nested kinds.
pub fn dump(tokens: &[Token]) -> String {
    let mut out = String::new();
    for t in tokens {
        out.push_str(&format!("{}:{}\t{}\n", t.line, t.col, dump_kind(&t.kind)));
    }
    out
}

fn dump_kind(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Kw(k) => format!("KW({})", k.as_str()),
        TokenKind::Ident(s) => format!("IDENT({s})"),
        TokenKind::Int(s) => format!("INT({s})"),
        TokenKind::Float(s) => format!("FLOAT({s})"),
        TokenKind::Lit(s) => format!("LIT({s})"),
        TokenKind::Op(s) => format!("OP({s})"),
        TokenKind::Str(parts) => {
            let inner: Vec<String> = parts
                .iter()
                .map(|p| match p {
                    StrPart::Text(t) => format!("\"{}\"", escape_text(t)),
                    StrPart::Interp(toks) => {
                        let kinds: Vec<String> = toks.iter().map(|t| dump_kind(&t.kind)).collect();
                        format!("{{{}}}", kinds.join(" "))
                    }
                })
                .collect();
            format!("STR({})", inner.join(" "))
        }
        TokenKind::Newline => "NEWLINE".to_string(),
        TokenKind::Indent => "INDENT".to_string(),
        TokenKind::Dedent => "DEDENT".to_string(),
        TokenKind::Eof => "EOF".to_string(),
    }
}

fn escape_text(t: &str) -> String {
    let mut out = String::new();
    for c in t.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '{' => out.push_str("\\{"),
            _ => out.push(c),
        }
    }
    out
}

struct Lexer {
    chars: Vec<char>,
    i: usize,
    line: u32,
    col: u32,
    indents: Vec<u32>,
    /// Open `( [ {` delimiters: (char, line, col). Non-empty = line continuation.
    open: Vec<(char, u32, u32)>,
    toks: Vec<Token>,
    interp_depth: u32,
    comments: Vec<Comment>,
}

impl Lexer {
    fn new(source: &str) -> Lexer {
        Lexer {
            chars: source.chars().collect(),
            i: 0,
            line: 1,
            col: 1,
            indents: vec![0],
            open: Vec::new(),
            toks: Vec::new(),
            interp_depth: 0,
            comments: Vec::new(),
        }
    }

    fn cur(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }

    fn peek(&self, n: usize) -> Option<char> {
        self.chars.get(self.i + n).copied()
    }

    fn advance(&mut self) -> char {
        let c = self.chars[self.i];
        self.i += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        c
    }

    fn push(&mut self, kind: TokenKind, line: u32, col: u32) {
        self.toks.push(Token { kind, line, col });
    }

    fn err<T>(&self, code: &'static str, msg: &str, line: u32, col: u32) -> Result<T, Diag> {
        Err(Diag {
            code,
            msg: msg.to_string(),
            line,
            col,
        })
    }

    fn run(&mut self) -> Result<Vec<Token>, Diag> {
        loop {
            // Start of a physical line with all brackets closed: measure indentation.
            let ind_line = self.line;
            let mut w: u32 = 0;
            loop {
                match self.cur() {
                    Some(' ') => {
                        self.advance();
                        w += 1;
                    }
                    Some('\t') => {
                        return self.err(
                            "E0001",
                            "tab character in indentation (use 4 spaces)",
                            self.line,
                            self.col,
                        )
                    }
                    _ => break,
                }
            }
            match self.cur() {
                None => break,
                // Blank and comment-only lines produce no layout tokens (SPEC §3).
                Some('\n') => {
                    self.advance();
                    continue;
                }
                Some('#') => {
                    self.skip_comment();
                    if self.cur() == Some('\n') {
                        self.advance();
                    }
                    continue;
                }
                _ => {}
            }
            self.layout(w, ind_line)?;
            self.lex_logical_line()?;
            if self.i >= self.chars.len() {
                break;
            }
        }
        while self.indents.len() > 1 {
            self.indents.pop();
            self.push(TokenKind::Dedent, self.line, 1);
        }
        self.push(TokenKind::Eof, self.line, self.col);
        Ok(std::mem::take(&mut self.toks))
    }

    fn layout(&mut self, w: u32, line: u32) -> Result<(), Diag> {
        let top = *self.indents.last().unwrap(); // stack always holds 0
        if w > top {
            if w != top + 4 {
                return self.err(
                    "E0002",
                    &format!(
                        "invalid indentation: expected {} spaces, found {w}",
                        top + 4
                    ),
                    line,
                    1,
                );
            }
            self.indents.push(w);
            self.push(TokenKind::Indent, line, 1);
        } else {
            while w < *self.indents.last().unwrap() {
                self.indents.pop();
                self.push(TokenKind::Dedent, line, 1);
            }
            if w != *self.indents.last().unwrap() {
                return self.err(
                    "E0002",
                    &format!("invalid indentation: {w} spaces does not match any open block"),
                    line,
                    1,
                );
            }
        }
        Ok(())
    }

    /// Lex until the logical line ends: a newline outside brackets (emits
    /// NEWLINE) or end of file (emits the final NEWLINE; E0006 if brackets
    /// are still open).
    fn lex_logical_line(&mut self) -> Result<(), Diag> {
        loop {
            match self.cur() {
                None => {
                    if let Some((c, l, col)) = self.open.first().copied() {
                        return self.err("E0006", &format!("unclosed '{c}'"), l, col);
                    }
                    self.push(TokenKind::Newline, self.line, self.col);
                    return Ok(());
                }
                Some(' ') | Some('\t') => {
                    self.advance();
                }
                Some('\n') => {
                    let (l, c) = (self.line, self.col);
                    self.advance();
                    if self.open.is_empty() {
                        self.push(TokenKind::Newline, l, c);
                        return Ok(());
                    }
                    // Inside brackets: implicit joining, indentation unmeasured (SPEC §3).
                }
                Some('#') => self.skip_comment(),
                _ => {
                    let tok = self.read_token()?;
                    if let TokenKind::Op(op) = tok.kind {
                        match op {
                            "(" | "[" | "{" => {
                                let c = op.chars().next().unwrap();
                                self.open.push((c, tok.line, tok.col));
                            }
                            ")" | "]" | "}" => {
                                let expect = match op {
                                    ")" => '(',
                                    "]" => '[',
                                    _ => '{',
                                };
                                match self.open.pop() {
                                    Some((c, _, _)) if c == expect => {}
                                    Some((c, _, _)) => {
                                        return self.err(
                                            "E0006",
                                            &format!("mismatched delimiter: '{op}' closes '{c}'"),
                                            tok.line,
                                            tok.col,
                                        )
                                    }
                                    None => {
                                        return self.err(
                                            "E0006",
                                            &format!("unmatched '{op}'"),
                                            tok.line,
                                            tok.col,
                                        )
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    self.toks.push(tok);
                }
            }
        }
    }

    fn skip_comment(&mut self) {
        let (line, col) = (self.line, self.col);
        // Alone on its line if only whitespace precedes it.
        let own_line = self.chars[..self.i]
            .iter()
            .rev()
            .take_while(|&&c| c != '\n')
            .all(|c| c.is_whitespace());
        let mut text = String::new();
        while let Some(c) = self.cur() {
            if c == '\n' {
                break;
            }
            text.push(c);
            self.advance();
        }
        self.comments.push(Comment { line, col, text: text.trim_end().to_string(), own_line });
    }

    fn read_token(&mut self) -> Result<Token, Diag> {
        let (l, c) = (self.line, self.col);
        let ch = self.cur().unwrap();
        let kind = if ch.is_ascii_digit() {
            self.lex_number()?
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            self.lex_word()
        } else if ch == '"' {
            self.lex_string()?
        } else {
            self.lex_op()?
        };
        Ok(Token {
            kind,
            line: l,
            col: c,
        })
    }

    fn lex_word(&mut self) -> TokenKind {
        let start = self.i;
        while let Some(c) = self.cur() {
            if c.is_ascii_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let word: String = self.chars[start..self.i].iter().collect();
        if let Some(kw) = Kw::from_ident(&word) {
            return TokenKind::Kw(kw);
        }
        match word.as_str() {
            "true" => TokenKind::Lit("true"),
            "false" => TokenKind::Lit("false"),
            "none" => TokenKind::Lit("none"),
            _ => TokenKind::Ident(word),
        }
    }

    fn lex_number(&mut self) -> Result<TokenKind, Diag> {
        let start = self.i;
        let mut is_float = false;
        if self.cur() == Some('0') && matches!(self.peek(1), Some('x') | Some('b') | Some('o')) {
            let base = self.peek(1).unwrap();
            self.advance();
            self.advance();
            match base {
                'x' => self.digits(|c| c.is_ascii_hexdigit())?,
                'b' => self.digits(|c| c == '0' || c == '1')?,
                _ => self.digits(|c| ('0'..='7').contains(&c))?,
            }
        } else {
            self.digits(|c| c.is_ascii_digit())?;
            if self.cur() == Some('.') && self.peek(1).is_some_and(|c| c.is_ascii_digit()) {
                is_float = true;
                self.advance();
                self.digits(|c| c.is_ascii_digit())?;
            }
            let exp_follows = self.cur() == Some('e')
                && (self.peek(1).is_some_and(|c| c.is_ascii_digit())
                    || (matches!(self.peek(1), Some('+') | Some('-'))
                        && self.peek(2).is_some_and(|c| c.is_ascii_digit())));
            if exp_follows {
                is_float = true;
                self.advance();
                if matches!(self.cur(), Some('+') | Some('-')) {
                    self.advance();
                }
                self.digits(|c| c.is_ascii_digit())?;
            }
        }
        if let Some(nc) = self.cur() {
            if nc.is_ascii_alphanumeric() || nc == '_' {
                return self.err(
                    "E0005",
                    &format!("invalid character '{nc}' in number literal"),
                    self.line,
                    self.col,
                );
            }
        }
        let raw: String = self.chars[start..self.i].iter().collect();
        Ok(if is_float {
            TokenKind::Float(raw)
        } else {
            TokenKind::Int(raw)
        })
    }

    /// Consume `[digit_]+` where every `_` sits between two digits (SPEC §5.1).
    fn digits(&mut self, is_digit: impl Fn(char) -> bool) -> Result<(), Diag> {
        let mut any = false;
        let mut prev_underscore = false;
        loop {
            match self.cur() {
                Some(c) if is_digit(c) => {
                    any = true;
                    prev_underscore = false;
                    self.advance();
                }
                Some('_') => {
                    if !any || prev_underscore {
                        return self.err(
                            "E0005",
                            "misplaced '_' in number literal (must sit between digits)",
                            self.line,
                            self.col,
                        );
                    }
                    prev_underscore = true;
                    self.advance();
                }
                _ => break,
            }
        }
        if !any {
            return self.err(
                "E0005",
                "number literal needs at least one digit",
                self.line,
                self.col,
            );
        }
        if prev_underscore {
            return self.err(
                "E0005",
                "misplaced '_' in number literal (must sit between digits)",
                self.line,
                self.col.saturating_sub(1),
            );
        }
        Ok(())
    }

    fn lex_op(&mut self) -> Result<TokenKind, Diag> {
        for op in OPS3 {
            if self.matches_op(op) {
                for _ in 0..op.len() {
                    self.advance();
                }
                return Ok(TokenKind::Op(op));
            }
        }
        for op in OPS2 {
            if self.matches_op(op) {
                for _ in 0..op.len() {
                    self.advance();
                }
                return Ok(TokenKind::Op(op));
            }
        }
        for op in OPS1 {
            if self.matches_op(op) {
                self.advance();
                return Ok(TokenKind::Op(op));
            }
        }
        let c = self.cur().unwrap();
        self.err(
            "E0004",
            &format!("unexpected character '{}'", c.escape_default()),
            self.line,
            self.col,
        )
    }

    fn matches_op(&self, op: &str) -> bool {
        op.chars().enumerate().all(|(n, c)| self.peek(n) == Some(c))
    }

    fn lex_string(&mut self) -> Result<TokenKind, Diag> {
        let (sl, sc) = (self.line, self.col);
        self.advance(); // opening quote
        let mut parts: Vec<StrPart> = Vec::new();
        let mut text = String::new();
        loop {
            let Some(ch) = self.cur() else {
                return self.err("E0003", "unclosed string literal", sl, sc);
            };
            if ch == '\n' {
                return self.err(
                    "E0003",
                    "unclosed string literal (strings cannot span lines)",
                    sl,
                    sc,
                );
            }
            if ch == '"' {
                self.advance();
                break;
            }
            if ch == '\\' {
                let (el, ec) = (self.line, self.col);
                self.advance();
                match self.cur() {
                    Some('n') => {
                        self.advance();
                        text.push('\n');
                    }
                    Some('t') => {
                        self.advance();
                        text.push('\t');
                    }
                    Some('\\') => {
                        self.advance();
                        text.push('\\');
                    }
                    Some('"') => {
                        self.advance();
                        text.push('"');
                    }
                    Some('{') => {
                        self.advance();
                        text.push('{');
                    }
                    Some('u') => {
                        self.advance();
                        text.push(self.lex_unicode_escape(el, ec)?);
                    }
                    _ => {
                        return self.err(
                            "E0004",
                            "invalid escape sequence (valid: \\n \\t \\\\ \\\" \\{ \\u{...})",
                            el,
                            ec,
                        )
                    }
                }
                continue;
            }
            if ch == '{' {
                self.advance();
                if !text.is_empty() {
                    parts.push(StrPart::Text(std::mem::take(&mut text)));
                }
                let toks = self.lex_interp(sl, sc)?;
                parts.push(StrPart::Interp(toks));
                continue;
            }
            text.push(ch);
            self.advance();
        }
        if !text.is_empty() || parts.is_empty() {
            parts.push(StrPart::Text(text));
        }
        Ok(TokenKind::Str(parts))
    }

    fn lex_unicode_escape(&mut self, el: u32, ec: u32) -> Result<char, Diag> {
        if self.cur() != Some('{') {
            return self.err("E0004", "invalid escape: expected '\\u{...}'", el, ec);
        }
        self.advance();
        let mut hex = String::new();
        while let Some(h) = self.cur() {
            if h.is_ascii_hexdigit() {
                hex.push(h);
                self.advance();
            } else {
                break;
            }
        }
        if self.cur() != Some('}') || hex.is_empty() || hex.len() > 6 {
            return self.err(
                "E0004",
                "invalid unicode escape (expected 1-6 hex digits in '\\u{...}')",
                el,
                ec,
            );
        }
        self.advance(); // '}'
        let v = u32::from_str_radix(&hex, 16).unwrap(); // ≤6 hex digits always fit u32
        match char::from_u32(v) {
            Some(u) => Ok(u),
            None => self.err(
                "E0004",
                &format!("'\\u{{{hex}}}' is not a valid unicode scalar value"),
                el,
                ec,
            ),
        }
    }

    /// Lex the tokens of one `{expr}` interpolation segment. `braces` tracks
    /// nested `{ }` tokens inside the expression so the closing brace of the
    /// segment is found at depth 0.
    fn lex_interp(&mut self, sl: u32, sc: u32) -> Result<Vec<Token>, Diag> {
        self.interp_depth += 1;
        if self.interp_depth > 16 {
            return self.err(
                "E0004",
                "string interpolation nested too deeply",
                self.line,
                self.col,
            );
        }
        let mut toks: Vec<Token> = Vec::new();
        let mut braces: u32 = 0;
        loop {
            match self.cur() {
                None | Some('\n') => {
                    return self.err("E0003", "unclosed interpolation in string literal", sl, sc)
                }
                Some(' ') | Some('\t') => {
                    self.advance();
                }
                Some('}') if braces == 0 => {
                    self.advance();
                    break;
                }
                _ => {
                    let t = self.read_token()?;
                    match t.kind {
                        TokenKind::Op("{") => braces += 1,
                        TokenKind::Op("}") => braces -= 1,
                        _ => {}
                    }
                    toks.push(t);
                }
            }
        }
        self.interp_depth -= 1;
        if toks.is_empty() {
            return self.err("E0004", "empty interpolation in string literal", sl, sc);
        }
        Ok(toks)
    }
}
