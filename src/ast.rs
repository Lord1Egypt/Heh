#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub uses: Vec<UseDecl>,
    pub items: Vec<TopItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    pub span: Span,
    pub path: String, // from path_ident or string
}

#[derive(Debug, Clone, PartialEq)]
pub enum TopItem {
    Fn(FnDecl),
    Type(TypeDecl),
    Let(LetStmt),
    Stmt(Statement),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    pub span: Span,
    pub receiver: Option<String>,
    pub name: String,
    pub params: Vec<Param>,
    pub ret_type: Option<TypeExpr>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub span: Span,
    pub name: String,
    pub typ: Option<TypeExpr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeDecl {
    pub span: Span,
    pub name: String,
    pub kind: TypeDeclKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeDeclKind {
    Record(Vec<FieldDef>),
    Enum(Vec<VariantDef>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    pub span: Span,
    pub name: String,
    pub typ: TypeExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VariantDef {
    pub span: Span,
    pub name: String,
    pub fields: Vec<FieldDef>, // Empty if no fields
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeExpr {
    pub span: Span,
    pub kind: TypeExprKind,
    pub optional: bool, // ?
    pub result: bool, // or error
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExprKind {
    Named(String, Vec<TypeExpr>), // Ident[args]
    Fn(Vec<TypeExpr>, Option<Box<TypeExpr>>), // fn(args) -> ret
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub span: Span,
    pub stmts: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Let(LetStmt),
    Assign(AssignStmt),
    If(IfStmt),
    While(WhileStmt),
    For(ForStmt),
    Match(MatchStmt),
    Return(ReturnStmt),
    Break(Span),
    Continue(Span),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LetStmt {
    pub span: Span,
    pub is_mut: bool,
    pub name: String,
    pub init: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssignStmt {
    pub span: Span,
    pub target: LValue,
    pub op: AssignOp,
    pub rhs: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AssignOp {
    Eq,
    AddEq,
    SubEq,
    MulEq,
    DivEq,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LValue {
    pub span: Span,
    pub name: String,
    pub tail: Vec<LValueTail>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LValueTail {
    Field(String),
    Index(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt {
    pub span: Span,
    pub cond: Expr,
    pub then_block: Block,
    pub elifs: Vec<(Expr, Block)>,
    pub else_block: Option<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt {
    pub span: Span,
    pub cond: Expr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    pub span: Span,
    pub name: String,
    pub iter: Expr,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchStmt {
    pub span: Span,
    pub expr: Expr,
    pub arms: Vec<MatchArm>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub span: Span,
    pub pattern: Pattern,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard(Span),
    Literal(Literal),
    Variant(Span, String, Vec<String>), // name(id1, id2...)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    pub span: Span,
    pub expr: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Call(Box<Expr>, Vec<CallArg>),
    Field(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),
    List(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    Record(String, Vec<(String, Expr)>),
    Closure(Vec<Param>, Option<Box<TypeExpr>>, Block),
    Try(Box<Expr>, bool), // true if `else exit`
    Ident(String),
    Literal(Literal),
    InterpStr(Vec<InterpPart>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallArg {
    Positional(Expr),
    Named(String, Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterpPart {
    Text(String),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Or, And,
    Eq, Neq, Lt, Leq, Gt, Geq,
    Range, RangeInc,
    Add, Sub,
    Mul, Div, FloorDiv, Mod,
    Pow,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Int(String),
    Float(String),
    Str(String),
    Bool(bool),
    None,
}

/// Helper functions to format AST as S-expressions
pub fn dump_file(file: &File) -> String {
    let mut out = String::new();
    out.push_str("(file\n");
    for u in &file.uses {
        out.push_str(&format!("  (use \"{}\")\n", u.path));
    }
    for item in &file.items {
        out.push_str(&dump_top_item(item, 1));
    }
    out.push_str(")\n");
    out
}

fn dump_top_item(item: &TopItem, depth: usize) -> String {
    let pad = "  ".repeat(depth);
    match item {
        TopItem::Fn(f) => format!("{pad}{}\n", dump_fn(f, depth)),
        TopItem::Type(t) => format!("{pad}{}\n", dump_type(t, depth)),
        TopItem::Let(l) => format!("{pad}{}\n", dump_let(l, depth)),
        TopItem::Stmt(s) => format!("{pad}{}\n", dump_stmt(s, depth)),
    }
}

fn dump_fn(f: &FnDecl, depth: usize) -> String {
    let mut s = String::new();
    s.push_str(&format!("(fn {}:{}", f.span.line, f.span.col));
    if let Some(r) = &f.receiver {
        s.push_str(&format!(" {}.{}", r, f.name));
    } else {
        s.push_str(&format!(" {}", f.name));
    }
    s.push_str(" (params");
    for p in &f.params {
        s.push_str(&format!(" {}", p.name));
        if let Some(t) = &p.typ {
            s.push_str(&format!(":{}", dump_type_expr(t)));
        }
    }
    s.push(')');
    if let Some(t) = &f.ret_type {
        s.push_str(&format!(" -> {}", dump_type_expr(t)));
    }
    s.push('\n');
    s.push_str(&dump_block(&f.body, depth + 1));
    s.push_str(&format!("{pad})", pad = "  ".repeat(depth)));
    s
}

fn dump_type(t: &TypeDecl, _depth: usize) -> String {
    let mut s = String::new();
    s.push_str(&format!("(type {}:{} {}", t.span.line, t.span.col, t.name));
    match &t.kind {
        TypeDeclKind::Record(fields) => {
            s.push_str(" (record");
            for f in fields {
                s.push_str(&format!(" ({} {})", f.name, dump_type_expr(&f.typ)));
            }
            s.push(')');
        }
        TypeDeclKind::Enum(variants) => {
            s.push_str(" (enum");
            for v in variants {
                s.push_str(&format!(" ({}", v.name));
                if !v.fields.is_empty() {
                    for f in &v.fields {
                        s.push_str(&format!(" ({} {})", f.name, dump_type_expr(&f.typ)));
                    }
                }
                s.push(')');
            }
            s.push(')');
        }
    }
    s.push(')');
    s
}

fn dump_let(l: &LetStmt, depth: usize) -> String {
    let kw = if l.is_mut { "mut" } else { "let" };
    format!("({kw} {}:{} {} {})", l.span.line, l.span.col, l.name, dump_expr(&l.init, depth))
}

fn dump_stmt(stmt: &Statement, depth: usize) -> String {
    let pad = "  ".repeat(depth);
    match stmt {
        Statement::Let(l) => dump_let(l, depth),
        Statement::Assign(a) => {
            let op = match a.op {
                AssignOp::Eq => "=",
                AssignOp::AddEq => "+=",
                AssignOp::SubEq => "-=",
                AssignOp::MulEq => "*=",
                AssignOp::DivEq => "/=",
            };
            format!("(= {}:{} {} {} {})", a.span.line, a.span.col, dump_lvalue(&a.target), op, dump_expr(&a.rhs, depth))
        }
        Statement::If(i) => {
            let mut s = format!("(if {}:{} {}\n", i.span.line, i.span.col, dump_expr(&i.cond, depth));
            s.push_str(&dump_block(&i.then_block, depth + 1));
            for (cond, block) in &i.elifs {
                s.push_str(&format!("\n{pad}  (elif {}\n{}", dump_expr(cond, depth), dump_block(block, depth + 2)));
                s.push_str(&format!("{pad}  )"));
            }
            if let Some(else_b) = &i.else_block {
                s.push_str(&format!("\n{pad}  (else\n{}", dump_block(else_b, depth + 2)));
                s.push_str(&format!("{pad}  )"));
            }
            s.push_str(&format!("\n{pad})"));
            s
        }
        Statement::While(w) => {
            let mut s = format!("(while {}:{} {}\n", w.span.line, w.span.col, dump_expr(&w.cond, depth));
            s.push_str(&dump_block(&w.body, depth + 1));
            s.push_str(&format!("\n{pad})"));
            s
        }
        Statement::For(f) => {
            let mut s = format!("(for {}:{} {} {}\n", f.span.line, f.span.col, f.name, dump_expr(&f.iter, depth));
            s.push_str(&dump_block(&f.body, depth + 1));
            s.push_str(&format!("\n{pad})"));
            s
        }
        Statement::Match(m) => {
            let mut s = format!("(match {}:{} {}\n", m.span.line, m.span.col, dump_expr(&m.expr, depth));
            for arm in &m.arms {
                s.push_str(&format!("{pad}  (arm {}:{} ", arm.span.line, arm.span.col));
                match &arm.pattern {
                    Pattern::Wildcard(_) => s.push('_'),
                    Pattern::Literal(Literal::Int(x)) => s.push_str(x),
                    Pattern::Literal(Literal::Float(x)) => s.push_str(x),
                    Pattern::Literal(Literal::Str(x)) => s.push_str(&format!("\"{}\"", x)),
                    Pattern::Literal(Literal::Bool(x)) => s.push_str(if *x { "true" } else { "false" }),
                    Pattern::Literal(Literal::None) => s.push_str("none"),
                    Pattern::Variant(_, name, binds) => {
                        s.push_str(&format!("({}", name));
                        for b in binds {
                            s.push_str(&format!(" {}", b));
                        }
                        s.push(')');
                    }
                }
                s.push('\n');
                s.push_str(&dump_block(&arm.body, depth + 2));
                s.push_str(&format!("\n{pad}  )\n"));
            }
            s.push_str(&format!("{pad})"));
            s
        }
        Statement::Return(r) => {
            if let Some(e) = &r.expr {
                format!("(return {}:{} {})", r.span.line, r.span.col, dump_expr(e, depth))
            } else {
                format!("(return {}:{})", r.span.line, r.span.col)
            }
        }
        Statement::Break(span) => format!("(break {}:{})", span.line, span.col),
        Statement::Continue(span) => format!("(continue {}:{})", span.line, span.col),
        Statement::Expr(e) => format!("(expr {})", dump_expr(e, depth)),
    }
}

fn dump_block(b: &Block, depth: usize) -> String {
    let mut s = String::new();
    for stmt in &b.stmts {
        s.push_str(&dump_stmt(stmt, depth));
        s.push('\n');
    }
    s
}

fn dump_type_expr(t: &TypeExpr) -> String {
    let mut s = match &t.kind {
        TypeExprKind::Named(n, args) => {
            if args.is_empty() {
                n.clone()
            } else {
                let a: Vec<String> = args.iter().map(dump_type_expr).collect();
                format!("{}[{}]", n, a.join(", "))
            }
        }
        TypeExprKind::Fn(args, ret) => {
            let a: Vec<String> = args.iter().map(dump_type_expr).collect();
            let mut s = format!("fn({})", a.join(", "));
            if let Some(r) = ret {
                s.push_str(&format!(" -> {}", dump_type_expr(r)));
            }
            s
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

fn dump_lvalue(lv: &LValue) -> String {
    let mut s = lv.name.clone();
    for t in &lv.tail {
        match t {
            LValueTail::Field(f) => s.push_str(&format!(".{}", f)),
            LValueTail::Index(e) => s.push_str(&format!("[{}]", dump_expr(e, 0))),
        }
    }
    s
}

fn dump_expr(e: &Expr, depth: usize) -> String {
    match &e.kind {
        ExprKind::Binary(op, left, right) => {
            let ops = match op {
                BinOp::Or => "or", BinOp::And => "and",
                BinOp::Eq => "==", BinOp::Neq => "!=", BinOp::Lt => "<", BinOp::Leq => "<=", BinOp::Gt => ">", BinOp::Geq => ">=",
                BinOp::Range => "..", BinOp::RangeInc => "..=",
                BinOp::Add => "+", BinOp::Sub => "-",
                BinOp::Mul => "*", BinOp::Div => "/", BinOp::FloorDiv => "//", BinOp::Mod => "%",
                BinOp::Pow => "**",
            };
            format!("({ops} {} {})", dump_expr(left, depth), dump_expr(right, depth))
        }
        ExprKind::Unary(op, inner) => {
            let ops = match op { UnOp::Neg => "-", UnOp::Not => "not" };
            format!("({ops} {})", dump_expr(inner, depth))
        }
        ExprKind::Call(callee, args) => {
            let mut s = format!("(call {}", dump_expr(callee, depth));
            for a in args {
                match a {
                    CallArg::Positional(e) => s.push_str(&format!(" {}", dump_expr(e, depth))),
                    CallArg::Named(n, e) => s.push_str(&format!(" ({} {})", n, dump_expr(e, depth))),
                }
            }
            s.push(')');
            s
        }
        ExprKind::Field(inner, f) => format!("(. {} {})", dump_expr(inner, depth), f),
        ExprKind::Index(inner, idx) => format!("(index {} {})", dump_expr(inner, depth), dump_expr(idx, depth)),
        ExprKind::List(items) => {
            let mut s = String::from("(list");
            for i in items {
                s.push_str(&format!(" {}", dump_expr(i, depth)));
            }
            s.push(')');
            s
        }
        ExprKind::Map(items) => {
            let mut s = String::from("(map");
            for (k, v) in items {
                s.push_str(&format!(" ({} {})", dump_expr(k, depth), dump_expr(v, depth)));
            }
            s.push(')');
            s
        }
        ExprKind::Record(name, fields) => {
            let mut s = format!("(record {}", name);
            for (k, v) in fields {
                s.push_str(&format!(" ({} {})", k, dump_expr(v, depth)));
            }
            s.push(')');
            s
        }
        ExprKind::Closure(params, ret, body) => {
            let mut s = String::from("(closure (");
            for p in params {
                s.push_str(&format!(" {}", p.name));
                if let Some(t) = &p.typ {
                    s.push_str(&format!(":{}", dump_type_expr(t)));
                }
            }
            s.push(')');
            if let Some(t) = ret {
                s.push_str(&format!(" -> {}", dump_type_expr(t)));
            }
            s.push('\n');
            s.push_str(&dump_block(body, depth + 1));
            s.push_str(&format!("{pad})", pad = "  ".repeat(depth)));
            s
        }
        ExprKind::Try(inner, else_exit) => {
            if *else_exit {
                format!("(try {} else exit)", dump_expr(inner, depth))
            } else {
                format!("(try {})", dump_expr(inner, depth))
            }
        }
        ExprKind::Ident(i) => i.clone(),
        ExprKind::Literal(Literal::Int(x)) => x.clone(),
        ExprKind::Literal(Literal::Float(x)) => x.clone(),
        ExprKind::Literal(Literal::Str(x)) => format!("\"{}\"", x),
        ExprKind::Literal(Literal::Bool(x)) => if *x { "true".into() } else { "false".into() },
        ExprKind::Literal(Literal::None) => "none".into(),
        ExprKind::InterpStr(parts) => {
            let mut s = String::from("(str");
            for p in parts {
                match p {
                    InterpPart::Text(t) => s.push_str(&format!(" \"{}\"", t)),
                    InterpPart::Expr(e) => s.push_str(&format!(" {}", dump_expr(e, depth))),
                }
            }
            s.push(')');
            s
        }
    }
}
