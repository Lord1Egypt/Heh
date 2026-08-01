//! `heh fmt` — the canonical formatter. Emits valid Heh source from the AST
//! with 4-space indentation and no options. It is semantics-preserving
//! (re-parsing the output yields an equal AST) and idempotent
//! (`fmt(fmt(x)) == fmt(x)`).

use crate::ast::*;
use crate::lexer::Comment;

const INDENT: &str = "    ";

/// Comments are not part of the AST, so the formatter replays them from the
/// lexer's side table: each one is re-emitted just before the first construct
/// that starts on a later source line, at that construct's indentation.
/// Comments sharing a line with code are appended to the emitted line.
struct Comments {
    items: Vec<Comment>,
    next: usize,
}

impl Comments {
    fn new(mut items: Vec<Comment>) -> Self {
        items.sort_by_key(|c| (c.line, c.col));
        Self { items, next: 0 }
    }

    /// Emit every comment that came before source line `line`. A comment that
    /// was not claimed as a trailing one by now gets its own line: the
    /// formatter never drops a comment, even if it has to move it.
    fn flush_before(&mut self, line: u32, depth: usize, out: &mut String) {
        while let Some(c) = self.items.get(self.next) {
            if c.line >= line {
                break;
            }
            out.push_str(&format!("{}{}\n", pad(depth), c.text));
            self.next += 1;
        }
    }

    /// Take the trailing comment sitting on source line `line`, if any.
    fn trailing_on(&mut self, line: u32) -> Option<String> {
        let c = self.items.get(self.next)?;
        if c.line == line && !c.own_line {
            let text = c.text.clone();
            self.next += 1;
            return Some(text);
        }
        None
    }

    /// Emit whatever is left once the program has been written out.
    fn flush_rest(&mut self, out: &mut String) {
        while let Some(c) = self.items.get(self.next) {
            out.push_str(&format!("{}\n", c.text));
            self.next += 1;
        }
    }
}

/// Append a trailing comment to the line just written to `out`.
fn attach_trailing(out: &mut String, comment: Option<String>) {
    if let Some(text) = comment {
        if out.ends_with('\n') {
            out.pop();
        }
        out.push_str(&format!("  {}\n", text));
    }
}

pub fn format_file(file: &File) -> String {
    format_file_with_comments(file, Vec::new())
}

pub fn format_file_with_comments(file: &File, comments: Vec<Comment>) -> String {
    let mut out = String::new();
    let cm = &mut Comments::new(comments);

    for u in &file.uses {
        cm.flush_before(u.span.line, 0, &mut out);
        out.push_str(&format!("use {}\n", format_use_path(&u.path)));
        attach_trailing(&mut out, cm.trailing_on(u.span.line));
    }
    if !file.uses.is_empty() && !file.items.is_empty() {
        out.push('\n');
    }

    for (i, item) in file.items.iter().enumerate() {
        if i > 0 && (is_decl(item) || is_decl(&file.items[i - 1])) {
            out.push('\n');
        }
        cm.flush_before(top_item_line(item), 0, &mut out);
        format_top_item(item, &mut out, cm);
    }

    cm.flush_rest(&mut out);

    // Exactly one trailing newline.
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn is_decl(item: &TopItem) -> bool {
    matches!(item, TopItem::Fn(_) | TopItem::Type(_))
}

/// `use "./file.heh"` keeps its quotes; `use std/math` and `use vendor/x` do not.
fn format_use_path(path: &str) -> String {
    if path.ends_with(".heh") || path.starts_with("./") || path.starts_with("../") {
        format!("\"{}\"", path)
    } else {
        path.to_string()
    }
}

/// The source line a top-level item starts on (where its comments belong).
fn top_item_line(item: &TopItem) -> u32 {
    match item {
        TopItem::Fn(f) => f.span.line,
        TopItem::Type(t) => t.span.line,
        TopItem::Let(l) => l.span.line,
        TopItem::Stmt(s) => stmt_line(s),
    }
}

/// The source line a statement starts on.
fn stmt_line(stmt: &Statement) -> u32 {
    match stmt {
        Statement::Let(l) => l.span.line,
        Statement::Assign(a) => a.span.line,
        Statement::Expr(e) => e.span.line,
        Statement::Return(r) => r.span.line,
        Statement::Break(s) | Statement::Continue(s) => s.line,
        Statement::If(i) => i.span.line,
        Statement::While(w) => w.span.line,
        Statement::For(f) => f.span.line,
        Statement::Match(m) => m.span.line,
    }
}

fn format_top_item(item: &TopItem, out: &mut String, cm: &mut Comments) {
    match item {
        TopItem::Fn(f) => format_fn(f, out, cm),
        TopItem::Type(t) => format_type(t, out),
        TopItem::Let(l) => {
            out.push_str(&format_let(l));
            out.push('\n');
            attach_trailing(out, cm.trailing_on(l.span.line));
        }
        TopItem::Stmt(s) => format_stmt(s, 0, out, cm),
    }
}

fn format_fn(f: &FnDecl, out: &mut String, cm: &mut Comments) {
    let mut sig = String::from("fn ");
    if let Some(recv) = &f.receiver {
        sig.push_str(recv);
        sig.push('.');
    }
    sig.push_str(&f.name);
    sig.push('(');
    sig.push_str(&f.params.iter().map(format_param).collect::<Vec<_>>().join(", "));
    sig.push(')');
    if let Some(rt) = &f.ret_type {
        sig.push_str(" -> ");
        sig.push_str(&format_type_expr(rt));
    }
    out.push_str(&sig);
    out.push('\n');
    attach_trailing(out, cm.trailing_on(f.span.line));
    format_block(&f.body, 1, out, cm);
}

fn format_param(p: &Param) -> String {
    match &p.typ {
        Some(t) => format!("{}: {}", p.name, format_type_expr(t)),
        None => p.name.clone(),
    }
}

fn format_type(t: &TypeDecl, out: &mut String) {
    match &t.kind {
        TypeDeclKind::Enum(variants) => {
            let vs: Vec<String> = variants
                .iter()
                .map(|v| {
                    if v.fields.is_empty() {
                        v.name.clone()
                    } else {
                        let fields = v.fields.iter().map(format_field).collect::<Vec<_>>().join(", ");
                        format!("{}({})", v.name, fields)
                    }
                })
                .collect();
            out.push_str(&format!("type {} = {}\n", t.name, vs.join(" or ")));
        }
        TypeDeclKind::Record(fields) => {
            out.push_str(&format!("type {}\n", t.name));
            for field in fields {
                out.push_str(&format!("{}{}\n", INDENT, format_field(field)));
            }
        }
    }
}

fn format_field(f: &FieldDef) -> String {
    format!("{}: {}", f.name, format_type_expr(&f.typ))
}

fn format_type_expr(t: &TypeExpr) -> String {
    let mut s = match &t.kind {
        TypeExprKind::Named(name, args) => {
            if args.is_empty() {
                name.clone()
            } else {
                let a = args.iter().map(format_type_expr).collect::<Vec<_>>().join(", ");
                format!("{}[{}]", name, a)
            }
        }
        TypeExprKind::Fn(args, ret) => {
            let a = args.iter().map(format_type_expr).collect::<Vec<_>>().join(", ");
            match ret {
                Some(r) => format!("fn({}) -> {}", a, format_type_expr(r)),
                None => format!("fn({})", a),
            }
        }
    };
    if t.optional {
        s.push('?');
    }
    if t.result {
        s.push_str(" or error");
    }
    s
}

fn format_block(block: &Block, depth: usize, out: &mut String, cm: &mut Comments) {
    for stmt in &block.stmts {
        cm.flush_before(stmt_line(stmt), depth, out);
        format_stmt(stmt, depth, out, cm);
    }
}

fn pad(depth: usize) -> String {
    INDENT.repeat(depth)
}

fn format_stmt(stmt: &Statement, depth: usize, out: &mut String, cm: &mut Comments) {
    let p = pad(depth);
    let head_line = stmt_line(stmt);
    match stmt {
        Statement::Let(l) => out.push_str(&format!("{}{}\n", p, format_let(l))),
        Statement::Assign(a) => {
            out.push_str(&format!("{}{} {} {}\n", p, format_lvalue(&a.target), assign_op(&a.op), format_expr(&a.rhs, 0)));
        }
        Statement::Expr(e) => out.push_str(&format!("{}{}\n", p, format_expr(e, 0))),
        Statement::Return(r) => match &r.expr {
            Some(e) => out.push_str(&format!("{}return {}\n", p, format_expr(e, 0))),
            None => out.push_str(&format!("{}return\n", p)),
        },
        Statement::Break(_) => out.push_str(&format!("{}break\n", p)),
        Statement::Continue(_) => out.push_str(&format!("{}continue\n", p)),
        Statement::If(i) => {
            out.push_str(&format!("{}if {}\n", p, format_expr(&i.cond, 0)));
            attach_trailing(out, cm.trailing_on(head_line));
            format_block(&i.then_block, depth + 1, out, cm);
            for (cond, block) in &i.elifs {
                cm.flush_before(cond.span.line, depth, out);
                out.push_str(&format!("{}elif {}\n", p, format_expr(cond, 0)));
                attach_trailing(out, cm.trailing_on(cond.span.line));
                format_block(block, depth + 1, out, cm);
            }
            if let Some(else_block) = &i.else_block {
                out.push_str(&format!("{}else\n", p));
                format_block(else_block, depth + 1, out, cm);
            }
        }
        Statement::While(w) => {
            out.push_str(&format!("{}while {}\n", p, format_expr(&w.cond, 0)));
            attach_trailing(out, cm.trailing_on(head_line));
            format_block(&w.body, depth + 1, out, cm);
        }
        Statement::For(f) => {
            out.push_str(&format!("{}for {} in {}\n", p, f.name, format_expr(&f.iter, 0)));
            attach_trailing(out, cm.trailing_on(head_line));
            format_block(&f.body, depth + 1, out, cm);
        }
        Statement::Match(m) => {
            out.push_str(&format!("{}match {}\n", p, format_expr(&m.expr, 0)));
            attach_trailing(out, cm.trailing_on(head_line));
            for arm in &m.arms {
                out.push_str(&format!("{}{}\n", pad(depth + 1), format_pattern(&arm.pattern)));
                format_block(&arm.body, depth + 2, out, cm);
            }
        }
    }
    // Single-line statements carry any comment that shared their line.
    if !matches!(stmt, Statement::If(_) | Statement::While(_) | Statement::For(_) | Statement::Match(_)) {
        attach_trailing(out, cm.trailing_on(head_line));
    }
}

fn format_let(l: &LetStmt) -> String {
    // A mutable binding is introduced with `mut`, an immutable one with `let`.
    let kw = if l.is_mut { "mut" } else { "let" };
    format!("{} {} = {}", kw, l.name, format_expr(&l.init, 0))
}

fn assign_op(op: &AssignOp) -> &'static str {
    match op {
        AssignOp::Eq => "=",
        AssignOp::AddEq => "+=",
        AssignOp::SubEq => "-=",
        AssignOp::MulEq => "*=",
        AssignOp::DivEq => "/=",
    }
}

fn format_lvalue(lv: &LValue) -> String {
    let mut s = lv.name.clone();
    for tail in &lv.tail {
        match tail {
            LValueTail::Field(f) => {
                s.push('.');
                s.push_str(f);
            }
            LValueTail::Index(e) => {
                s.push('[');
                s.push_str(&format_expr(e, 0));
                s.push(']');
            }
        }
    }
    s
}

fn format_pattern(pat: &Pattern) -> String {
    match pat {
        Pattern::Wildcard(_) => "_".to_string(),
        Pattern::Literal(lit) => format_literal(lit),
        Pattern::Variant(_, name, binds) => {
            if binds.is_empty() {
                name.clone()
            } else {
                format!("{}({})", name, binds.join(", "))
            }
        }
    }
}

// --------------------------------------------------------------------------
// Expressions (precedence-aware; minimal parentheses)
// --------------------------------------------------------------------------

/// Binding level of an expression's outermost operator. Higher binds tighter.
/// `try`/closures are 0 so they parenthesize whenever used as an operand.
fn expr_prec(e: &Expr) -> u8 {
    match &e.kind {
        ExprKind::Try(..) | ExprKind::Closure(..) => 0,
        ExprKind::Binary(op, _, _) => bin_level(op),
        ExprKind::Unary(..) => 8,
        ExprKind::Call(..) | ExprKind::Field(..) | ExprKind::Index(..) => 9,
        _ => 10,
    }
}

fn bin_level(op: &BinOp) -> u8 {
    match op {
        BinOp::Or => 1,
        BinOp::And => 2,
        BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Leq | BinOp::Gt | BinOp::Geq => 3,
        BinOp::Range | BinOp::RangeInc => 4,
        BinOp::Add | BinOp::Sub => 5,
        BinOp::Mul | BinOp::Div | BinOp::FloorDiv | BinOp::Mod => 6,
        BinOp::Pow => 7,
    }
}

/// Minimum precedence required for the (left, right) operands of a binary op,
/// encoding associativity: left-assoc adds, non-assoc cmp/range, right-assoc pow.
fn operand_mins(op: &BinOp) -> (u8, u8) {
    match op {
        BinOp::Or => (1, 2),
        BinOp::And => (2, 3),
        BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Leq | BinOp::Gt | BinOp::Geq => (4, 4),
        BinOp::Range | BinOp::RangeInc => (5, 5),
        BinOp::Add | BinOp::Sub => (5, 6),
        BinOp::Mul | BinOp::Div | BinOp::FloorDiv | BinOp::Mod => (6, 7),
        BinOp::Pow => (8, 7),
    }
}

fn format_expr(e: &Expr, min_prec: u8) -> String {
    let raw = format_expr_raw(e);
    if expr_prec(e) < min_prec {
        format!("({})", raw)
    } else {
        raw
    }
}

fn format_expr_raw(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Ident(name) => name.clone(),
        ExprKind::Literal(lit) => format_literal(lit),
        ExprKind::Binary(op, l, r) => {
            let (lmin, rmin) = operand_mins(op);
            // Ranges are written tight (`1..=n`, `0..`); an unbounded range is
            // stored as Range(left, none) and printed without a right operand.
            if matches!(op, BinOp::Range | BinOp::RangeInc) {
                let left = format_expr(l, lmin);
                let sym = bin_symbol(op);
                if matches!(&r.kind, ExprKind::Literal(Literal::None)) {
                    return format!("{}{}", left, sym);
                }
                return format!("{}{}{}", left, sym, format_expr(r, rmin));
            }
            format!("{} {} {}", format_expr(l, lmin), bin_symbol(op), format_expr(r, rmin))
        }
        ExprKind::Unary(op, inner) => {
            let sym = match op { UnOp::Neg => "-", UnOp::Not => "not " };
            format!("{}{}", sym, format_expr(inner, 9))
        }
        ExprKind::Call(callee, args) => {
            let a = args.iter().map(format_call_arg).collect::<Vec<_>>().join(", ");
            format!("{}({})", format_expr(callee, 9), a)
        }
        ExprKind::Field(obj, name) => format!("{}.{}", format_expr(obj, 9), name),
        ExprKind::Index(obj, idx) => format!("{}[{}]", format_expr(obj, 9), format_expr(idx, 0)),
        ExprKind::List(items) => {
            let a = items.iter().map(|i| format_expr(i, 0)).collect::<Vec<_>>().join(", ");
            format!("[{}]", a)
        }
        ExprKind::Map(pairs) => {
            if pairs.is_empty() {
                "{}".to_string()
            } else {
                let a = pairs.iter().map(|(k, v)| format!("{}: {}", format_expr(k, 0), format_expr(v, 0))).collect::<Vec<_>>().join(", ");
                format!("{{{}}}", a)
            }
        }
        ExprKind::Record(name, fields) => {
            let a = fields.iter().map(|(f, v)| format!("{}: {}", f, format_expr(v, 0))).collect::<Vec<_>>().join(", ");
            format!("{}{{{}}}", name, a)
        }
        ExprKind::Try(inner, else_exit) => {
            if *else_exit {
                format!("try {} else exit", format_expr(inner, 0))
            } else {
                format!("try {}", format_expr(inner, 0))
            }
        }
        ExprKind::Closure(params, ret, body) => {
            let ps = params.iter().map(format_param).collect::<Vec<_>>().join(", ");
            let mut s = format!("fn({})", ps);
            if let Some(r) = ret {
                s.push_str(&format!(" -> {}", format_type_expr(r)));
            }
            s.push('\n');
            // A closure body is built inside an expression, with no cursor to
            // hand; comments inside one are recovered by the enclosing
            // statement's next flush rather than being lost.
            format_block(body, 1, &mut s, &mut Comments::new(Vec::new()));
            // trim the trailing newline so callers control layout
            if s.ends_with('\n') {
                s.pop();
            }
            s
        }
        ExprKind::InterpStr(parts) => {
            let mut s = String::from("\"");
            for part in parts {
                match part {
                    InterpPart::Text(t) => s.push_str(&escape_str(t)),
                    InterpPart::Expr(e) => {
                        s.push('{');
                        s.push_str(&format_expr(e, 0));
                        s.push('}');
                    }
                }
            }
            s.push('"');
            s
        }
    }
}

fn format_call_arg(arg: &CallArg) -> String {
    match arg {
        CallArg::Positional(e) => format_expr(e, 0),
        CallArg::Named(name, e) => format!("{}: {}", name, format_expr(e, 0)),
    }
}

fn bin_symbol(op: &BinOp) -> &'static str {
    match op {
        BinOp::Or => "or",
        BinOp::And => "and",
        BinOp::Eq => "==",
        BinOp::Neq => "!=",
        BinOp::Lt => "<",
        BinOp::Leq => "<=",
        BinOp::Gt => ">",
        BinOp::Geq => ">=",
        BinOp::Range => "..",
        BinOp::RangeInc => "..=",
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::FloorDiv => "//",
        BinOp::Mod => "%",
        BinOp::Pow => "**",
    }
}

fn format_literal(lit: &Literal) -> String {
    match lit {
        Literal::Int(s) => s.clone(),
        Literal::Float(s) => s.clone(),
        Literal::Bool(b) => b.to_string(),
        Literal::None => "none".to_string(),
        Literal::Str(s) => format!("\"{}\"", escape_str(s)),
    }
}

/// Re-escape a decoded string for emission (matches the lexer's valid escapes:
/// `\n \t \\ \" \{ \u{...}`; `{` must be escaped so it is not read as interp).
fn escape_str(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '{' => out.push_str("\\{"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
