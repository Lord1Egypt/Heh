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
    /// Directory used to resolve relative `use "./file.heh"` imports.
    pub base_dir: std::path::PathBuf,
    /// Canonical paths currently being loaded, for cycle detection (E0030).
    loading: Vec<std::path::PathBuf>,
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
            base_dir: std::path::PathBuf::from("."),
            loading: Vec::new(),
        }
    }

    pub fn with_base_dir(base_dir: std::path::PathBuf) -> Self {
        Self {
            global: Rc::new(RefCell::new(Scope::new(None))),
            base_dir,
            loading: Vec::new(),
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
        self.prepare(file, run_args)?;
        self.run_prepared(file)
    }

    /// Set up the global scope (sys capabilities, std modules, builtins, and
    /// top-level function definitions) without executing `main`/top-level.
    /// Shared by `eval_file` and the bytecode VM (`heh run --vm`).
    pub fn prepare(&mut self, file: &File, run_args: Vec<String>) -> Result<(), Diag> {
        let mut deny_fs = false;
        let mut deny_net = false;
        let mut deny_env = false;
        let mut deny_clock = false;
        let mut deny_rand = false;
        let mut app_args = Vec::new();
        for arg in run_args {
            match arg.as_str() {
                "--deny-fs" => deny_fs = true,
                "--deny-net" => deny_net = true,
                "--deny-env" => deny_env = true,
                "--deny-clock" => deny_clock = true,
                "--deny-rand" => deny_rand = true,
                _ => {
                    if !arg.starts_with("--deny-") {
                        app_args.push(Val::Str(arg));
                    }
                }
            }
        }

        let mut sys_map = std::collections::HashMap::new();
        sys_map.insert("print".into(), Val::BuiltinFn("sys.print"));
        sys_map.insert("input".into(), Val::BuiltinFn("sys.input"));
        sys_map.insert("args".into(), Val::List(Rc::new(RefCell::new(app_args))));

        let mut fs_map = std::collections::HashMap::new();
        for func in ["read", "read_bytes", "write", "append", "exists", "list_dir", "remove"] {
            if deny_fs {
                fs_map.insert(func.into(), Val::BuiltinFn("sys.fs.denied"));
            } else {
                fs_map.insert(func.into(), Val::BuiltinFn(Box::leak(format!("sys.fs.{}", func).into_boxed_str())));
            }
        }
        sys_map.insert("fs".into(), Val::Record("SysFs".into(), Rc::new(RefCell::new(fs_map))));

        let mut env_map = std::collections::HashMap::new();
        for func in ["get", "set"] {
            if deny_env {
                env_map.insert(func.into(), Val::BuiltinFn("sys.env.denied"));
            } else {
                env_map.insert(func.into(), Val::BuiltinFn(Box::leak(format!("sys.env.{}", func).into_boxed_str())));
            }
        }
        sys_map.insert("env".into(), Val::Record("SysEnv".into(), Rc::new(RefCell::new(env_map))));

        let mut clock_map = std::collections::HashMap::new();
        for func in ["now", "sleep"] {
            if deny_clock {
                clock_map.insert(func.into(), Val::BuiltinFn("sys.clock.denied"));
            } else {
                clock_map.insert(func.into(), Val::BuiltinFn(Box::leak(format!("sys.clock.{}", func).into_boxed_str())));
            }
        }
        sys_map.insert("clock".into(), Val::Record("SysClock".into(), Rc::new(RefCell::new(clock_map))));

        let mut rand_map = std::collections::HashMap::new();
        for func in ["bytes", "int", "float"] {
            if deny_rand {
                rand_map.insert(func.into(), Val::BuiltinFn("sys.rand.denied"));
            } else {
                rand_map.insert(func.into(), Val::BuiltinFn(Box::leak(format!("sys.rand.{}", func).into_boxed_str())));
            }
        }
        sys_map.insert("rand".into(), Val::Record("SysRand".into(), Rc::new(RefCell::new(rand_map))));

        let mut net_map = std::collections::HashMap::new();
        net_map.insert(
            "get".into(),
            Val::BuiltinFn(if deny_net { "sys.net.denied" } else { "sys.net.get" }),
        );
        sys_map.insert("net".into(), Val::Record("SysNet".into(), Rc::new(RefCell::new(net_map))));

        self.global.borrow_mut().define(
            "sys".into(),
            Val::Record("Sys".into(), Rc::new(RefCell::new(sys_map))),
            false,
        );

        // Resolve `use` declarations, register builtins, and define top-level
        // functions (shared with imported modules).
        self.install_defs(file)?;
        Ok(())
    }

    /// Execute a file previously set up with `prepare`: run `main` if present,
    /// otherwise the top-level statements (script mode).
    /// `try` that escapes the top level is a program error, not a propagation.
    fn run_prepared(&mut self, file: &File) -> Result<(), Diag> {
        // Check for main
        let main_fn = self.global.borrow().get("main");

        // SPEC §11 allows top-level `let` constants in a file that also defines
        // `fn main`; bind them before main runs. Script mode instead evaluates
        // them below, interleaved with statements in source order.
        if main_fn.is_some() {
            for item in &file.items {
                if let TopItem::Let(l) = item {
                    self.eval_let(l, self.global.clone()).map_err(rewrite_top_level_try)?;
                }
            }
        }

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
                    if d.code == "E_TRY_PROPAGATE" || d.code == "E_TRY_PROPAGATE_NONE" {
                        return Err(Diag {
                            code: "E0114",
                            msg: "try propagated outside result-returning function".into(),
                            line: 1,
                            col: 1,
                        });
                    }
                    return Err(d);
                }
            }
        }

        // Script mode
        for item in &file.items {
            match item {
                TopItem::Stmt(s) => match self.eval_stmt(s, self.global.clone()) {
                    Ok(Err(Flow::Return(_))) => return Ok(()),
                    Ok(Err(_)) => {
                        return Err(Diag {
                            code: "E0110",
                            msg: "break/continue outside loop".into(),
                            line: 1,
                            col: 1,
                        })
                    }
                    Ok(Ok(_)) => {}
                    Err(d) => {
                        if d.code == "E_TRY_PROPAGATE" || d.code == "E_TRY_PROPAGATE_NONE" {
                            return Err(Diag {
                                code: "E0114",
                                msg: "try propagated outside result-returning function".into(),
                                line: 1,
                                col: 1,
                            });
                        }
                        return Err(d);
                    }
                },
                TopItem::Let(l) => {
                    self.eval_let(l, self.global.clone()).map_err(rewrite_top_level_try)?;
                }
                TopItem::Fn(_) | TopItem::Type(_) => {}
            }
        }
        Ok(())
    }

    /// Install a file's `use`s, builtins, and top-level functions without any
    /// `sys` binding (used by `heh test`: test functions must be pure).
    pub fn load_defs(&mut self, file: &File) -> Result<(), Diag> {
        self.install_defs(file)
    }

    /// Call a zero-argument function by name and return its value. A fault
    /// (e.g. a failed `debug.assert`) surfaces as `Err`. Used by `heh test`.
    pub fn call_zero_arg_fn(&mut self, name: &str) -> Result<Val, Diag> {
        let f = self.global.borrow().get(name);
        match f {
            Some(Val::Fn(params, body, env)) => {
                if !params.is_empty() {
                    return Err(Diag {
                        code: "E0109",
                        msg: format!("test function '{}' must take no arguments", name),
                        line: 1,
                        col: 1,
                    });
                }
                let call_env = Rc::new(RefCell::new(Scope::new(Some(env))));
                match self.eval_block(&body, call_env) {
                    Ok(Ok(v)) | Ok(Err(Flow::Return(v))) => Ok(v),
                    Ok(Err(_)) => Err(Diag {
                        code: "E0110",
                        msg: "break/continue outside loop".into(),
                        line: 1,
                        col: 1,
                    }),
                    Err(d) => Err(d),
                }
            }
            _ => Err(Diag {
                code: "E0011",
                msg: format!("no function named '{}'", name),
                line: 1,
                col: 1,
            }),
        }
    }

    /// Resolve `use` declarations, register builtin methods, and define the
    /// file's top-level functions in the current global scope. Shared by
    /// `eval_file` and imported modules (imported modules never see `sys`).
    fn install_defs(&mut self, file: &File) -> Result<(), Diag> {
        for u in &file.uses {
            let bare = u.path.rsplit('/').next().unwrap_or(&u.path).to_string();
            if let Some(record) = crate::modules::module_record(&u.path) {
                self.global.borrow_mut().define(bare, record, false);
            } else if is_local_import(&u.path) {
                let record = self.load_module(&u.path, u.span.clone())?;
                let name = module_bind_name(&u.path);
                self.global.borrow_mut().define(name, record, false);
            } else {
                return Err(Diag {
                    code: "E0031",
                    msg: format!("unknown module '{}'", u.path),
                    line: u.span.line,
                    col: u.span.col,
                });
            }
        }

        let builtins = [
            "len", "upper", "lower", "trim", "split", "replace", "contains", "starts_with", "chars",
            "push", "pop", "get", "sort", "map", "filter", "join", "set", "remove", "keys", "values",
            "int_of", "str", "int", "float", "list",
        ];
        for b in builtins {
            self.global.borrow_mut().define(b.into(), Val::BuiltinFn(b), false);
        }

        for item in &file.items {
            if let TopItem::Fn(f) = item {
                let params = f.params.iter().map(|p| p.name.clone()).collect();
                let func = Val::Fn(params, f.body.clone(), self.global.clone());
                self.global.borrow_mut().define(f.name.clone(), func, false);
            }
        }
        Ok(())
    }

    /// Load a local `use "./file.heh"` (or `vendor/name`) import: parse, check,
    /// evaluate its definitions in an isolated scope, and return a namespace
    /// record of its exported functions. Detects import cycles (E0030).
    fn load_module(&mut self, path: &str, span: Span) -> Result<Val, Diag> {
        let resolved = self.resolve_import_path(path);
        let canonical = resolved.canonicalize().map_err(|_| Diag {
            code: "E0032",
            msg: format!("cannot find imported file '{}'", path),
            line: span.line,
            col: span.col,
        })?;

        if self.loading.contains(&canonical) {
            return Err(Diag {
                code: "E0030",
                msg: format!("import cycle through '{}'", path),
                line: span.line,
                col: span.col,
            });
        }

        let source = std::fs::read_to_string(&canonical).map_err(|e| Diag {
            code: "E0032",
            msg: format!("cannot read '{}': {}", path, e),
            line: span.line,
            col: span.col,
        })?;

        let tokens = crate::lexer::lex(&source).map_err(|mut d| { d.line = span.line; d.col = span.col; d })?;
        let mut parser = crate::parser::Parser::new(&tokens);
        let module_file = parser.parse_file().map_err(|mut d| { d.line = span.line; d.col = span.col; d })?;

        let mut checker = crate::check::Checker::new();
        checker.check_file(&module_file);
        if let Some(d) = checker.diags.into_iter().next() {
            return Err(Diag {
                code: "E0033",
                msg: format!("in imported '{}': {}", path, d.msg),
                line: span.line,
                col: span.col,
            });
        }

        let module_dir = canonical.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from("."));
        let mut sub = Evaluator::with_base_dir(module_dir);
        sub.loading = self.loading.clone();
        sub.loading.push(canonical.clone());
        sub.install_defs(&module_file)?;

        // Harvest exported functions into a namespace record.
        let mut exports = HashMap::new();
        for item in &module_file.items {
            if let TopItem::Fn(f) = item {
                if let Some(v) = sub.global.borrow().get(&f.name) {
                    exports.insert(f.name.clone(), v);
                }
            }
        }
        let name = module_bind_name(path);
        Ok(Val::Record(name, Rc::new(RefCell::new(exports))))
    }

    fn resolve_import_path(&self, path: &str) -> std::path::PathBuf {
        // Unquoted `use vendor/name` paths omit the extension; add it.
        let with_ext = if path.ends_with(".heh") {
            path.to_string()
        } else {
            format!("{path}.heh")
        };
        let p = std::path::PathBuf::from(&with_ext);
        if p.is_absolute() {
            p
        } else {
            self.base_dir.join(p)
        }
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

    /// `p.x = v`, `l[i] = v`, and any nesting of the two (SPEC §14 `lvalue`).
    /// Records, lists, and maps are reference values, so this walks to the
    /// container that owns the last step and mutates it in place — the `let` /
    /// `mut` distinction governs rebinding the name, not deep mutation (§5.4).
    fn assign_into_tail(
        &mut self,
        a: &AssignStmt,
        rhs: Val,
        env: Rc<RefCell<Scope>>,
    ) -> Result<Val, Diag> {
        let err = |msg: String| Diag { code: "E0102", msg, line: a.span.line, col: a.span.col };

        let mut container = env.borrow().get(&a.target.name).ok_or_else(|| Diag {
            code: "E0103",
            msg: format!("undefined variable '{}'", a.target.name),
            line: a.span.line,
            col: a.span.col,
        })?;

        // Walk every step but the last; those are plain reads.
        let (last, path) = a.target.tail.split_last().expect("caller checked tail is non-empty");
        for step in path {
            container = self.read_step(container, step, env.clone(), a)?;
        }

        // Compound assignment needs the old value first.
        let val = match a.op {
            AssignOp::Eq => rhs,
            _ => {
                let cur = self.read_step(container.clone(), last, env.clone(), a)?;
                let op = match a.op {
                    AssignOp::AddEq => BinOp::Add,
                    AssignOp::SubEq => BinOp::Sub,
                    AssignOp::MulEq => BinOp::Mul,
                    AssignOp::DivEq => BinOp::Div,
                    AssignOp::Eq => unreachable!(),
                };
                self.eval_binop(op, cur, rhs, a.span.clone())?
            }
        };

        match (container, last) {
            (Val::Record(name, fields), LValueTail::Field(f)) => {
                if !fields.borrow().contains_key(f) {
                    return Err(err(format!("record '{}' has no field '{}'", name, f)));
                }
                fields.borrow_mut().insert(f.clone(), val);
            }
            (Val::List(items), LValueTail::Index(idx)) => {
                let idx_val = self.eval_expr(idx, env)?;
                let Val::Int(i) = idx_val else {
                    return Err(err("list index must be an int".into()));
                };
                let mut items = items.borrow_mut();
                match i.to_usize() {
                    Some(n) if n < items.len() => items[n] = val,
                    _ => {
                        return Err(Diag {
                            code: "E0106",
                            msg: "index out of bounds".into(),
                            line: a.span.line,
                            col: a.span.col,
                        })
                    }
                }
            }
            (Val::Map(m), LValueTail::Index(key)) => {
                let key_val = self.eval_expr(key, env)?;
                m.borrow_mut().insert(key_val, val);
            }
            (other, LValueTail::Field(f)) => {
                return Err(err(format!("cannot set field '{}' on {}", f, other.type_name())))
            }
            (other, LValueTail::Index(_)) => {
                return Err(err(format!("cannot index-assign into {}", other.type_name())))
            }
        }
        Ok(Val::None)
    }

    /// Read one lvalue step (`.field` or `[i]`) while walking to the target.
    fn read_step(
        &mut self,
        container: Val,
        step: &LValueTail,
        env: Rc<RefCell<Scope>>,
        a: &AssignStmt,
    ) -> Result<Val, Diag> {
        let err = |msg: String| Diag { code: "E0102", msg, line: a.span.line, col: a.span.col };
        match (container, step) {
            (Val::Record(name, fields), LValueTail::Field(f)) => fields
                .borrow()
                .get(f)
                .cloned()
                .ok_or_else(|| err(format!("record '{}' has no field '{}'", name, f))),
            (Val::List(items), LValueTail::Index(idx)) => {
                let idx_val = self.eval_expr(idx, env)?;
                let Val::Int(i) = idx_val else {
                    return Err(err("list index must be an int".into()));
                };
                let items = items.borrow();
                match i.to_usize() {
                    Some(n) if n < items.len() => Ok(items[n].clone()),
                    _ => Err(Diag {
                        code: "E0106",
                        msg: "index out of bounds".into(),
                        line: a.span.line,
                        col: a.span.col,
                    }),
                }
            }
            (Val::Map(m), LValueTail::Index(key)) => {
                let key_val = self.eval_expr(key, env)?;
                m.borrow()
                    .get(&key_val)
                    .cloned()
                    .ok_or_else(|| err(format!("key not found: {}", key_val)))
            }
            (other, LValueTail::Field(f)) => {
                Err(err(format!("cannot read field '{}' from {}", f, other.type_name())))
            }
            (other, LValueTail::Index(_)) => {
                Err(err(format!("cannot index into {}", other.type_name())))
            }
        }
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
                    return self.assign_into_tail(a, val, env).map(Ok);
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

                // Optional narrowing (SPEC §6.4) is a runtime unwrap as well as
                // a type-level one: the checker gives the binding type `T` here,
                // so the value must be the `T`, not a `some(T)` wrapper.
                //   `if x != none` → narrow inside the then-branch
                //   `if x == none` (false) → narrow for everything after
                let narrow = crate::check::none_comparison(&i.cond)
                    .filter(|&(_, is_neq)| is_neq == b)
                    .and_then(|(name, _)| match env.borrow().get(name) {
                        Some(Val::Some(inner)) => Some((name.to_string(), *inner)),
                        _ => None,
                    });

                if b {
                    let then_env = match narrow {
                        Some((name, inner)) => {
                            let scoped = Rc::new(RefCell::new(Scope::new(Some(env.clone()))));
                            scoped.borrow_mut().define(name, inner, false);
                            scoped
                        }
                        None => env.clone(),
                    };
                    match self.eval_block(&i.then_block, then_env)? {
                        Ok(v) => return Ok(Ok(v)),
                        Err(f) => return Ok(Err(f)),
                    }
                } else {
                    if let Some((name, inner)) = narrow {
                        env.borrow_mut().define(name, inner, false);
                    }
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
                        // An unbounded range (`0..`) is encoded with a `none`
                        // upper bound and iterates forever (until `break`).
                        let unbounded = matches!(end, Val::None);
                        loop {
                            // Check loop bound
                            let cond = if unbounded {
                                true
                            } else if is_inc {
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
                    // Lists, maps (by key), and strs (by char) all iterate over
                    // a snapshot, so mutating the collection inside the loop
                    // cannot invalidate the iteration (SPEC §6.3).
                    other => {
                        let items = match other {
                            Val::List(l) => l.borrow().clone(),
                            Val::Map(m) => m.borrow().keys().cloned().collect(),
                            Val::Str(s) => s.chars().map(|c| Val::Str(c.to_string())).collect(),
                            _ => {
                                return Err(Diag {
                                    code: "E0104",
                                    msg: "not iterable (expected a list, map, str, or range)".into(),
                                    line: f.iter.span.line,
                                    col: f.iter.span.col,
                                })
                            }
                        };
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
                    if obj == "some" && args.len() == 1 {
                        if let CallArg::Positional(a) = &args[0] {
                            let val = self.eval_expr(a, env.clone())?;
                            return Ok(Val::Some(Box::new(val)));
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
                        } else if let Val::BuiltinFn(name) = func_val {
                            self.call_builtin(name, new_args, callee)
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
                    Val::BuiltinFn(name) => self.call_builtin(name, arg_vals, callee),
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
                    Val::Some(v) => Ok(*v),
                    Val::None => {
                        if *else_exit {
                            eprintln!("fault: none");
                            std::process::exit(1);
                        } else {
                            Err(Diag {
                                code: "E_TRY_PROPAGATE_NONE",
                                msg: "none".into(),
                                line: expr.span.line,
                                col: expr.span.col,
                            })
                        }
                    }
                    Val::Err(e) => {
                        if *else_exit {
                            eprintln!("fault: {}", e);
                            std::process::exit(1);
                        } else {
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
                    Val::Str(_) | Val::List(_) | Val::Map(_) => {
                        Ok(Val::BoundMethod(Box::new(obj.clone()), f.clone()))
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
                let mut map = crate::val::OrderedMap::new();
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

    pub fn expect_bool(&self, val: Val, span: Span) -> Result<bool, Diag> {
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

    pub fn eval_binop(&self, op: BinOp, left: Val, right: Val, span: Span) -> Result<Val, Diag> {
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
                    let (q, _) = a.div_mod_floor(&b);
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
                    let (_, r) = a.div_mod_floor(&b);
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
                (Val::Int(a), Val::Int(b)) => {
                    // A negative exponent has no integer answer; the spec keeps
                    // `int` closed under `**`, so this is a fault, not a float.
                    if b < crate::bignum::BigInt::zero() {
                        return Err(Diag {
                            code: "E0201",
                            msg: "negative exponent in '**' (use floats for fractional powers)".into(),
                            line: span.line,
                            col: span.col,
                        });
                    }
                    Ok(Val::Int(a.pow(&b)))
                }
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

    fn call_builtin(&mut self, name: &str, arg_vals: Vec<Val>, callee: &Expr) -> Result<Val, Diag> {
        match name {
            "sys.print" => {
                let mut outs = Vec::new();
                for a in arg_vals {
                    outs.push(a.to_string());
                }
                println!("{}", outs.join(" "));
                Ok(Val::None)
            }
            "sys.input" => {
                let mut s = String::new();
                match std::io::stdin().read_line(&mut s) {
                    Ok(_) => Ok(Val::Ok(Box::new(Val::Str(s.trim_end_matches('\n').trim_end_matches('\r').to_string())))),
                    Err(e) => Ok(Val::Err(e.to_string())),
                }
            }
            "sys.fs.denied" => Ok(Val::Err("capability denied: fs".into())),
            "sys.env.denied" => Ok(Val::Err("capability denied: env".into())),
            "sys.clock.denied" => Ok(Val::Err("capability denied: clock".into())),
            "sys.rand.denied" => Ok(Val::Err("capability denied: rand".into())),
            "sys.net.denied" => Ok(Val::Err("capability denied: net".into())),
            "sys.net.get" => {
                if arg_vals.len() != 1 {
                    return Err(Diag { code: "E0109", msg: "sys.net.get expects 1 arg".into(), line: callee.span.line, col: callee.span.col });
                }
                if let Val::Str(url) = &arg_vals[0] {
                    Ok(http_get(url))
                } else {
                    Ok(Val::Err("url must be string".into()))
                }
            }
            "sys.fs.read" => {
                if arg_vals.len() != 1 {
                    return Err(Diag { code: "E0109", msg: "sys.fs.read expects 1 arg".into(), line: callee.span.line, col: callee.span.col });
                }
                if let Val::Str(s) = &arg_vals[0] {
                    // Restrict traversal outside cwd if not absolute
                    let path = std::path::Path::new(s);
                    if !path.is_absolute() && path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                        return Ok(Val::Err("path traversal outside cwd is denied".into()));
                    }
                    match std::fs::read_to_string(s) {
                        Ok(content) => Ok(Val::Ok(Box::new(Val::Str(content)))),
                        Err(e) => Ok(Val::Err(e.to_string())),
                    }
                } else {
                    Ok(Val::Err("path must be string".into()))
                }
            }
            "sys.fs.read_bytes" => {
                if arg_vals.len() != 1 {
                    return Err(Diag { code: "E0109", msg: "sys.fs.read_bytes expects 1 arg".into(), line: callee.span.line, col: callee.span.col });
                }
                if let Val::Str(s) = &arg_vals[0] {
                    let path = std::path::Path::new(s);
                    if !path.is_absolute() && path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                        return Ok(Val::Err("path traversal outside cwd is denied".into()));
                    }
                    match std::fs::read(s) {
                        Ok(bytes) => {
                            let list: Vec<Val> = bytes.into_iter().map(|b| Val::Int(crate::bignum::BigInt::from_i64(b as i64))).collect();
                            Ok(Val::Ok(Box::new(Val::List(Rc::new(RefCell::new(list))))))
                        }
                        Err(e) => Ok(Val::Err(e.to_string())),
                    }
                } else {
                    Ok(Val::Err("path must be string".into()))
                }
            }
            "sys.fs.write" => {
                if arg_vals.len() != 2 {
                    return Err(Diag { code: "E0109", msg: "sys.fs.write expects 2 args".into(), line: callee.span.line, col: callee.span.col });
                }
                if let (Val::Str(s), Val::Str(data)) = (&arg_vals[0], &arg_vals[1]) {
                    let path = std::path::Path::new(s);
                    if !path.is_absolute() && path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                        return Ok(Val::Err("path traversal outside cwd is denied".into()));
                    }
                    match std::fs::write(s, data) {
                        Ok(_) => Ok(Val::Ok(Box::new(Val::None))),
                        Err(e) => Ok(Val::Err(e.to_string())),
                    }
                } else {
                    Ok(Val::Err("path and data must be strings".into()))
                }
            }
            "sys.fs.append" => {
                if arg_vals.len() != 2 {
                    return Err(Diag { code: "E0109", msg: "sys.fs.append expects 2 args".into(), line: callee.span.line, col: callee.span.col });
                }
                if let (Val::Str(s), Val::Str(data)) = (&arg_vals[0], &arg_vals[1]) {
                    let path = std::path::Path::new(s);
                    if !path.is_absolute() && path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                        return Ok(Val::Err("path traversal outside cwd is denied".into()));
                    }
                    use std::io::Write;
                    match std::fs::OpenOptions::new().create(true).append(true).open(s) {
                        Ok(mut file) => match file.write_all(data.as_bytes()) {
                            Ok(_) => Ok(Val::Ok(Box::new(Val::None))),
                            Err(e) => Ok(Val::Err(e.to_string())),
                        },
                        Err(e) => Ok(Val::Err(e.to_string())),
                    }
                } else {
                    Ok(Val::Err("path and data must be strings".into()))
                }
            }
            "sys.fs.exists" => {
                if arg_vals.len() != 1 {
                    return Err(Diag { code: "E0109", msg: "sys.fs.exists expects 1 arg".into(), line: callee.span.line, col: callee.span.col });
                }
                if let Val::Str(s) = &arg_vals[0] {
                    let path = std::path::Path::new(s);
                    if !path.is_absolute() && path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                        return Ok(Val::Err("path traversal outside cwd is denied".into()));
                    }
                    Ok(Val::Bool(std::path::Path::new(s).exists()))
                } else {
                    Ok(Val::Err("path must be string".into()))
                }
            }
            "sys.fs.list_dir" => {
                if arg_vals.len() != 1 {
                    return Err(Diag { code: "E0109", msg: "sys.fs.list_dir expects 1 arg".into(), line: callee.span.line, col: callee.span.col });
                }
                if let Val::Str(s) = &arg_vals[0] {
                    let path = std::path::Path::new(s);
                    if !path.is_absolute() && path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                        return Ok(Val::Err("path traversal outside cwd is denied".into()));
                    }
                    match std::fs::read_dir(s) {
                        Ok(entries) => {
                            let mut list = Vec::new();
                            for entry in entries {
                                if let Ok(entry) = entry {
                                    if let Ok(name) = entry.file_name().into_string() {
                                        list.push(Val::Str(name));
                                    }
                                }
                            }
                            Ok(Val::Ok(Box::new(Val::List(Rc::new(RefCell::new(list))))))
                        }
                        Err(e) => Ok(Val::Err(e.to_string())),
                    }
                } else {
                    Ok(Val::Err("path must be string".into()))
                }
            }
            "sys.fs.remove" => {
                if arg_vals.len() != 1 {
                    return Err(Diag { code: "E0109", msg: "sys.fs.remove expects 1 arg".into(), line: callee.span.line, col: callee.span.col });
                }
                if let Val::Str(s) = &arg_vals[0] {
                    let path = std::path::Path::new(s);
                    if !path.is_absolute() && path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                        return Ok(Val::Err("path traversal outside cwd is denied".into()));
                    }
                    if path.is_file() {
                        match std::fs::remove_file(s) {
                            Ok(_) => Ok(Val::Ok(Box::new(Val::None))),
                            Err(e) => Ok(Val::Err(e.to_string())),
                        }
                    } else if path.is_dir() {
                        match std::fs::remove_dir_all(s) {
                            Ok(_) => Ok(Val::Ok(Box::new(Val::None))),
                            Err(e) => Ok(Val::Err(e.to_string())),
                        }
                    } else {
                        Ok(Val::Err("path not found".into()))
                    }
                } else {
                    Ok(Val::Err("path must be string".into()))
                }
            }
            "sys.env.get" => {
                if arg_vals.len() != 1 {
                    return Err(Diag { code: "E0109", msg: "sys.env.get expects 1 arg".into(), line: callee.span.line, col: callee.span.col });
                }
                if let Val::Str(s) = &arg_vals[0] {
                    match std::env::var(s) {
                        Ok(v) => Ok(Val::Some(Box::new(Val::Str(v)))),
                        Err(_) => Ok(Val::None),
                    }
                } else {
                    Ok(Val::Err("key must be string".into()))
                }
            }
            "sys.env.set" => {
                if arg_vals.len() != 2 {
                    return Err(Diag { code: "E0109", msg: "sys.env.set expects 2 args".into(), line: callee.span.line, col: callee.span.col });
                }
                if let (Val::Str(k), Val::Str(v)) = (&arg_vals[0], &arg_vals[1]) {
                    std::env::set_var(k, v);
                    Ok(Val::None)
                } else {
                    Ok(Val::Err("key and value must be strings".into()))
                }
            }
            "sys.clock.now" => {
                let now = std::time::SystemTime::now();
                match now.duration_since(std::time::UNIX_EPOCH) {
                    // SPEC §10: unix milliseconds as an int, never a float.
                    Ok(d) => Ok(Val::Int(crate::bignum::BigInt::from_u64(d.as_millis() as u64))),
                    Err(_) => Ok(Val::Err("time went backwards".into())),
                }
            }
            "sys.clock.sleep" => {
                if arg_vals.len() != 1 {
                    return Err(Diag { code: "E0109", msg: "sys.clock.sleep expects 1 arg".into(), line: callee.span.line, col: callee.span.col });
                }
                let Val::Int(ms) = &arg_vals[0] else {
                    return Ok(Val::Err("sys.clock.sleep expects an int of milliseconds".into()));
                };
                // A negative or absurd duration sleeps not at all rather than
                // wedging the program forever.
                let ms = ms.to_usize().unwrap_or(0);
                std::thread::sleep(std::time::Duration::from_millis(ms as u64));
                Ok(Val::None)
            }
            "sys.rand.float" => {
                use std::io::Read;
                let mut buf = [0u8; 8];
                match std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut buf)) {
                    // 53 bits of entropy scaled into [0.0, 1.0) — every f64 in
                    // that interval with an exact 53-bit mantissa is reachable.
                    Ok(_) => {
                        let bits = u64::from_ne_bytes(buf) >> 11;
                        Ok(Val::Float(bits as f64 / (1u64 << 53) as f64))
                    }
                    Err(e) => Ok(Val::Err(e.to_string())),
                }
            }
            "sys.rand.bytes" => {
                if arg_vals.len() != 1 {
                    return Err(Diag { code: "E0109", msg: "sys.rand.bytes expects 1 arg".into(), line: callee.span.line, col: callee.span.col });
                }
                if let Val::Int(n) = &arg_vals[0] {
                    use std::io::Read;
                    let count = n.limbs.first().copied().unwrap_or(0) as usize;
                    let mut buf = vec![0u8; count];
                    match std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut buf)) {
                        Ok(_) => {
                            let list: Vec<Val> = buf.into_iter().map(|b| Val::Int(crate::bignum::BigInt::from_i64(b as i64))).collect();
                            Ok(Val::Ok(Box::new(Val::List(Rc::new(RefCell::new(list))))))
                        }
                        Err(e) => Ok(Val::Err(e.to_string())),
                    }
                } else {
                    Ok(Val::Err("length must be int".into()))
                }
            }
            "sys.rand.int" => {
                if arg_vals.len() != 2 {
                    return Err(Diag { code: "E0109", msg: "sys.rand.int expects 2 args".into(), line: callee.span.line, col: callee.span.col });
                }
                if let (Val::Int(min), Val::Int(max)) = (&arg_vals[0], &arg_vals[1]) {
                    use std::io::Read;
                    let min_val = min.limbs.first().copied().unwrap_or(0) as i64 * if min.sign { -1 } else { 1 };
                    let max_val = max.limbs.first().copied().unwrap_or(0) as i64 * if max.sign { -1 } else { 1 };
                    if min_val >= max_val {
                        return Ok(Val::Err("min must be < max".into()));
                    }
                    let mut buf = [0u8; 8];
                    match std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut buf)) {
                        Ok(_) => {
                            let r = u64::from_ne_bytes(buf) as i64;
                            let range = max_val - min_val;
                            let val = min_val + r.rem_euclid(range);
                            Ok(Val::Ok(Box::new(Val::Int(crate::bignum::BigInt::from_i64(val)))))
                        }
                        Err(e) => Ok(Val::Err(e.to_string())),
                    }
                } else {
                    Ok(Val::Err("bounds must be int".into()))
                }
            }
            "sort" => {
                if arg_vals.len() != 1 {
                    return Err(Diag { code: "E0109", msg: "sort expects 1 arg".into(), line: callee.span.line, col: callee.span.col });
                }
                if let Val::List(l) = &arg_vals[0] {
                    let mut b = l.borrow_mut();
                    b.sort_by(|a, b| {
                        if let (Val::Int(i1), Val::Int(i2)) = (a, b) {
                            i1.cmp(i2)
                        } else if let (Val::Str(s1), Val::Str(s2)) = (a, b) {
                            s1.cmp(s2)
                        } else {
                            std::cmp::Ordering::Equal
                        }
                    });
                    Ok(Val::None)
                } else {
                    Err(Diag { code: "E0111", msg: "sort expects list".into(), line: callee.span.line, col: callee.span.col })
                }
            }
            "map" => {
                if arg_vals.len() != 2 {
                    return Err(Diag { code: "E0109", msg: "map expects 2 args".into(), line: callee.span.line, col: callee.span.col });
                }
                let l_val = arg_vals[0].clone();
                let f_val = arg_vals[1].clone();
                if let Val::List(l) = l_val {
                    let mut outs = Vec::new();
                    let items = l.borrow().clone();
                    for item in items {
                        if let Val::Fn(params, body, closure_env) = &f_val {
                            let call_env = Rc::new(RefCell::new(Scope::new(Some(closure_env.clone()))));
                            if !params.is_empty() {
                                call_env.borrow_mut().define(params[0].clone(), item, false);
                            }
                            match self.eval_block(body, call_env) {
                                Ok(Ok(v)) => outs.push(v),
                                Ok(Err(Flow::Return(v))) => outs.push(v),
                                Err(e) => return Err(e),
                                _ => return Err(Diag { code: "E0110", msg: "flow error in map".into(), line: callee.span.line, col: callee.span.col })
                            }
                        } else {
                            return Err(Diag { code: "E0111", msg: "map expects fn".into(), line: callee.span.line, col: callee.span.col });
                        }
                    }
                    Ok(Val::List(Rc::new(RefCell::new(outs))))
                } else {
                    Err(Diag { code: "E0111", msg: "map expects list".into(), line: callee.span.line, col: callee.span.col })
                }
            }
            "filter" => {
                if arg_vals.len() != 2 {
                    return Err(Diag { code: "E0109", msg: "filter expects 2 args".into(), line: callee.span.line, col: callee.span.col });
                }
                let l_val = arg_vals[0].clone();
                let f_val = arg_vals[1].clone();
                if let Val::List(l) = l_val {
                    let mut outs = Vec::new();
                    let items = l.borrow().clone();
                    for item in items {
                        if let Val::Fn(params, body, closure_env) = &f_val {
                            let call_env = Rc::new(RefCell::new(Scope::new(Some(closure_env.clone()))));
                            if !params.is_empty() {
                                call_env.borrow_mut().define(params[0].clone(), item.clone(), false);
                            }
                            let res = match self.eval_block(body, call_env) {
                                Ok(Ok(v)) => v,
                                Ok(Err(Flow::Return(v))) => v,
                                Err(e) => return Err(e),
                                _ => return Err(Diag { code: "E0110", msg: "flow error in filter".into(), line: callee.span.line, col: callee.span.col })
                            };
                            if let Val::Bool(b) = res {
                                if b { outs.push(item); }
                            }
                        } else {
                            return Err(Diag { code: "E0111", msg: "filter expects fn".into(), line: callee.span.line, col: callee.span.col });
                        }
                    }
                    Ok(Val::List(Rc::new(RefCell::new(outs))))
                } else {
                    Err(Diag { code: "E0111", msg: "filter expects list".into(), line: callee.span.line, col: callee.span.col })
                }
            }
            _ => {
                if name.starts_with("sys.") {
                    Err(Diag {
                        code: "E0111",
                        msg: format!("unknown builtin '{}'", name),
                        line: callee.span.line,
                        col: callee.span.col,
                    })
                } else {
                    match crate::stdlib::eval_builtin(name, arg_vals) {
                        Ok(v) => Ok(v),
                        Err(e) => Err(Diag {
                            code: "E0111",
                            msg: e,
                            line: callee.span.line,
                            col: callee.span.col,
                        })
                    }
                }
            }
        }
    }

    pub fn match_pattern(&self, val: &Val, pat: &Pattern) -> Option<Vec<(String, Val)>> {
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
                } else if name == "some" {
                    if let Val::Some(inner) = val {
                        if binds.len() == 1 {
                            return Some(vec![(binds[0].clone(), *inner.clone())]);
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

/// VM-support surface: the same value operations the tree-walker uses, exposed
/// so `src/vm.rs` produces byte-identical results. These mirror the matching
/// arms of `eval_expr`/`eval_stmt`; the differential test guards against drift.
impl Evaluator {
    /// Dispatch a builtin by name (span is only used for error messages).
    pub fn run_builtin(&mut self, name: &str, args: Vec<Val>, line: u32, col: u32) -> Result<Val, Diag> {
        let dummy = Expr { span: Span { line, col }, kind: ExprKind::Ident(String::new()) };
        self.call_builtin(name, args, &dummy)
    }

    /// Run a user function value (`Val::Fn`) with already-evaluated args.
    pub fn call_user(&mut self, params: Vec<String>, body: Block, closure_env: Rc<RefCell<Scope>>, args: Vec<Val>, line: u32, col: u32) -> Result<Val, Diag> {
        if params.len() != args.len() {
            return Err(Diag { code: "E0109", msg: format!("expected {} args, got {}", params.len(), args.len()), line, col });
        }
        let call_env = Rc::new(RefCell::new(Scope::new(Some(closure_env))));
        for (p, v) in params.into_iter().zip(args) {
            call_env.borrow_mut().define(p, v, false);
        }
        match self.eval_block(&body, call_env) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(Flow::Return(v))) => Ok(v),
            Ok(Err(_)) => Err(Diag { code: "E0110", msg: "break/continue outside loop".into(), line, col }),
            Err(d) => {
                if d.code == "E_TRY_PROPAGATE" { Ok(Val::Err(d.msg)) } else { Err(d) }
            }
        }
    }

    /// Apply an already-evaluated callee to already-evaluated args, mirroring
    /// the `Call` dispatch in `eval_expr` (BoundMethod / enum-or-record ctor /
    /// user fn / builtin). `named` holds field names when the args were named.
    pub fn apply_callee(&mut self, callee_val: Val, arg_vals: Vec<Val>, named: Option<Vec<String>>, line: u32, col: u32) -> Result<Val, Diag> {
        match callee_val {
            Val::BoundMethod(obj, method) => {
                if let Val::Float(f) = *obj {
                    if method == "sqrt" && arg_vals.is_empty() {
                        return Ok(Val::Float(f.sqrt()));
                    }
                }
                let mut new_args = vec![*obj];
                new_args.extend(arg_vals);
                let func_val = self.global.borrow().get(&method).ok_or(Diag {
                    code: "E0111", msg: format!("method '{}' not found", method), line, col,
                })?;
                match func_val {
                    Val::Fn(params, body, closure_env) => self.call_user(params, body, closure_env, new_args, line, col),
                    Val::BuiltinFn(name) => self.run_builtin(name, new_args, line, col),
                    _ => Err(Diag { code: "E0111", msg: "not callable".into(), line, col }),
                }
            }
            Val::Enum(name, empty) if empty.is_empty() => {
                if let Some(field_names) = named {
                    let mut map = std::collections::HashMap::new();
                    for (k, v) in field_names.into_iter().zip(arg_vals) {
                        map.insert(k, v);
                    }
                    Ok(Val::Record(name, Rc::new(RefCell::new(map))))
                } else {
                    Ok(Val::Enum(name, arg_vals))
                }
            }
            Val::Fn(params, body, closure_env) => self.call_user(params, body, closure_env, arg_vals, line, col),
            Val::BuiltinFn(name) => self.run_builtin(name, arg_vals, line, col),
            _ => Err(Diag { code: "E0111", msg: "not callable".into(), line, col }),
        }
    }

    /// Field access, mirroring the `Field` arm of `eval_expr`.
    pub fn field_get(&mut self, obj: Val, f: &str, line: u32, col: u32) -> Result<Val, Diag> {
        match obj {
            Val::Err(s) => {
                if f == "msg" { Ok(Val::Str(s)) }
                else { Err(Diag { code: "E0105", msg: format!("no field '{}' on error", f), line, col }) }
            }
            Val::Record(_, ref r) => {
                if let Some(v) = r.borrow().get(f) { Ok(v.clone()) }
                else { Ok(Val::BoundMethod(Box::new(obj.clone()), f.to_string())) }
            }
            Val::Str(_) | Val::List(_) | Val::Map(_) => {
                Ok(Val::BoundMethod(Box::new(obj.clone()), f.to_string()))
            }
            _ => Ok(Val::BoundMethod(Box::new(obj.clone()), f.to_string())),
        }
    }

    /// Index access, mirroring the `Index` arm of `eval_expr`.
    pub fn index_get(&mut self, obj_val: Val, idx_val: Val, line: u32, col: u32) -> Result<Val, Diag> {
        match obj_val {
            Val::List(l) => {
                if let Val::Int(i) = idx_val {
                    let b = l.borrow();
                    let idx = i.to_f64() as usize;
                    if idx < b.len() { return Ok(b[idx].clone()); }
                }
                Err(Diag { code: "E0106", msg: "index out of bounds".into(), line, col })
            }
            Val::Map(m) => {
                if let Some(v) = m.borrow().get(&idx_val) { Ok(v.clone()) }
                else { Err(Diag { code: "E0106", msg: "key not found".into(), line, col }) }
            }
            _ => Err(Diag { code: "E0106", msg: "not indexable".into(), line, col }),
        }
    }
}

/// A `use` path is a local import if it references a `.heh` file (by relative
/// or absolute path) or a vendored module under `vendor/`.
fn is_local_import(path: &str) -> bool {
    path.ends_with(".heh")
        || path.starts_with("./")
        || path.starts_with("../")
        || path.starts_with('/')
        || path.starts_with("vendor/")
}

/// The scope name a local import binds to: the file stem of its last segment.
fn module_bind_name(path: &str) -> String {
    let last = path.rsplit('/').next().unwrap_or(path);
    last.strip_suffix(".heh").unwrap_or(last).to_string()
}

/// `sys.net.get(url)` — HTTP/1.1 over a std TcpStream for `http://`, delegating
/// `https://` to the `curl` subprocess (std has no TLS). Returns `ok(body)` or
/// `err(msg)`; never panics, fails closed on any error.
fn http_get(url: &str) -> Val {
    if let Some(rest) = url.strip_prefix("http://") {
        http_get_plain(rest)
    } else if url.starts_with("https://") {
        https_get_via_curl(url)
    } else {
        Val::Err(format!("sys.net.get: unsupported URL scheme in '{}'", url))
    }
}

fn http_get_plain(host_path: &str) -> Val {
    use std::io::{Read, Write};
    use std::time::Duration;

    let (authority, path) = match host_path.find('/') {
        Some(i) => (&host_path[..i], &host_path[i..]),
        None => (host_path, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => match p.parse::<u16>() {
            Ok(port) => (h.to_string(), port),
            Err(_) => return Val::Err(format!("sys.net.get: bad port in '{}'", authority)),
        },
        None => (authority.to_string(), 80u16),
    };

    let addr = format!("{}:{}", host, port);
    let stream = match std::net::TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(e) => return Val::Err(format!("sys.net.get: connect failed: {}", e)),
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));
    let mut stream = stream;

    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: heh\r\nAccept: */*\r\nConnection: close\r\n\r\n",
        path, host
    );
    if let Err(e) = stream.write_all(req.as_bytes()) {
        return Val::Err(format!("sys.net.get: write failed: {}", e));
    }

    let mut buf = Vec::new();
    if let Err(e) = stream.read_to_end(&mut buf) {
        return Val::Err(format!("sys.net.get: read failed: {}", e));
    }

    // Split headers from body on the first blank line.
    let split = buf.windows(4).position(|w| w == b"\r\n\r\n");
    let (head, body) = match split {
        Some(i) => (&buf[..i], &buf[i + 4..]),
        None => return Val::Err("sys.net.get: malformed response (no header terminator)".into()),
    };
    let status_line = String::from_utf8_lossy(head).lines().next().unwrap_or("").to_string();
    let code = status_line.split_whitespace().nth(1).and_then(|c| c.parse::<u16>().ok());
    match code {
        Some(c) if (200..300).contains(&c) => Val::Ok(Box::new(Val::Str(String::from_utf8_lossy(body).into_owned()))),
        Some(c) => Val::Err(format!("sys.net.get: HTTP {}", c)),
        None => Val::Err(format!("sys.net.get: bad status line '{}'", status_line)),
    }
}

fn https_get_via_curl(url: &str) -> Val {
    // std has no TLS; delegate to curl via an arg-list (never a shell string).
    match std::process::Command::new("curl")
        .arg("-sS")
        .arg("--fail")
        .arg("--max-time")
        .arg("30")
        .arg(url)
        .output()
    {
        Ok(out) if out.status.success() => Val::Ok(Box::new(Val::Str(String::from_utf8_lossy(&out.stdout).into_owned()))),
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            Val::Err(format!("sys.net.get: curl failed: {}", err.trim()))
        }
        Err(_) => Val::Err("sys.net.get: https requires the 'curl' program, which was not found".into()),
    }
}

/// A `try` that propagates out of top-level code has no function to return
/// from; report that rather than leaking the internal propagation signal.
fn rewrite_top_level_try(d: Diag) -> Diag {
    if d.code == "E_TRY_PROPAGATE" || d.code == "E_TRY_PROPAGATE_NONE" {
        return Diag {
            code: "E0114",
            msg: "try propagated outside result-returning function".into(),
            line: d.line,
            col: d.col,
        };
    }
    d
}
