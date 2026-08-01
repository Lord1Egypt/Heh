//! P11 compiler: lowers the AST to bytecode for the stack VM in `src/vm.rs`.
//!
//! Variables use the same `Scope` chain as the tree-walker (so semantics are
//! byte-identical), while control flow, function calls, and expression
//! evaluation run on the VM. Leaf value operations delegate to the evaluator's
//! shared helpers (see `Evaluator::{eval_binop, apply_callee, field_get, …}`).

use crate::ast::*;

#[derive(Clone, Debug)]
pub enum Const {
    Int(String),
    Float(String),
    Str(String),
    Bool(bool),
    None,
}

#[derive(Clone, Debug)]
pub enum Op {
    PushConst(usize),
    PushNone,
    PushBool(bool),
    Pop,
    Load(String),
    Define(String, bool),
    Assign(String, u32, u32),
    OpAssign(String, BinOp, u32, u32),
    Binop(BinOp, u32, u32),
    Neg(u32, u32),
    Not(u32, u32),
    MakeList(usize),
    MakeMap(usize),
    MakeRecord(String, Vec<String>),
    WrapOk,
    WrapErr,
    WrapSome,
    ConcatStr(usize),
    Field(String, u32, u32),
    Index(u32, u32),
    CallUser(usize, usize, u32, u32),               // fn index, argc
    CallValue(usize, Option<Vec<String>>, u32, u32), // argc, named field names
    Sqrt,
    Try(bool, u32, u32),
    Return,
    Jump(usize),
    JumpIfFalse(usize, u32, u32),
    TestBoolJumpFalse(usize, u32, u32),
    TestBoolJumpTrue(usize, u32, u32),
    ToBool(u32, u32),
    ForStart(u32, u32),
    ForNext(String, usize),
    PopIter,
    MatchArm(Pattern, usize),
    PopScrutinee,
    MatchFail(u32, u32),
}

#[derive(Clone, Debug)]
pub struct Chunk {
    pub name: String,
    pub params: Vec<String>,
    pub ops: Vec<Op>,
    pub consts: Vec<Const>,
}

pub struct Program {
    pub functions: Vec<Chunk>,
    pub main: Option<usize>,
    pub top_level: Chunk,
}

struct Loop {
    continue_addr: usize,
    break_jumps: Vec<usize>,
    is_for: bool,
}

struct Compiler<'a> {
    ops: Vec<Op>,
    consts: Vec<Const>,
    fn_index: &'a std::collections::HashMap<String, usize>,
    loops: Vec<Loop>,
}

impl<'a> Compiler<'a> {
    fn new(fn_index: &'a std::collections::HashMap<String, usize>) -> Self {
        Compiler { ops: Vec::new(), consts: Vec::new(), fn_index, loops: Vec::new() }
    }

    fn emit(&mut self, op: Op) -> usize {
        self.ops.push(op);
        self.ops.len() - 1
    }

    fn konst(&mut self, c: Const) -> usize {
        self.consts.push(c);
        self.consts.len() - 1
    }

    fn here(&self) -> usize {
        self.ops.len()
    }

    // ---- statements ----------------------------------------------------

    fn compile_block_value(&mut self, block: &Block) {
        if block.stmts.is_empty() {
            self.emit(Op::PushNone);
            return;
        }
        let (last, rest) = block.stmts.split_last().unwrap();
        for s in rest {
            self.compile_stmt_discard(s);
        }
        self.compile_stmt_value(last);
    }

    fn compile_block_stmt(&mut self, block: &Block) {
        for s in &block.stmts {
            self.compile_stmt_discard(s);
        }
    }

    fn compile_stmt_value(&mut self, s: &Statement) {
        match s {
            Statement::Expr(e) => self.compile_expr(e),
            Statement::Return(r) => self.compile_return(r),
            Statement::If(i) => self.compile_if(i, true),
            Statement::Match(m) => self.compile_match(m, true),
            _ => {
                self.compile_stmt_discard(s);
                self.emit(Op::PushNone);
            }
        }
    }

    fn compile_stmt_discard(&mut self, s: &Statement) {
        match s {
            Statement::Expr(e) => {
                self.compile_expr(e);
                self.emit(Op::Pop);
            }
            Statement::Let(l) => {
                self.compile_expr(&l.init);
                self.emit(Op::Define(l.name.clone(), l.is_mut));
            }
            Statement::Assign(a) => {
                // Only bare-name targets reach here; `needs_tree_walker` sends
                // field/index targets down the tree-walking path instead.
                debug_assert!(a.target.tail.is_empty());
                self.compile_expr(&a.rhs);
                let (line, col) = (a.span.line, a.span.col);
                match a.op {
                    AssignOp::Eq => { self.emit(Op::Assign(a.target.name.clone(), line, col)); }
                    AssignOp::AddEq => { self.emit(Op::OpAssign(a.target.name.clone(), BinOp::Add, line, col)); }
                    AssignOp::SubEq => { self.emit(Op::OpAssign(a.target.name.clone(), BinOp::Sub, line, col)); }
                    AssignOp::MulEq => { self.emit(Op::OpAssign(a.target.name.clone(), BinOp::Mul, line, col)); }
                    AssignOp::DivEq => { self.emit(Op::OpAssign(a.target.name.clone(), BinOp::Div, line, col)); }
                }
            }
            Statement::If(i) => self.compile_if(i, false),
            Statement::While(w) => self.compile_while(w),
            Statement::For(f) => self.compile_for(f),
            Statement::Match(m) => self.compile_match(m, false),
            Statement::Return(r) => self.compile_return(r),
            Statement::Break(_) => {
                if let Some(lp) = self.loops.last_mut() {
                    if lp.is_for {
                        // break must discard the active iterator before leaving
                    }
                }
                let idx = self.emit(Op::Jump(0));
                self.loops.last_mut().unwrap().break_jumps.push(idx);
            }
            Statement::Continue(_) => {
                let target = self.loops.last().unwrap().continue_addr;
                self.emit(Op::Jump(target));
            }
        }
    }

    fn compile_return(&mut self, r: &ReturnStmt) {
        match &r.expr {
            Some(e) => self.compile_expr(e),
            None => { self.emit(Op::PushNone); }
        }
        self.emit(Op::Return);
    }

    fn compile_if(&mut self, i: &IfStmt, value: bool) {
        // Each condition jumps to the next branch on false.
        let mut end_jumps = Vec::new();
        // then
        self.compile_expr(&i.cond);
        let jf = self.emit(Op::JumpIfFalse(0, i.cond.span.line, i.cond.span.col));
        if value { self.compile_block_value(&i.then_block); } else { self.compile_block_stmt(&i.then_block); }
        end_jumps.push(self.emit(Op::Jump(0)));
        self.patch_jump(jf, self.here());
        // elifs
        for (cond, block) in &i.elifs {
            self.compile_expr(cond);
            let jf = self.emit(Op::JumpIfFalse(0, cond.span.line, cond.span.col));
            if value { self.compile_block_value(block); } else { self.compile_block_stmt(block); }
            end_jumps.push(self.emit(Op::Jump(0)));
            self.patch_jump(jf, self.here());
        }
        // else
        if let Some(else_block) = &i.else_block {
            if value { self.compile_block_value(else_block); } else { self.compile_block_stmt(else_block); }
        } else if value {
            self.emit(Op::PushNone);
        }
        let end = self.here();
        for j in end_jumps {
            self.patch_jump(j, end);
        }
    }

    fn compile_while(&mut self, w: &WhileStmt) {
        let cond_addr = self.here();
        self.compile_expr(&w.cond);
        let exit = self.emit(Op::JumpIfFalse(0, w.cond.span.line, w.cond.span.col));
        self.loops.push(Loop { continue_addr: cond_addr, break_jumps: Vec::new(), is_for: false });
        self.compile_block_stmt(&w.body);
        self.emit(Op::Jump(cond_addr));
        let lp = self.loops.pop().unwrap();
        let end = self.here();
        self.patch_jump(exit, end);
        for j in lp.break_jumps {
            self.patch_jump(j, end);
        }
    }

    fn compile_for(&mut self, f: &ForStmt) {
        self.compile_expr(&f.iter);
        self.emit(Op::ForStart(f.iter.span.line, f.iter.span.col));
        let next_addr = self.here();
        let for_next = self.emit(Op::ForNext(f.name.clone(), 0)); // patched to end-of-iter
        self.loops.push(Loop { continue_addr: next_addr, break_jumps: Vec::new(), is_for: true });
        self.compile_block_stmt(&f.body);
        self.emit(Op::Jump(next_addr));
        let lp = self.loops.pop().unwrap();
        // break lands here: pop the still-active iterator, then continue past the
        // normal exhaustion target.
        let break_target = self.here();
        for j in &lp.break_jumps {
            self.patch_jump(*j, break_target);
        }
        if !lp.break_jumps.is_empty() {
            self.emit(Op::PopIter);
        }
        let end = self.here();
        // ForNext jumps here when the iterator is exhausted (already popped).
        self.patch_for_next(for_next, end);
    }

    fn compile_match(&mut self, m: &MatchStmt, value: bool) {
        self.compile_expr(&m.expr); // scrutinee stays on stack
        let mut end_jumps = Vec::new();
        for arm in &m.arms {
            let test = self.emit(Op::MatchArm(arm.pattern.clone(), 0)); // jump to next arm on no-match
            // matched: scrutinee still on stack under bindings; run body then pop scrutinee
            self.emit(Op::PopScrutinee);
            if value { self.compile_block_value(&arm.body); } else { self.compile_block_stmt(&arm.body); }
            end_jumps.push(self.emit(Op::Jump(0)));
            self.patch_match_arm(test, self.here());
        }
        // no arm matched -> exhaustiveness fault
        self.emit(Op::MatchFail(m.span.line, m.span.col));
        let end = self.here();
        for j in end_jumps {
            self.patch_jump(j, end);
        }
    }

    // ---- expressions ---------------------------------------------------

    fn compile_expr(&mut self, e: &Expr) {
        let (line, col) = (e.span.line, e.span.col);
        match &e.kind {
            ExprKind::Literal(lit) => {
                let c = match lit {
                    Literal::Int(s) => Const::Int(s.clone()),
                    Literal::Float(s) => Const::Float(s.clone()),
                    Literal::Str(s) => Const::Str(s.clone()),
                    Literal::Bool(b) => Const::Bool(*b),
                    Literal::None => Const::None,
                };
                let idx = self.konst(c);
                self.emit(Op::PushConst(idx));
            }
            ExprKind::Ident(id) => { self.emit(Op::Load(id.clone())); }
            ExprKind::Binary(op, l, r) => {
                match op {
                    BinOp::And => {
                        self.compile_expr(l);
                        let jf = self.emit(Op::TestBoolJumpFalse(0, l.span.line, l.span.col));
                        self.compile_expr(r);
                        self.emit(Op::ToBool(r.span.line, r.span.col));
                        let done = self.emit(Op::Jump(0));
                        self.patch_test(jf, self.here());
                        self.emit(Op::PushBool(false));
                        self.patch_jump(done, self.here());
                    }
                    BinOp::Or => {
                        self.compile_expr(l);
                        let jt = self.emit(Op::TestBoolJumpTrue(0, l.span.line, l.span.col));
                        self.compile_expr(r);
                        self.emit(Op::ToBool(r.span.line, r.span.col));
                        let done = self.emit(Op::Jump(0));
                        self.patch_test(jt, self.here());
                        self.emit(Op::PushBool(true));
                        self.patch_jump(done, self.here());
                    }
                    _ => {
                        self.compile_expr(l);
                        self.compile_expr(r);
                        self.emit(Op::Binop(op.clone(), line, col));
                    }
                }
            }
            ExprKind::Unary(op, inner) => {
                self.compile_expr(inner);
                match op {
                    UnOp::Neg => { self.emit(Op::Neg(inner.span.line, inner.span.col)); }
                    UnOp::Not => { self.emit(Op::Not(inner.span.line, inner.span.col)); }
                }
            }
            ExprKind::List(items) => {
                for it in items { self.compile_expr(it); }
                self.emit(Op::MakeList(items.len()));
            }
            ExprKind::Map(pairs) => {
                for (k, v) in pairs {
                    self.compile_expr(k);
                    self.compile_expr(v);
                }
                self.emit(Op::MakeMap(pairs.len()));
            }
            ExprKind::Record(name, fields) => {
                let names: Vec<String> = fields.iter().map(|(n, _)| n.clone()).collect();
                for (_, v) in fields { self.compile_expr(v); }
                self.emit(Op::MakeRecord(name.clone(), names));
            }
            ExprKind::Field(obj, f) => {
                self.compile_expr(obj);
                self.emit(Op::Field(f.clone(), line, col));
            }
            ExprKind::Index(obj, idx) => {
                self.compile_expr(obj);
                self.compile_expr(idx);
                self.emit(Op::Index(line, col));
            }
            ExprKind::Try(inner, else_exit) => {
                self.compile_expr(inner);
                self.emit(Op::Try(*else_exit, line, col));
            }
            ExprKind::InterpStr(parts) => {
                for part in parts {
                    match part {
                        InterpPart::Text(t) => {
                            let idx = self.konst(Const::Str(t.clone()));
                            self.emit(Op::PushConst(idx));
                        }
                        InterpPart::Expr(ex) => self.compile_expr(ex),
                    }
                }
                self.emit(Op::ConcatStr(parts.len()));
            }
            ExprKind::Closure(..) => {
                // Closures are not compiled to bytecode (they only appear as
                // map/filter callbacks, which run via the evaluator). Emitting a
                // Load of a sentinel keeps the compiler total; the VM refuses to
                // compile programs containing closures (see Program::compile).
                self.emit(Op::PushNone);
            }
            ExprKind::Call(callee, args) => self.compile_call(callee, args, line, col),
        }
    }

    fn compile_call(&mut self, callee: &Expr, args: &[CallArg], line: u32, col: u32) {
        // Bare-ident intercepts that the tree-walker special-cases.
        if let ExprKind::Ident(name) = &callee.kind {
            if args.len() == 1 {
                if let CallArg::Positional(a) = &args[0] {
                    match name.as_str() {
                        "ok" => { self.compile_expr(a); self.emit(Op::WrapOk); return; }
                        "err" => { self.compile_expr(a); self.emit(Op::WrapErr); return; }
                        "some" => { self.compile_expr(a); self.emit(Op::WrapSome); return; }
                        "sqrt" => { self.compile_expr(a); self.emit(Op::Sqrt); return; }
                        _ => {}
                    }
                }
            }
            if let Some(&idx) = self.fn_index.get(name) {
                for arg in args {
                    match arg {
                        CallArg::Positional(a) | CallArg::Named(_, a) => self.compile_expr(a),
                    }
                }
                self.emit(Op::CallUser(idx, args.len(), line, col));
                return;
            }
        }
        // General path: evaluate callee then args, then dispatch on the value.
        self.compile_expr(callee);
        let mut named: Option<Vec<String>> = None;
        for arg in args {
            match arg {
                CallArg::Positional(a) => self.compile_expr(a),
                CallArg::Named(n, a) => {
                    named.get_or_insert_with(Vec::new).push(n.clone());
                    self.compile_expr(a);
                }
            }
        }
        self.emit(Op::CallValue(args.len(), named, line, col));
    }

    // ---- backpatching --------------------------------------------------

    fn patch_jump(&mut self, at: usize, target: usize) {
        match &mut self.ops[at] {
            Op::Jump(t) | Op::JumpIfFalse(t, _, _) => *t = target,
            _ => panic!("patch_jump on non-jump"),
        }
    }
    fn patch_test(&mut self, at: usize, target: usize) {
        match &mut self.ops[at] {
            Op::TestBoolJumpFalse(t, _, _) | Op::TestBoolJumpTrue(t, _, _) => *t = target,
            _ => panic!("patch_test on non-test"),
        }
    }
    fn patch_for_next(&mut self, at: usize, target: usize) {
        if let Op::ForNext(_, t) = &mut self.ops[at] { *t = target; } else { panic!("patch_for_next"); }
    }
    fn patch_match_arm(&mut self, at: usize, target: usize) {
        if let Op::MatchArm(_, t) = &mut self.ops[at] { *t = target; } else { panic!("patch_match_arm"); }
    }

    fn finish(mut self, name: String, params: Vec<String>) -> Chunk {
        self.emit(Op::Return);
        Chunk { name, params, ops: self.ops, consts: self.consts }
    }
}

/// Does this program use anything the bytecode VM cannot reproduce exactly?
/// Two constructs qualify: closures (no compilation path) and optional
/// narrowing (`if x != none`), whose unwrap is scoped to the then-branch while
/// VM chunks share one flat scope. `heh run --vm` falls back to the
/// tree-walker for such programs so output stays byte-identical either way.
pub fn needs_tree_walker(file: &File) -> bool {
    fn expr_has(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Closure(..) => true,
            ExprKind::Binary(_, a, b) => expr_has(a) || expr_has(b),
            ExprKind::Unary(_, a) => expr_has(a),
            ExprKind::Call(c, args) => expr_has(c) || args.iter().any(|a| match a { CallArg::Positional(e) | CallArg::Named(_, e) => expr_has(e) }),
            ExprKind::Field(a, _) => expr_has(a),
            ExprKind::Index(a, b) => expr_has(a) || expr_has(b),
            ExprKind::List(xs) => xs.iter().any(expr_has),
            ExprKind::Map(ps) => ps.iter().any(|(k, v)| expr_has(k) || expr_has(v)),
            ExprKind::Record(_, fs) => fs.iter().any(|(_, v)| expr_has(v)),
            ExprKind::Try(a, _) => expr_has(a),
            ExprKind::InterpStr(parts) => parts.iter().any(|p| matches!(p, InterpPart::Expr(e) if expr_has(e))),
            _ => false,
        }
    }
    fn block_has(b: &Block) -> bool { b.stmts.iter().any(stmt_has) }
    fn stmt_has(s: &Statement) -> bool {
        match s {
            Statement::Expr(e) => expr_has(e),
            Statement::Let(l) => expr_has(&l.init),
            // A field/index target (`p.x = v`, `l[i] = v`) has no VM encoding;
            // the compiler below only ever assigns to a bare name.
            Statement::Assign(a) => !a.target.tail.is_empty() || expr_has(&a.rhs),
            // `if x != none` narrows only inside the then-branch.
            Statement::If(i) => matches!(crate::check::none_comparison(&i.cond), Some((_, true)))
                || expr_has(&i.cond) || block_has(&i.then_block) || i.elifs.iter().any(|(c, b)| expr_has(c) || block_has(b)) || i.else_block.as_ref().is_some_and(block_has),
            Statement::While(w) => expr_has(&w.cond) || block_has(&w.body),
            Statement::For(f) => expr_has(&f.iter) || block_has(&f.body),
            Statement::Match(m) => expr_has(&m.expr) || m.arms.iter().any(|a| block_has(&a.body)),
            Statement::Return(r) => r.expr.as_ref().is_some_and(expr_has),
            _ => false,
        }
    }
    file.items.iter().any(|item| match item {
        TopItem::Fn(f) => block_has(&f.body),
        TopItem::Stmt(s) => stmt_has(s),
        TopItem::Let(l) => expr_has(&l.init),
        TopItem::Type(_) => false,
    })
}

/// Compile a whole file to a `Program`.
pub fn compile(file: &File) -> Program {
    // Assign an index to every top-level function first, so calls can be
    // resolved to direct `CallUser` ops (supports forward + mutual recursion).
    let mut fn_index = std::collections::HashMap::new();
    let mut fn_decls = Vec::new();
    for item in &file.items {
        if let TopItem::Fn(f) = item {
            fn_index.insert(f.name.clone(), fn_decls.len());
            fn_decls.push(f);
        }
    }

    let mut functions = Vec::with_capacity(fn_decls.len());
    for f in &fn_decls {
        let mut c = Compiler::new(&fn_index);
        c.compile_block_value(&f.body);
        let params: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
        functions.push(c.finish(f.name.clone(), params));
    }

    let main = fn_index.get("main").copied();

    // Top-level `let` constants run in both modes (SPEC §11 allows them
    // alongside `fn main`); bare statements are script mode only.
    let mut top = Compiler::new(&fn_index);
    for item in &file.items {
        match item {
            TopItem::Let(l) => {
                top.compile_expr(&l.init);
                top.emit(Op::Define(l.name.clone(), l.is_mut));
            }
            TopItem::Stmt(s) if main.is_none() => top.compile_stmt_discard(s),
            _ => {}
        }
    }
    let top_level = top.finish("<top>".into(), Vec::new());

    Program { functions, main, top_level }
}
