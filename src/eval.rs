use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use crate::ast::*;
use crate::diag::Diag;
use crate::val::Val;
use crate::bignum::BigInt;

pub struct Scope {
    parent: Option<Rc<RefCell<Scope>>>,
    vars: HashMap<String, (Val, bool)>, // (value, is_mut)
}

impl Scope {
    pub fn new(parent: Option<Rc<RefCell<Scope>>>) -> Self {
        Self { parent, vars: HashMap::new() }
    }

    pub fn get(&self, name: &str) -> Option<Val> {
        if let Some((v, _)) = self.vars.get(name) {
            Some(v.clone())
        } else if let Some(p) = &self.parent {
            p.borrow().get(name)
        } else {
            None
        }
    }

    pub fn set(&mut self, name: &str, val: Val) -> Result<(), ()> {
        if let Some((v, is_mut)) = self.vars.get_mut(name) {
            if !*is_mut {
                return Err(()); // Cannot reassign immutable let
            }
            *v = val;
            Ok(())
        } else if let Some(p) = &self.parent {
            p.borrow_mut().set(name, val)
        } else {
            Err(()) // Not found
        }
    }

    pub fn define(&mut self, name: String, val: Val, is_mut: bool) {
        self.vars.insert(name, (val, is_mut));
    }
}

pub struct Evaluator {
    pub global: Rc<RefCell<Scope>>,
}

pub enum Flow {
    Return(Val),
    Break(Span),
    Continue(Span),
}

impl Evaluator {
    pub fn new() -> Self {
        Self { global: Rc::new(RefCell::new(Scope::new(None))) }
    }

    pub fn eval_file(&mut self, file: &File) -> Result<(), Diag> {
        for item in &file.items {
            match item {
                TopItem::Stmt(s) => {
                    self.eval_stmt(s, self.global.clone())?;
                }
                TopItem::Let(l) => {
                    self.eval_let(l, self.global.clone())?;
                }
                TopItem::Fn(_) | TopItem::Type(_) => {
                    // Ignored in P3 unless we support top-level fns. Wait, P3 has no functions.
                }
            }
        }
        Ok(())
    }

    fn eval_block(&mut self, block: &Block, parent_env: Rc<RefCell<Scope>>) -> Result<Result<Val, Flow>, Diag> {
        let env = Rc::new(RefCell::new(Scope::new(Some(parent_env))));
        for stmt in &block.stmts {
            if let Some(flow) = self.eval_stmt(stmt, env.clone())? {
                return Ok(Err(flow));
            }
        }
        Ok(Ok(Val::None))
    }

    fn eval_stmt(&mut self, stmt: &Statement, env: Rc<RefCell<Scope>>) -> Result<Option<Flow>, Diag> {
        match stmt {
            Statement::Expr(e) => {
                self.eval_expr(e, env.clone())?;
            }
            Statement::Let(l) => {
                self.eval_let(l, env.clone())?;
            }
            Statement::Assign(a) => {
                let val = self.eval_expr(&a.rhs, env.clone())?;
                let target = &a.target.name;
                if !a.target.tail.is_empty() {
                    return Err(Diag { code: "E0102", msg: "field/index assignment not yet supported".into(), line: a.span.line, col: a.span.col });
                }
                
                let final_val = match a.op {
                    AssignOp::Eq => val,
                    AssignOp::AddEq => {
                        let cur = env.borrow().get(target).ok_or(Diag { code: "E0103", msg: format!("undefined variable '{}'", target), line: a.span.line, col: a.span.col })?;
                        self.eval_binop(BinOp::Add, cur, val, a.span.clone())?
                    }
                    AssignOp::SubEq => {
                        let cur = env.borrow().get(target).ok_or(Diag { code: "E0103", msg: format!("undefined variable '{}'", target), line: a.span.line, col: a.span.col })?;
                        self.eval_binop(BinOp::Sub, cur, val, a.span.clone())?
                    }
                    AssignOp::MulEq => {
                        let cur = env.borrow().get(target).ok_or(Diag { code: "E0103", msg: format!("undefined variable '{}'", target), line: a.span.line, col: a.span.col })?;
                        self.eval_binop(BinOp::Mul, cur, val, a.span.clone())?
                    }
                    AssignOp::DivEq => {
                        let cur = env.borrow().get(target).ok_or(Diag { code: "E0103", msg: format!("undefined variable '{}'", target), line: a.span.line, col: a.span.col })?;
                        self.eval_binop(BinOp::Div, cur, val, a.span.clone())?
                    }
                };

                if env.borrow_mut().set(target, final_val).is_err() {
                    return Err(Diag { code: "E0010", msg: format!("cannot reassign immutable variable '{}'", target), line: a.span.line, col: a.span.col });
                }
            }
            Statement::If(i) => {
                let cond_val = self.eval_expr(&i.cond, env.clone())?;
                let b = self.expect_bool(cond_val, i.cond.span.clone())?;
                if b {
                    if let Err(f) = self.eval_block(&i.then_block, env.clone())? {
                        return Ok(Some(f));
                    }
                } else {
                    let mut matched = false;
                    for (elif_cond, elif_block) in &i.elifs {
                        let elif_val = self.eval_expr(elif_cond, env.clone())?;
                        if self.expect_bool(elif_val, elif_cond.span.clone())? {
                            if let Err(f) = self.eval_block(elif_block, env.clone())? {
                                return Ok(Some(f));
                            }
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        if let Some(else_b) = &i.else_block {
                            if let Err(f) = self.eval_block(else_b, env.clone())? {
                                return Ok(Some(f));
                            }
                        }
                    }
                }
            }
            Statement::While(w) => {
                loop {
                    let cond_val = self.eval_expr(&w.cond, env.clone())?;
                    if !self.expect_bool(cond_val, w.cond.span.clone())? {
                        break;
                    }
                    match self.eval_block(&w.body, env.clone())? {
                        Ok(_) => {}
                        Err(Flow::Break(_)) => break,
                        Err(Flow::Continue(_)) => continue,
                        Err(Flow::Return(v)) => return Ok(Some(Flow::Return(v))),
                    }
                }
            }
            Statement::For(f) => {
                let iter_val = self.eval_expr(&f.iter, env.clone())?;
                match iter_val {
                    Val::Range(start, end, is_inc) => {
                        let mut current = *start;
                        let end = *end;
                        loop {
                            // Check loop bound
                            let cond = if is_inc {
                                current.partial_cmp(&end) != Some(std::cmp::Ordering::Greater)
                            } else {
                                current.partial_cmp(&end) == Some(std::cmp::Ordering::Less)
                            };
                            if !cond { break; }
                            
                            // Bind loop var
                            let loop_env = Rc::new(RefCell::new(Scope::new(Some(env.clone()))));
                            loop_env.borrow_mut().define(f.name.clone(), current.clone(), false);
                            
                            match self.eval_block(&f.body, loop_env)? {
                                Ok(_) => {}
                                Err(Flow::Break(_)) => break,
                                Err(Flow::Continue(_)) => continue,
                                Err(Flow::Return(v)) => return Ok(Some(Flow::Return(v))),
                            }
                            
                            // Increment current
                            let next = self.eval_binop(BinOp::Add, current.clone(), Val::Int(BigInt::from_i64(1)), f.span.clone())?;
                            current = next;
                        }
                    }
                    _ => return Err(Diag { code: "E0104", msg: "not iterable".into(), line: f.iter.span.line, col: f.iter.span.col }),
                }
            }
            Statement::Break(span) => return Ok(Some(Flow::Break(span.clone()))),
            Statement::Continue(span) => return Ok(Some(Flow::Continue(span.clone()))),
            Statement::Return(r) => {
                let v = if let Some(e) = &r.expr {
                    self.eval_expr(e, env.clone())?
                } else {
                    Val::None
                };
                return Ok(Some(Flow::Return(v)));
            }
            _ => unimplemented!(),
        }
        Ok(None)
    }

    fn eval_let(&mut self, l: &LetStmt, env: Rc<RefCell<Scope>>) -> Result<(), Diag> {
        let val = self.eval_expr(&l.init, env.clone())?;
        env.borrow_mut().define(l.name.clone(), val, l.is_mut);
        Ok(())
    }

    fn eval_expr(&mut self, expr: &Expr, env: Rc<RefCell<Scope>>) -> Result<Val, Diag> {
        match &expr.kind {
            ExprKind::Literal(lit) => {
                match lit {
                    Literal::Int(s) => Ok(Val::Int(BigInt::parse(s).unwrap())),
                    Literal::Float(s) => Ok(Val::Float(s.parse().unwrap())),
                    Literal::Bool(b) => Ok(Val::Bool(*b)),
                    Literal::Str(s) => Ok(Val::Str(s.clone())),
                    Literal::None => Ok(Val::None),
                }
            }
            ExprKind::Ident(id) => {
                env.borrow().get(id).ok_or_else(|| Diag { code: "E0103", msg: format!("undefined variable '{}'", id), line: expr.span.line, col: expr.span.col })
            }
            ExprKind::Binary(op, left, right) => {
                let lval = self.eval_expr(left, env.clone())?;
                // Short circuit for logic
                if *op == BinOp::And {
                    if !self.expect_bool(lval.clone(), left.span.clone())? {
                        return Ok(Val::Bool(false));
                    }
                    let rval = self.eval_expr(right, env.clone())?;
                    return Ok(Val::Bool(self.expect_bool(rval, right.span.clone())?));
                }
                if *op == BinOp::Or {
                    if self.expect_bool(lval.clone(), left.span.clone())? {
                        return Ok(Val::Bool(true));
                    }
                    let rval = self.eval_expr(right, env.clone())?;
                    return Ok(Val::Bool(self.expect_bool(rval, right.span.clone())?));
                }

                let rval = self.eval_expr(right, env.clone())?;
                self.eval_binop(op.clone(), lval, rval, expr.span.clone())
            }
            ExprKind::Unary(op, inner) => {
                let val = self.eval_expr(inner, env.clone())?;
                match op {
                    UnOp::Neg => match val {
                        Val::Int(mut i) => { i.sign = !i.sign; Ok(Val::Int(i)) },
                        Val::Float(f) => Ok(Val::Float(-f)),
                        _ => Err(Diag { code: "E0104", msg: "bad type for '-'".into(), line: inner.span.line, col: inner.span.col }),
                    }
                    UnOp::Not => {
                        let b = self.expect_bool(val, inner.span.clone())?;
                        Ok(Val::Bool(!b))
                    }
                }
            }
            ExprKind::Call(callee, args) => {
                // stub for sys.print
                if let ExprKind::Field(inner, f) = &callee.kind {
                    if let ExprKind::Ident(obj) = &inner.kind {
                        if obj == "sys" && f == "print" {
                            let mut outs = Vec::new();
                            for arg in args {
                                if let CallArg::Positional(a) = arg {
                                    outs.push(self.eval_expr(a, env.clone())?.to_string());
                                }
                            }
                            println!("{}", outs.join(" "));
                            return Ok(Val::None);
                        }
                    }
                }
                
                // stub for builtin str()
                if let ExprKind::Ident(obj) = &callee.kind {
                    if obj == "str" && args.len() == 1 {
                        if let CallArg::Positional(a) = &args[0] {
                            let val = self.eval_expr(a, env.clone())?;
                            return Ok(Val::Str(val.to_string()));
                        }
                    }
                }

                Err(Diag { code: "E0105", msg: "calls not yet implemented".into(), line: callee.span.line, col: callee.span.col })
            }
            ExprKind::InterpStr(parts) => {
                let mut out = String::new();
                for part in parts {
                    match part {
                        InterpPart::Text(t) => out.push_str(t),
                        InterpPart::Expr(e) => {
                            let val = self.eval_expr(e, env.clone())?;
                            out.push_str(&val.to_string());
                        }
                    }
                }
                Ok(Val::Str(out))
            }
            _ => Err(Diag { code: "E0106", msg: "expression not yet supported".into(), line: expr.span.line, col: expr.span.col }),
        }
    }

    fn expect_bool(&self, val: Val, span: Span) -> Result<bool, Diag> {
        if let Val::Bool(b) = val {
            Ok(b)
        } else {
            Err(Diag { code: "E0104", msg: "expected bool".into(), line: span.line, col: span.col })
        }
    }

    fn eval_binop(&self, op: BinOp, left: Val, right: Val, span: Span) -> Result<Val, Diag> {
        match op {
            BinOp::Eq => Ok(Val::Bool(left == right)),
            BinOp::Neq => Ok(Val::Bool(left != right)),
            BinOp::Lt => Ok(Val::Bool(left.partial_cmp(&right) == Some(std::cmp::Ordering::Less))),
            BinOp::Leq => Ok(Val::Bool(left.partial_cmp(&right) == Some(std::cmp::Ordering::Less) || left == right)),
            BinOp::Gt => Ok(Val::Bool(left.partial_cmp(&right) == Some(std::cmp::Ordering::Greater))),
            BinOp::Geq => Ok(Val::Bool(left.partial_cmp(&right) == Some(std::cmp::Ordering::Greater) || left == right)),
            
            BinOp::Range => Ok(Val::Range(Box::new(left), Box::new(right), false)),
            BinOp::RangeInc => Ok(Val::Range(Box::new(left), Box::new(right), true)),
            
            BinOp::Add => match (left, right) {
                (Val::Int(a), Val::Int(b)) => Ok(Val::Int(&a + &b)),
                (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a + b)),
                (Val::Str(a), Val::Str(b)) => Ok(Val::Str(a + &b)),
                _ => Err(Diag { code: "E0104", msg: "type mismatch in '+'".into(), line: span.line, col: span.col }),
            }
            BinOp::Sub => match (left, right) {
                (Val::Int(a), Val::Int(b)) => Ok(Val::Int(&a - &b)),
                (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a - b)),
                _ => Err(Diag { code: "E0104", msg: "type mismatch in '-'".into(), line: span.line, col: span.col }),
            }
            BinOp::Mul => match (left, right) {
                (Val::Int(a), Val::Int(b)) => Ok(Val::Int(&a * &b)),
                (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a * b)),
                _ => Err(Diag { code: "E0104", msg: "type mismatch in '*'".into(), line: span.line, col: span.col }),
            }
            BinOp::Div => match (left, right) {
                (Val::Int(a), Val::Int(b)) => {
                    if b.is_zero() {
                        return Err(Diag { code: "E0200", msg: "division by zero".into(), line: span.line, col: span.col });
                    }
                    // division of ints returns float!
                    Ok(Val::Float(a.to_f64() / b.to_f64()))
                }
                (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a / b)),
                _ => Err(Diag { code: "E0104", msg: "type mismatch in '/'".into(), line: span.line, col: span.col }),
            }
            BinOp::FloorDiv => match (left, right) {
                (Val::Int(a), Val::Int(b)) => {
                    if b.is_zero() {
                        return Err(Diag { code: "E0200", msg: "division by zero".into(), line: span.line, col: span.col });
                    }
                    let (q, _) = a.div_mod(&b);
                    Ok(Val::Int(q))
                }
                _ => Err(Diag { code: "E0104", msg: "type mismatch in '//'".into(), line: span.line, col: span.col }),
            }
            BinOp::Mod => match (left, right) {
                (Val::Int(a), Val::Int(b)) => {
                    if b.is_zero() {
                        return Err(Diag { code: "E0200", msg: "division by zero".into(), line: span.line, col: span.col });
                    }
                    let (_, r) = a.div_mod(&b);
                    Ok(Val::Int(r))
                }
                _ => Err(Diag { code: "E0104", msg: "type mismatch in '%'".into(), line: span.line, col: span.col }),
            }
            BinOp::Pow => match (left, right) {
                (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a.powf(b))),
                // Int pow not implemented yet
                _ => Err(Diag { code: "E0104", msg: "type mismatch in '**'".into(), line: span.line, col: span.col }),
            }
            _ => Err(Diag { code: "E0105", msg: "operator not supported".into(), line: span.line, col: span.col }),
        }
    }
}
