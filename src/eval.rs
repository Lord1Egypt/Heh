use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::*;
use crate::bignum::BigInt;
use crate::diag::Diag;
use crate::val::Val;

#[derive(Clone)]
pub struct Scope {
    parent: Option<Rc<RefCell<Scope>>>,
    vars: HashMap<String, (Val, bool)>, // (value, is_mut)
}

impl std::fmt::Debug for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Scope")
    }
}

impl Scope {
    pub fn new(parent: Option<Rc<RefCell<Scope>>>) -> Self {
        Self {
            parent,
            vars: HashMap::new(),
        }
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

    pub fn set(&mut self, name: &str, val: Val) -> Result<(), bool> {
        if let Some((v, is_mut)) = self.vars.get_mut(name) {
            if !*is_mut {
                return Err(false); // Cannot reassign immutable let
            }
            *v = val;
            Ok(())
        } else if let Some(p) = &self.parent {
            p.borrow_mut().set(name, val)
        } else {
            Err(false) // Not found
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
        Self {
            global: Rc::new(RefCell::new(Scope::new(None))),
        }
    }
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator {
    pub fn eval_file(&mut self, file: &File, run_args: Vec<String>) -> Result<(), Diag> {
        let mut deny_fs = false;
        let mut app_args = Vec::new();
        for arg in run_args {
            if arg == "--deny-fs" {
                deny_fs = true;
            } else if !arg.starts_with("--deny-") {
                app_args.push(Val::Str(arg));
            }
        }

        let mut sys_map = std::collections::HashMap::new();
        sys_map.insert("print".into(), Val::BuiltinFn("sys.print"));
        sys_map.insert("args".into(), Val::List(Rc::new(RefCell::new(app_args))));

        let mut fs_map = std::collections::HashMap::new();
        if deny_fs {
            fs_map.insert("read".into(), Val::BuiltinFn("sys.fs.denied"));
            fs_map.insert("write".into(), Val::BuiltinFn("sys.fs.denied"));
        } else {
            fs_map.insert("read".into(), Val::BuiltinFn("sys.fs.read"));
            fs_map.insert("write".into(), Val::BuiltinFn("sys.fs.write"));
        }
        sys_map.insert(
            "fs".into(),
            Val::Record("SysFs".into(), Rc::new(RefCell::new(fs_map))),
        );

        self.global.borrow_mut().define(
            "sys".into(),
            Val::Record("Sys".into(), Rc::new(RefCell::new(sys_map))),
            false,
        );

        // Find main
        for item in &file.items {
            if let TopItem::Fn(f) = item {
                let params = f.params.iter().map(|p| p.name.clone()).collect();
                let func = Val::Fn(params, f.body.clone(), self.global.clone());
                self.global.borrow_mut().define(f.name.clone(), func, false);
            }
        }

        // Check for main
        let main_fn = self.global.borrow().get("main");
        if let Some(Val::Fn(params, body, env)) = main_fn {
            let call_env = Rc::new(RefCell::new(Scope::new(Some(env))));
            // Pass sys if it expects a param
            if !params.is_empty() {
                let sys_val = self.global.borrow().get("sys").unwrap();
                call_env
                    .borrow_mut()
                    .define(params[0].clone(), sys_val, false);
            }
            match self.eval_block(&body, call_env) {
                Ok(Ok(_)) | Ok(Err(Flow::Return(_))) => return Ok(()),
                Ok(Err(_)) => {
                    return Err(Diag {
                        code: "E0107",
                        msg: "invalid flow in main".into(),
                        line: 1,
                        col: 1,
                    })
                }
                Err(d) => {
                    if d.code == "E_TRY_PROPAGATE" {
                        // main returns err, exit with code
                        eprintln!("fault: {}", d.msg);
                        std::process::exit(1);
                    }
                    return Err(d);
                }
            }
        }

        // Script mode
        for item in &file.items {
            match item {
                TopItem::Stmt(s) => match self.eval_stmt(s, self.global.clone())? {
                    Err(Flow::Return(_)) => return Ok(()),
                    Err(_) => {
                        return Err(Diag {
                            code: "E0110",
                            msg: "break/continue outside loop".into(),
                            line: 1,
                            col: 1,
                        })
                    }
                    Ok(_) => {}
                },
                TopItem::Let(l) => {
                    self.eval_let(l, self.global.clone())?;
                }
                TopItem::Fn(_) | TopItem::Type(_) => {}
            }
        }
        Ok(())
    }

    fn eval_block(
        &mut self,
        block: &Block,
        parent_env: Rc<RefCell<Scope>>,
    ) -> Result<Result<Val, Flow>, Diag> {
        let env = Rc::new(RefCell::new(Scope::new(Some(parent_env))));
        let mut last_val = Val::None;
        for stmt in &block.stmts {
            match self.eval_stmt(stmt, env.clone())? {
                Ok(v) => last_val = v,
                Err(flow) => return Ok(Err(flow)),
            }
        }
        Ok(Ok(last_val))
    }

    fn eval_stmt(
        &mut self,
        stmt: &Statement,
        env: Rc<RefCell<Scope>>,
    ) -> Result<Result<Val, Flow>, Diag> {
        match stmt {
            Statement::Expr(e) => {
                let v = self.eval_expr(e, env.clone())?;
                return Ok(Ok(v));
            }
            Statement::Let(l) => {
                self.eval_let(l, env.clone())?;
            }
            Statement::Assign(a) => {
                let val = self.eval_expr(&a.rhs, env.clone())?;
                let target = &a.target.name;
                if !a.target.tail.is_empty() {
                    return Err(Diag {
                        code: "E0102",
                        msg: "field/index assignment not yet supported".into(),
                        line: a.span.line,
                        col: a.span.col,
                    });
                }

                let final_val = match a.op {
                    AssignOp::Eq => val,
                    AssignOp::AddEq => {
                        let cur = env.borrow().get(target).ok_or(Diag {
                            code: "E0103",
                            msg: format!("undefined variable '{}'", target),
                            line: a.span.line,
                            col: a.span.col,
                        })?;
                        self.eval_binop(BinOp::Add, cur, val, a.span.clone())?
                    }
                    AssignOp::SubEq => {
                        let cur = env.borrow().get(target).ok_or(Diag {
                            code: "E0103",
                            msg: format!("undefined variable '{}'", target),
                            line: a.span.line,
                            col: a.span.col,
                        })?;
                        self.eval_binop(BinOp::Sub, cur, val, a.span.clone())?
                    }
                    AssignOp::MulEq => {
                        let cur = env.borrow().get(target).ok_or(Diag {
                            code: "E0103",
                            msg: format!("undefined variable '{}'", target),
                            line: a.span.line,
                            col: a.span.col,
                        })?;
                        self.eval_binop(BinOp::Mul, cur, val, a.span.clone())?
                    }
                    AssignOp::DivEq => {
                        let cur = env.borrow().get(target).ok_or(Diag {
                            code: "E0103",
                            msg: format!("undefined variable '{}'", target),
                            line: a.span.line,
                            col: a.span.col,
                        })?;
                        self.eval_binop(BinOp::Div, cur, val, a.span.clone())?
                    }
                };

                if let Err(is_mut_err) = env.borrow_mut().set(target, final_val) {
                    return Err(Diag {
                        code: if !is_mut_err { "E0103" } else { "E0010" },
                        msg: format!("cannot reassign variable '{}'", target),
                        line: a.span.line,
                        col: a.span.col,
                    });
                }
            }
            Statement::If(i) => {
                let cond_val = self.eval_expr(&i.cond, env.clone())?;
                let b = self.expect_bool(cond_val, i.cond.span.clone())?;
                if b {
                    match self.eval_block(&i.then_block, env.clone())? {
                        Ok(v) => return Ok(Ok(v)),
                        Err(f) => return Ok(Err(f)),
                    }
                } else {
                    for (elif_cond, elif_block) in &i.elifs {
                        let elif_val = self.eval_expr(elif_cond, env.clone())?;
                        if self.expect_bool(elif_val, elif_cond.span.clone())? {
                            match self.eval_block(elif_block, env.clone())? {
                                Ok(v) => return Ok(Ok(v)),
                                Err(f) => return Ok(Err(f)),
                            }
                        }
                    }
                    if let Some(else_b) = &i.else_block {
                        match self.eval_block(else_b, env.clone())? {
                            Ok(v) => return Ok(Ok(v)),
                            Err(f) => return Ok(Err(f)),
                        }
                    }
                }
            }
            Statement::While(w) => loop {
                let cond_val = self.eval_expr(&w.cond, env.clone())?;
                if !self.expect_bool(cond_val, w.cond.span.clone())? {
                    break;
                }
                match self.eval_block(&w.body, env.clone())? {
                    Ok(_) => {}
                    Err(Flow::Break(_)) => break,
                    Err(Flow::Continue(_)) => continue,
                    Err(Flow::Return(v)) => return Ok(Err(Flow::Return(v))),
                }
            },
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
                            if !cond {
                                break;
                            }

                            // Bind loop var
                            let loop_env = Rc::new(RefCell::new(Scope::new(Some(env.clone()))));
                            loop_env
                                .borrow_mut()
                                .define(f.name.clone(), current.clone(), false);

                            match self.eval_block(&f.body, loop_env)? {
                                Ok(_) => {}
                                Err(Flow::Break(_)) => break,
                                Err(Flow::Continue(_)) => continue,
                                Err(Flow::Return(v)) => return Ok(Err(Flow::Return(v))),
                            }

                            // Increment current
                            let next = self.eval_binop(
                                BinOp::Add,
                                current.clone(),
                                Val::Int(BigInt::from_i64(1)),
                                f.span.clone(),
                            )?;
                            current = next;
                        }
                    }
                    Val::List(l) => {
                        let items = l.borrow().clone();
                        for item in items {
                            let loop_env = Rc::new(RefCell::new(Scope::new(Some(env.clone()))));
                            loop_env.borrow_mut().define(f.name.clone(), item, false);
                            match self.eval_block(&f.body, loop_env)? {
                                Ok(_) => {}
                                Err(Flow::Break(_)) => break,
                                Err(Flow::Continue(_)) => continue,
                                Err(Flow::Return(v)) => return Ok(Err(Flow::Return(v))),
                            }
                        }
                    }
                    _ => {
                        return Err(Diag {
                            code: "E0104",
                            msg: "not iterable".into(),
                            line: f.iter.span.line,
                            col: f.iter.span.col,
                        })
                    }
                }
            }
            Statement::Match(m) => {
                let val = self.eval_expr(&m.expr, env.clone())?;
                for arm in &m.arms {
                    if let Some(bindings) = self.match_pattern(&val, &arm.pattern) {
                        let arm_env = Rc::new(RefCell::new(Scope::new(Some(env.clone()))));
                        for (k, v) in bindings {
                            arm_env.borrow_mut().define(k, v, false);
                        }
                        match self.eval_block(&arm.body, arm_env)? {
                            Ok(v) => return Ok(Ok(v)),
                            Err(f) => return Ok(Err(f)),
                        }
                    }
                }
                return Err(Diag {
                    code: "E0020",
                    msg: "non-exhaustive match".into(),
                    line: m.span.line,
                    col: m.span.col,
                });
            }
            Statement::Break(span) => return Ok(Err(Flow::Break(span.clone()))),
            Statement::Continue(span) => return Ok(Err(Flow::Continue(span.clone()))),
            Statement::Return(r) => {
                let v = if let Some(e) = &r.expr {
                    self.eval_expr(e, env.clone())?
                } else {
                    Val::None
                };
                return Ok(Err(Flow::Return(v)));
            }
        }
        Ok(Ok(Val::None))
    }

    fn eval_let(&mut self, l: &LetStmt, env: Rc<RefCell<Scope>>) -> Result<(), Diag> {
        let val = self.eval_expr(&l.init, env.clone())?;
        env.borrow_mut().define(l.name.clone(), val, l.is_mut);
        Ok(())
    }

    fn eval_expr(&mut self, expr: &Expr, env: Rc<RefCell<Scope>>) -> Result<Val, Diag> {
        match &expr.kind {
            ExprKind::Literal(lit) => match lit {
                Literal::Int(s) => Ok(Val::Int(BigInt::parse(s).unwrap())),
                Literal::Float(s) => Ok(Val::Float(s.parse().unwrap())),
                Literal::Bool(b) => Ok(Val::Bool(*b)),
                Literal::Str(s) => Ok(Val::Str(s.clone())),
                Literal::None => Ok(Val::None),
            },
            ExprKind::Ident(id) => {
                if let Some(v) = env.borrow().get(id) {
                    Ok(v)
                } else {
                    Ok(Val::Enum(id.clone(), vec![]))
                }
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
                
                // Float Power: a ** b
                if *op == BinOp::Pow {
                    if let (Val::Float(f1), Val::Float(f2)) = (&lval, &rval) {
                        return Ok(Val::Float(f1.powf(*f2)));
                    }
                }
                self.eval_binop(op.clone(), lval, rval, expr.span.clone())
            }
            ExprKind::Unary(op, inner) => {
                let val = self.eval_expr(inner, env.clone())?;
                match op {
                    UnOp::Neg => match val {
                        Val::Int(mut i) => {
                            i.sign = !i.sign;
                            Ok(Val::Int(i))
                        }
                        Val::Float(f) => Ok(Val::Float(-f)),
                        _ => Err(Diag {
                            code: "E0104",
                            msg: "bad type for '-'".into(),
                            line: inner.span.line,
                            col: inner.span.col,
                        }),
                    },
                    UnOp::Not => {
                        let b = self.expect_bool(val, inner.span.clone())?;
                        Ok(Val::Bool(!b))
                    }
                }
            }
            ExprKind::Call(callee, args) => {
                // stub for builtins
                if let ExprKind::Ident(obj) = &callee.kind {
                    if obj == "str" && args.len() == 1 {
                        if let CallArg::Positional(a) = &args[0] {
                            let val = self.eval_expr(a, env.clone())?;
                            return Ok(Val::Str(val.to_string()));
                        }
                    }
                    if obj == "int_of" && args.len() == 1 {
                        if let CallArg::Positional(a) = &args[0] {
                            let val = self.eval_expr(a, env.clone())?;
                            if let Val::Str(s) = val {
                                if let Some(i) = BigInt::parse(&s) {
                                    return Ok(Val::Ok(Box::new(Val::Int(i))));
                                }
                            }
                            return Ok(Val::Err("invalid int".into()));
                        }
                    }
                    if obj == "ok" && args.len() == 1 {
                        if let CallArg::Positional(a) = &args[0] {
                            let val = self.eval_expr(a, env.clone())?;
                            return Ok(Val::Ok(Box::new(val)));
                        }
                    }
                    if obj == "err" && args.len() == 1 {
                        if let CallArg::Positional(a) = &args[0] {
                            let val = self.eval_expr(a, env.clone())?;
                            if let Val::Str(s) = val {
                                return Ok(Val::Err(s));
                            }
                            return Ok(Val::Err(val.to_string())); // fallback
                        }
                    }
                    if obj == "sqrt" && args.len() == 1 {
                        if let CallArg::Positional(a) = &args[0] {
                            let val = self.eval_expr(a, env.clone())?;
                            if let Val::Float(f) = val {
                                return Ok(Val::Float(f.sqrt()));
                            }
                        }
                    }
                }

                let callee_val = self.eval_expr(callee, env.clone())?;

                let mut arg_vals = Vec::new();
                let mut is_named_args = false;
                for arg in args {
                    if let CallArg::Positional(a) = arg {
                        arg_vals.push(self.eval_expr(a, env.clone())?);
                    } else if let CallArg::Named(_, a) = arg {
                        is_named_args = true;
                        arg_vals.push(self.eval_expr(a, env.clone())?);
                    }
                }

                match callee_val {
                    Val::BoundMethod(obj, method) => {
                        if let Val::Float(f) = *obj {
                            if method == "sqrt" && arg_vals.is_empty() {
                                return Ok(Val::Float(f.sqrt()));
                            }
                        }
                        
                        let mut new_args = vec![*obj];
                        new_args.extend(arg_vals);
                        // Invoke as a static function with self as first arg
                        let func_val = env.borrow().get(&method).ok_or(Diag {
                            code: "E0111",
                            msg: format!("method '{}' not found", method),
                            line: callee.span.line,
                            col: callee.span.col,
                        })?;
                        if let Val::Fn(params, body, closure_env) = func_val {
                            let call_env = Rc::new(RefCell::new(Scope::new(Some(closure_env))));
                            for (p, v) in params.into_iter().zip(new_args) {
                                call_env.borrow_mut().define(p, v, false);
                            }
                            match self.eval_block(&body, call_env) {
                                Ok(Ok(v)) => Ok(v),
                                Ok(Err(Flow::Return(v))) => Ok(v),
                                Ok(Err(_)) => Err(Diag {
                                    code: "E0110",
                                    msg: "break/continue outside loop".into(),
                                    line: callee.span.line,
                                    col: callee.span.col,
                                }),
                                Err(d) => Err(d),
                            }
                        } else {
                            Err(Diag {
                                code: "E0111",
                                msg: "not callable".into(),
                                line: callee.span.line,
                                col: callee.span.col,
                            })
                        }
                    }
                    Val::Enum(name, empty_args) if empty_args.is_empty() => {
                        // It's either an Enum or a Record constructor
                        if is_named_args {
                            let mut map = std::collections::HashMap::new();
                            for arg in args {
                                match arg {
                                    CallArg::Named(k, val_expr) => {
                                        map.insert(
                                            k.clone(),
                                            self.eval_expr(&val_expr, env.clone())?,
                                        );
                                    }
                                    _ => {
                                        return Err(Diag {
                                            code: "E0106",
                                            msg: "positional arg in record literal".into(),
                                            line: callee.span.line,
                                            col: callee.span.col,
                                        })
                                    }
                                }
                            }
                            Ok(Val::Record(name, Rc::new(RefCell::new(map))))
                        } else {
                            Ok(Val::Enum(name, arg_vals))
                        }
                    }
                    Val::Fn(params, body, closure_env) => {
                        if params.len() != arg_vals.len() {
                            return Err(Diag {
                                code: "E0109",
                                msg: format!(
                                    "expected {} args, got {}",
                                    params.len(),
                                    arg_vals.len()
                                ),
                                line: callee.span.line,
                                col: callee.span.col,
                            });
                        }
                        let call_env = Rc::new(RefCell::new(Scope::new(Some(closure_env))));
                        for (p, v) in params.into_iter().zip(arg_vals) {
                            call_env.borrow_mut().define(p, v, false);
                        }
                        match self.eval_block(&body, call_env) {
                            Ok(Ok(v)) => Ok(v),
                            Ok(Err(Flow::Return(v))) => Ok(v),
                            Ok(Err(_)) => Err(Diag {
                                code: "E0110",
                                msg: "break/continue outside loop".into(),
                                line: callee.span.line,
                                col: callee.span.col,
                            }),
                            Err(d) => {
                                if d.code == "E_TRY_PROPAGATE" {
                                    Ok(Val::Err(d.msg))
                                } else {
                                    Err(d)
                                }
                            }
                        }
                    }
                    Val::BuiltinFn(name) => match name {
                        "sys.print" => {
                            let mut outs = Vec::new();
                            for a in arg_vals {
                                outs.push(a.to_string());
                            }
                            println!("{}", outs.join(" "));
                            Ok(Val::None)
                        }
                        "sys.fs.denied" => Ok(Val::Err("capability denied: fs".into())),
                        "sys.fs.read" => {
                            if arg_vals.len() != 1 {
                                return Err(Diag {
                                    code: "E0109",
                                    msg: "sys.fs.read expects 1 arg".into(),
                                    line: callee.span.line,
                                    col: callee.span.col,
                                });
                            }
                            if let Val::Str(s) = &arg_vals[0] {
                                match std::fs::read_to_string(s) {
                                    Ok(content) => Ok(Val::Ok(Box::new(Val::Str(content)))),
                                    Err(e) => Ok(Val::Err(e.to_string())),
                                }
                            } else {
                                Ok(Val::Err("path must be string".into()))
                            }
                        }
                        "sys.fs.write" => {
                            if arg_vals.len() != 2 {
                                return Err(Diag {
                                    code: "E0109",
                                    msg: "sys.fs.write expects 2 args".into(),
                                    line: callee.span.line,
                                    col: callee.span.col,
                                });
                            }
                            if let (Val::Str(path), Val::Str(data)) = (&arg_vals[0], &arg_vals[1]) {
                                match std::fs::write(path, data) {
                                    Ok(_) => Ok(Val::Ok(Box::new(Val::None))),
                                    Err(e) => Ok(Val::Err(e.to_string())),
                                }
                            } else {
                                Ok(Val::Err("path and data must be strings".into()))
                            }
                        }
                        _ => Err(Diag {
                            code: "E0111",
                            msg: format!("unknown builtin '{}'", name),
                            line: callee.span.line,
                            col: callee.span.col,
                        }),
                    },
                    _ => Err(Diag {
                        code: "E0111",
                        msg: "not callable".into(),
                        line: callee.span.line,
                        col: callee.span.col,
                    }),
                }
            }
            ExprKind::Closure(params, _, body) => {
                let p_names = params.iter().map(|p| p.name.clone()).collect();
                Ok(Val::Fn(p_names, body.clone(), env.clone()))
            }
            ExprKind::Try(inner, else_exit) => {
                let val = self.eval_expr(inner, env.clone())?;
                match val {
                    Val::Ok(v) => Ok(*v),
                    Val::Err(e) => {
                        if *else_exit {
                            // exit with diagnostic message
                            eprintln!("fault: {}", e);
                            std::process::exit(1);
                        } else {
                            // In Heh, `try` without `else exit` returns the error from the current function.
                            // We can use a special Diag code, but actually Heh uses `try` to return `err(e)`.
                            // To implement `try` returning `err(e)` from the function, we need a special Flow::Return.
                            // But `eval_expr` doesn't return `Flow`. It returns `Val` or `Diag`.
                            // This means `try` is an expression that can short-circuit the whole function!
                            // Rust `?` equivalent. Wait! We need `Flow::Return` to be propagatable from `eval_expr`.
                            // Let's cheat and return a special Diag that gets caught by `eval_block`?
                            // No, `eval_expr` returning a Diag is a hard error (panic).
                            // Let's implement `try` via a special Diag that `eval_block` handles.
                            Err(Diag {
                                code: "E_TRY_PROPAGATE",
                                msg: e,
                                line: expr.span.line,
                                col: expr.span.col,
                            })
                        }
                    }
                    _ => Err(Diag {
                        code: "E0112",
                        msg: "try on non-result".into(),
                        line: expr.span.line,
                        col: expr.span.col,
                    }),
                }
            }
            ExprKind::Field(inner, f) => {
                let obj = self.eval_expr(inner, env.clone())?;
                match obj {
                    Val::Err(s) => {
                        if f == "msg" {
                            Ok(Val::Str(s))
                        } else {
                            Err(Diag {
                                code: "E0105",
                                msg: format!("no field '{}' on error", f),
                                line: expr.span.line,
                                col: expr.span.col,
                            })
                        }
                    }
                    Val::Record(_, ref r) => {
                        if let Some(v) = r.borrow().get(f) {
                            Ok(v.clone())
                        } else {
                            Ok(Val::BoundMethod(Box::new(obj.clone()), f.clone()))
                        }
                    }
                    Val::Str(ref s) => {
                        if f == "len" {
                            Ok(Val::Int(BigInt::from_i64(s.chars().count() as i64)))
                        } else {
                            Ok(Val::BoundMethod(Box::new(obj.clone()), f.clone()))
                        }
                    }
                    Val::List(ref l) => {
                        if f == "len" {
                            Ok(Val::Int(BigInt::from_i64(l.borrow().len() as i64)))
                        } else {
                            Ok(Val::BoundMethod(Box::new(obj.clone()), f.clone()))
                        }
                    }
                    Val::Map(ref m) => {
                        if f == "len" {
                            Ok(Val::Int(BigInt::from_i64(m.borrow().len() as i64)))
                        } else {
                            Ok(Val::BoundMethod(Box::new(obj.clone()), f.clone()))
                        }
                    }
                    _ => Ok(Val::BoundMethod(Box::new(obj.clone()), f.clone())),
                }
            }
            ExprKind::Index(obj, idx) => {
                let obj_val = self.eval_expr(obj, env.clone())?;
                let idx_val = self.eval_expr(idx, env.clone())?;
                match obj_val {
                    Val::List(l) => {
                        if let Val::Int(i) = idx_val {
                            let b = l.borrow();
                            let idx = i.to_f64() as usize;
                            if idx < b.len() {
                                return Ok(b[idx].clone());
                            }
                        }
                        Err(Diag {
                            code: "E0106",
                            msg: "index out of bounds".into(),
                            line: expr.span.line,
                            col: expr.span.col,
                        })
                    }
                    Val::Map(m) => {
                        if let Some(v) = m.borrow().get(&idx_val) {
                            Ok(v.clone())
                        } else {
                            Err(Diag {
                                code: "E0106",
                                msg: "key not found".into(),
                                line: expr.span.line,
                                col: expr.span.col,
                            })
                        }
                    }
                    _ => Err(Diag {
                        code: "E0106",
                        msg: "not indexable".into(),
                        line: expr.span.line,
                        col: expr.span.col,
                    }),
                }
            }
            ExprKind::Record(name, fields) => {
                let mut map = std::collections::HashMap::new();
                for (k, v) in fields {
                    map.insert(k.clone(), self.eval_expr(v, env.clone())?);
                }
                Ok(Val::Record(name.clone(), Rc::new(RefCell::new(map))))
            }
            ExprKind::Map(entries) => {
                #[allow(clippy::mutable_key_type)]
                let mut map = std::collections::HashMap::new();
                for (k, v) in entries {
                    map.insert(
                        self.eval_expr(k, env.clone())?,
                        self.eval_expr(v, env.clone())?,
                    );
                }
                Ok(Val::Map(Rc::new(RefCell::new(map))))
            }
            ExprKind::List(items) => {
                let mut vals = Vec::new();
                for item in items {
                    vals.push(self.eval_expr(item, env.clone())?);
                }
                Ok(Val::List(Rc::new(RefCell::new(vals))))
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
        }
    }

    fn expect_bool(&self, val: Val, span: Span) -> Result<bool, Diag> {
        if let Val::Bool(b) = val {
            Ok(b)
        } else {
            Err(Diag {
                code: "E0104",
                msg: "expected bool".into(),
                line: span.line,
                col: span.col,
            })
        }
    }

    fn eval_binop(&self, op: BinOp, left: Val, right: Val, span: Span) -> Result<Val, Diag> {
        match op {
            BinOp::Eq => Ok(Val::Bool(left == right)),
            BinOp::Neq => Ok(Val::Bool(left != right)),
            BinOp::Lt => Ok(Val::Bool(
                left.partial_cmp(&right) == Some(std::cmp::Ordering::Less),
            )),
            BinOp::Leq => Ok(Val::Bool(
                left.partial_cmp(&right) == Some(std::cmp::Ordering::Less) || left == right,
            )),
            BinOp::Gt => Ok(Val::Bool(
                left.partial_cmp(&right) == Some(std::cmp::Ordering::Greater),
            )),
            BinOp::Geq => Ok(Val::Bool(
                left.partial_cmp(&right) == Some(std::cmp::Ordering::Greater) || left == right,
            )),

            BinOp::Range => Ok(Val::Range(Box::new(left), Box::new(right), false)),
            BinOp::RangeInc => Ok(Val::Range(Box::new(left), Box::new(right), true)),

            BinOp::Add => match (left, right) {
                (Val::Int(a), Val::Int(b)) => Ok(Val::Int(&a + &b)),
                (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a + b)),
                (Val::Str(a), Val::Str(b)) => Ok(Val::Str(a + &b)),
                _ => Err(Diag {
                    code: "E0104",
                    msg: "type mismatch in '+'".into(),
                    line: span.line,
                    col: span.col,
                }),
            },
            BinOp::Sub => match (left, right) {
                (Val::Int(a), Val::Int(b)) => Ok(Val::Int(&a - &b)),
                (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a - b)),
                _ => Err(Diag {
                    code: "E0104",
                    msg: "type mismatch in '-'".into(),
                    line: span.line,
                    col: span.col,
                }),
            },
            BinOp::Mul => match (left, right) {
                (Val::Int(a), Val::Int(b)) => Ok(Val::Int(&a * &b)),
                (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a * b)),
                _ => Err(Diag {
                    code: "E0104",
                    msg: "type mismatch in '*'".into(),
                    line: span.line,
                    col: span.col,
                }),
            },
            BinOp::Div => match (left, right) {
                (Val::Int(a), Val::Int(b)) => {
                    if b.is_zero() {
                        return Err(Diag {
                            code: "E0200",
                            msg: "division by zero".into(),
                            line: span.line,
                            col: span.col,
                        });
                    }
                    // division of ints returns float!
                    Ok(Val::Float(a.to_f64() / b.to_f64()))
                }
                (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a / b)),
                _ => Err(Diag {
                    code: "E0104",
                    msg: "type mismatch in '/'".into(),
                    line: span.line,
                    col: span.col,
                }),
            },
            BinOp::FloorDiv => match (left, right) {
                (Val::Int(a), Val::Int(b)) => {
                    if b.is_zero() {
                        return Err(Diag {
                            code: "E0200",
                            msg: "division by zero".into(),
                            line: span.line,
                            col: span.col,
                        });
                    }
                    let (q, _) = a.div_mod(&b);
                    Ok(Val::Int(q))
                }
                _ => Err(Diag {
                    code: "E0104",
                    msg: "type mismatch in '//'".into(),
                    line: span.line,
                    col: span.col,
                }),
            },
            BinOp::Mod => match (left, right) {
                (Val::Int(a), Val::Int(b)) => {
                    if b.is_zero() {
                        return Err(Diag {
                            code: "E0200",
                            msg: "division by zero".into(),
                            line: span.line,
                            col: span.col,
                        });
                    }
                    let (_, r) = a.div_mod(&b);
                    Ok(Val::Int(r))
                }
                _ => Err(Diag {
                    code: "E0104",
                    msg: "type mismatch in '%'".into(),
                    line: span.line,
                    col: span.col,
                }),
            },
            BinOp::Pow => match (left, right) {
                (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a.powf(b))),
                // Int pow not implemented yet
                _ => Err(Diag {
                    code: "E0104",
                    msg: "type mismatch in '**'".into(),
                    line: span.line,
                    col: span.col,
                }),
            },
            _ => Err(Diag {
                code: "E0105",
                msg: "operator not supported".into(),
                line: span.line,
                col: span.col,
            }),
        }
    }

    fn match_pattern(&self, val: &Val, pat: &Pattern) -> Option<Vec<(String, Val)>> {
        match pat {
            Pattern::Wildcard(_) => Some(vec![]),
            Pattern::Literal(lit) => {
                let lit_val = match lit {
                    Literal::Int(s) => Val::Int(BigInt::parse(s).unwrap()),
                    Literal::Float(s) => Val::Float(s.parse().unwrap()),
                    Literal::Bool(b) => Val::Bool(*b),
                    Literal::Str(s) => Val::Str(s.clone()),
                    Literal::None => Val::None,
                };
                if val == &lit_val {
                    Some(vec![])
                } else {
                    None
                }
            }
            Pattern::Variant(_, name, binds) => {
                if name == "ok" {
                    if let Val::Ok(inner) = val {
                        if binds.len() == 1 {
                            return Some(vec![(binds[0].clone(), *inner.clone())]);
                        }
                    }
                } else if name == "err" {
                    if let Val::Err(e) = val {
                        if binds.len() == 1 {
                            return Some(vec![(binds[0].clone(), Val::Err(e.clone()))]);
                        }
                    }
                } else {
                    if let Val::Enum(variant, v_binds) = val {
                        if variant == name && binds.len() == v_binds.len() {
                            let mut res = Vec::new();
                            for (b_name, b_val) in binds.iter().zip(v_binds.iter()) {
                                res.push((b_name.clone(), b_val.clone()));
                            }
                            return Some(res);
                        }
                    } else if let Val::Record(variant, r) = val {
                        if variant == name {
                            let r_borrow = r.borrow();
                            if binds.len() == r_borrow.len() {
                                let mut res = Vec::new();
                                // We map bindings by matching the binding name to the field name.
                                // Because in Heh `match circle(r)` binds the field `r` to the variable `r`.
                                let mut ok = true;
                                for b_name in binds {
                                    if let Some(v) = r_borrow.get(b_name) {
                                        res.push((b_name.clone(), v.clone()));
                                    } else {
                                        ok = false;
                                        break;
                                    }
                                }
                                if ok {
                                    return Some(res);
                                }
                            }
                        }
                    }
                }
                None
            }
        }
    }
}
