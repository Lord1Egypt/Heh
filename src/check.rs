use crate::ast::*;
use crate::diag::Diag;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int,
    Float,
    Bool,
    Str,
    List(Box<Ty>),
    Map(Box<Ty>, Box<Ty>),
    Optional(Box<Ty>),
    Result(Box<Ty>), // `T or error`
    Fn(Vec<Ty>, Box<Ty>),
    NamedFn(Vec<(String, Ty)>, Box<Ty>),
    VariadicFn(Box<Ty>, Box<Ty>),
    Namespace(String),
    Builtin(String),
    RecordCtor(String, Vec<(String, Ty)>),
    EnumCtor(String, Vec<Ty>),
    Record(String),
    RecordValue(String, Vec<(String, Ty)>),
    Enum(String),
    Unit,
    // Runtime-dynamic values are confined to explicitly untyped boundaries:
    // JSON values and APIs that intentionally accept every runtime value.
    Any,
    // A locally polymorphic placeholder for literals/builtins whose type is
    // determined by context (`none`, `err`, and empty collections). Unlike
    // `Any`, this never represents a runtime-dynamic API boundary.
    Infer,
    Error, // To prevent cascading errors
}

impl Ty {
    pub fn is_error(&self) -> bool {
        matches!(self, Ty::Error)
    }
}

#[derive(Clone)]
pub struct Scope {
    pub vars: HashMap<String, (Ty, bool)>, // (type, is_mut)
}

pub struct Checker {
    pub diags: Vec<Diag>,
    pub scopes: Vec<Scope>,
    pub types: HashMap<String, TypeDecl>,
    pub funcs: HashMap<String, FnDecl>,
    // For return type checking
    pub current_fn_ret: Option<Ty>,
    loop_depth: usize,
    module_members: HashMap<String, HashMap<String, Ty>>,
    module_cache: HashMap<PathBuf, (String, HashMap<String, Ty>)>,
    module_loading: Vec<PathBuf>,
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}

impl Checker {
    pub fn new() -> Self {
        Self {
            diags: Vec::new(),
            scopes: vec![Scope {
                vars: HashMap::new(),
            }],
            types: HashMap::new(),
            funcs: HashMap::new(),
            current_fn_ret: None,
            loop_depth: 0,
            module_members: HashMap::new(),
            module_cache: HashMap::new(),
            module_loading: Vec::new(),
        }
    }

    pub fn check_file(&mut self, file: &File) {
        self.check_file_in(file, Path::new("."));
    }

    pub fn check_file_at(&mut self, file: &File, path: &Path) {
        let base = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        self.check_file_in(file, base);
    }

    fn check_file_in(&mut self, file: &File, base_dir: &Path) {
        // Collect types and functions
        self.define("sys".to_string(), Ty::Namespace("sys".into()), false);
        for name in [
            "len",
            "upper",
            "lower",
            "trim",
            "split",
            "replace",
            "contains",
            "starts_with",
            "chars",
            "push",
            "pop",
            "get",
            "sort",
            "map",
            "filter",
            "join",
            "set",
            "remove",
            "keys",
            "values",
            "int_of",
            "str",
            "int",
            "float",
            "list",
            "ok",
            "err",
            "some",
        ] {
            self.define(name.into(), Ty::Builtin(name.into()), false);
        }

        self.types.insert(
            "Sys".to_string(),
            TypeDecl {
                name: "Sys".to_string(),
                kind: TypeDeclKind::Record(vec![]), // Doesn't need real fields since we treat sys as Any
                span: Span { line: 0, col: 0 },
            },
        );

        // Names brought into scope by `use` (std modules or local imports).
        for u in &file.uses {
            let last = u.path.rsplit('/').next().unwrap_or(&u.path);
            let bare = last.strip_suffix(".heh").unwrap_or(last).to_string();
            let ty = if u.path.starts_with("std/") {
                Ty::Namespace(format!("std.{bare}"))
            } else {
                match self.load_module_interface(&u.path, base_dir, &u.span) {
                    Some(namespace) => Ty::Namespace(namespace),
                    None => Ty::Error,
                }
            };
            self.define(bare, ty, false);
        }

        for item in &file.items {
            match item {
                TopItem::Type(t) => {
                    self.types.insert(t.name.clone(), t.clone());
                    // The constructor carries the type it builds, so
                    // `P(x: 1).x` and `p.x = 2` both know `p` is a record.
                    let built = match &t.kind {
                        TypeDeclKind::Record(fields) => Ty::RecordCtor(
                            t.name.clone(),
                            fields
                                .iter()
                                .map(|field| (field.name.clone(), self.resolve_type(&field.typ)))
                                .collect(),
                        ),
                        TypeDeclKind::Enum(_) => Ty::Enum(t.name.clone()),
                    };
                    self.define(t.name.clone(), built, false);

                    if let TypeDeclKind::Enum(variants) = &t.kind {
                        for v in variants {
                            let payload: Vec<Ty> = v
                                .fields
                                .iter()
                                .map(|field| self.resolve_type(&field.typ))
                                .collect();
                            let variant_ty = if payload.is_empty() {
                                Ty::Enum(t.name.clone())
                            } else {
                                Ty::EnumCtor(t.name.clone(), payload)
                            };
                            self.define(v.name.clone(), variant_ty, false);
                        }
                    }
                }
                TopItem::Fn(f) => {
                    self.funcs.insert(f.name.clone(), f.clone());
                }
                _ => {}
            }
        }

        for item in &file.items {
            match item {
                TopItem::Fn(f) => self.check_fn(f),
                TopItem::Stmt(s) => self.check_stmt(s),
                TopItem::Let(l) => self.check_let(l),
                TopItem::Type(_) => {}
            }
        }
    }

    pub fn resolve_type(&mut self, t: &TypeExpr) -> Ty {
        let mut base_ty = match &t.kind {
            TypeExprKind::Named(name, args) => match name.as_str() {
                "int" => Ty::Int,
                "float" => Ty::Float,
                "bool" => Ty::Bool,
                "str" => Ty::Str,
                "Sys" => Ty::Namespace("sys".into()),
                "list" => {
                    if args.len() == 1 {
                        Ty::List(Box::new(self.resolve_type(&args[0])))
                    } else {
                        self.diags.push(Diag {
                            code: "E0050",
                            msg: "list takes 1 type argument".into(),
                            line: t.span.line,
                            col: t.span.col,
                        });
                        Ty::Error
                    }
                }
                "map" => {
                    if args.len() == 2 {
                        Ty::Map(
                            Box::new(self.resolve_type(&args[0])),
                            Box::new(self.resolve_type(&args[1])),
                        )
                    } else {
                        self.diags.push(Diag {
                            code: "E0050",
                            msg: "map takes 2 type arguments".into(),
                            line: t.span.line,
                            col: t.span.col,
                        });
                        Ty::Error
                    }
                }
                _ => {
                    if self.types.contains_key(name) {
                        let decl = self.types.get(name).unwrap().clone();
                        match decl.kind {
                            TypeDeclKind::Record(_) => Ty::Record(name.clone()),
                            TypeDeclKind::Enum(_) => Ty::Enum(name.clone()),
                        }
                    } else {
                        self.diags.push(Diag {
                            code: "E0051",
                            msg: format!("unknown type '{}'", name),
                            line: t.span.line,
                            col: t.span.col,
                        });
                        Ty::Error
                    }
                }
            },
            TypeExprKind::Fn(params, ret) => {
                let p_tys = params.iter().map(|p| self.resolve_type(p)).collect();
                let r_ty = match ret {
                    Some(r) => self.resolve_type(r),
                    None => Ty::Unit,
                };
                Ty::Fn(p_tys, Box::new(r_ty))
            }
        };

        if t.optional {
            base_ty = Ty::Optional(Box::new(base_ty));
        }
        if t.result {
            base_ty = Ty::Result(Box::new(base_ty));
        }

        base_ty
    }

    pub fn check_fn(&mut self, f: &FnDecl) {
        let mut p_tys = Vec::new();
        self.push_scope();
        for p in &f.params {
            let p_ty = if let Some(pt) = &p.typ {
                self.resolve_type(pt)
            } else {
                self.diags.push(Diag {
                    code: "E0052",
                    msg: "missing type annotation for parameter".into(),
                    line: p.span.line,
                    col: p.span.col,
                });
                Ty::Error
            };
            p_tys.push(p_ty.clone());
            self.define(p.name.clone(), p_ty, false);
        }
        let r_ty = match &f.ret_type {
            Some(r) => self.resolve_type(r),
            None => Ty::Unit,
        };
        let prev_ret = self.current_fn_ret.take();
        self.current_fn_ret = Some(r_ty.clone());
        let (implicit, always_returns) = self.check_function_block(&f.body);
        if r_ty != Ty::Unit && !always_returns {
            match implicit {
                Some(actual) if !types_compatible(&r_ty, &actual) => self.diags.push(Diag {
                    code: "E0040",
                    msg: "implicit return type mismatch".into(),
                    line: f.span.line,
                    col: f.span.col,
                }),
                None => self.diags.push(Diag {
                    code: "E0059",
                    msg: "function may finish without returning its declared type".into(),
                    line: f.span.line,
                    col: f.span.col,
                }),
                _ => {}
            }
        }
        self.current_fn_ret = prev_ret;
        self.pop_scope();
    }

    fn check_function_block(&mut self, block: &Block) -> (Option<Ty>, bool) {
        self.push_scope();
        let last = block.stmts.len().saturating_sub(1);
        let mut implicit = None;
        for (index, statement) in block.stmts.iter().enumerate() {
            if index == last {
                if let Statement::Expr(expr) = statement {
                    implicit = Some(self.check_expr(expr));
                    continue;
                }
            }
            self.check_stmt(statement);
        }
        let always_returns = self.block_always_returns_typed(block);
        self.pop_scope();
        (implicit, always_returns)
    }

    fn block_always_returns_typed(&mut self, block: &Block) -> bool {
        let Some(last) = block.stmts.last() else {
            return false;
        };
        match last {
            Statement::Return(_) => true,
            Statement::If(branch) => {
                self.block_always_returns_typed(&branch.then_block)
                    && branch
                        .elifs
                        .iter()
                        .all(|(_, block)| self.block_always_returns_typed(block))
                    && branch
                        .else_block
                        .as_ref()
                        .is_some_and(|block| self.block_always_returns_typed(block))
            }
            Statement::Match(branch) => {
                let ty = self.check_expr(&branch.expr);
                self.match_is_exhaustive(&ty, &branch.arms)
                    && branch
                        .arms
                        .iter()
                        .all(|arm| self.block_always_returns_typed(&arm.body))
            }
            _ => false,
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Scope {
            vars: HashMap::new(),
        });
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn define(&mut self, name: String, ty: Ty, is_mut: bool) {
        self.scopes
            .last_mut()
            .unwrap()
            .vars
            .insert(name, (ty, is_mut));
    }

    pub fn lookup(&self, name: &str) -> Option<(Ty, bool)> {
        for scope in self.scopes.iter().rev() {
            if let Some((ty, is_mut)) = scope.vars.get(name) {
                return Some((ty.clone(), *is_mut));
            }
        }
        None
    }

    pub fn check_block(&mut self, block: &Block) {
        self.push_scope();
        for s in &block.stmts {
            self.check_stmt(s);
        }
        self.pop_scope();
    }

    pub fn check_stmt(&mut self, s: &Statement) {
        match s {
            Statement::Let(l) => self.check_let(l),
            Statement::Assign(a) => {
                let lhs_ty = self.check_lvalue(&a.target);
                let rhs_ty = self.check_expr(&a.rhs);

                if a.target.tail.is_empty() {
                    if let Some((_, is_mut)) = self.lookup(&a.target.name) {
                        if !is_mut {
                            self.diags.push(Diag {
                                code: "E0010",
                                msg: format!("cannot reassign immutable '{}'", a.target.name),
                                line: a.span.line,
                                col: a.span.col,
                            });
                        }
                    } else {
                        self.diags.push(Diag {
                            code: "E0011",
                            msg: format!("unknown variable '{}'", a.target.name),
                            line: a.span.line,
                            col: a.span.col,
                        });
                    }
                }

                if !lhs_ty.is_error() && !rhs_ty.is_error() && !types_compatible(&lhs_ty, &rhs_ty) {
                    self.diags.push(Diag {
                        code: "E0040",
                        msg: "type mismatch in assignment".into(),
                        line: a.span.line,
                        col: a.span.col,
                    });
                }
            }
            Statement::If(i) => {
                let cond_ty = self.check_expr(&i.cond);
                if !cond_ty.is_error() && cond_ty != Ty::Bool && cond_ty != Ty::Any {
                    self.diags.push(Diag {
                        code: "E0041",
                        msg: "if condition must be bool".into(),
                        line: i.cond.span.line,
                        col: i.cond.span.col,
                    });
                }

                // Optional narrowing (SPEC §6.4). `if x != none` narrows x to T
                // inside the then-branch; `if x == none` followed by a jump out
                // narrows x to T for everything after the if.
                let narrowing =
                    none_comparison(&i.cond).and_then(|(name, is_neq)| match self.lookup(name) {
                        Some((Ty::Optional(inner), is_mut)) => {
                            Some((name.to_string(), is_neq, *inner, is_mut))
                        }
                        _ => None,
                    });

                match &narrowing {
                    Some((name, true, inner, is_mut)) => {
                        self.push_scope();
                        self.define(name.clone(), inner.clone(), *is_mut);
                        for stmt in &i.then_block.stmts {
                            self.check_stmt(stmt);
                        }
                        self.pop_scope();
                    }
                    _ => self.check_block(&i.then_block),
                }

                if let Some((name, false, inner, is_mut)) = narrowing {
                    if block_diverges(&i.then_block) {
                        self.define(name, inner, is_mut);
                    }
                }

                for (elif_cond, elif_block) in &i.elifs {
                    let elif_cond_ty = self.check_expr(elif_cond);
                    if !elif_cond_ty.is_error()
                        && elif_cond_ty != Ty::Bool
                        && elif_cond_ty != Ty::Any
                    {
                        self.diags.push(Diag {
                            code: "E0041",
                            msg: "elif condition must be bool".into(),
                            line: elif_cond.span.line,
                            col: elif_cond.span.col,
                        });
                    }
                    self.check_block(elif_block);
                }
                if let Some(else_block) = &i.else_block {
                    self.check_block(else_block);
                }
            }
            Statement::While(w) => {
                let cond_ty = self.check_expr(&w.cond);
                if !cond_ty.is_error() && cond_ty != Ty::Bool && cond_ty != Ty::Any {
                    self.diags.push(Diag {
                        code: "E0041",
                        msg: "while condition must be bool".into(),
                        line: w.cond.span.line,
                        col: w.cond.span.col,
                    });
                }
                self.loop_depth += 1;
                self.check_block(&w.body);
                self.loop_depth -= 1;
            }
            Statement::For(f) => {
                let iter_ty = self.check_expr(&f.iter);
                let elem_ty = match iter_ty {
                    Ty::List(inner) => *inner,
                    // Iterating a map yields its keys (SPEC §6.3).
                    Ty::Map(key, _) => *key,
                    Ty::Str => Ty::Str,
                    Ty::Any => Ty::Any,
                    Ty::Error => Ty::Error,
                    _ => {
                        self.diags.push(Diag {
                            code: "E0042",
                            msg: "can only iterate over a list, map, str, or range".into(),
                            line: f.iter.span.line,
                            col: f.iter.span.col,
                        });
                        Ty::Error
                    }
                };
                self.push_scope();
                self.define(f.name.clone(), elem_ty, false);
                self.loop_depth += 1;
                // Can't use self.check_block directly because it pushes a scope.
                for stmt in &f.body.stmts {
                    self.check_stmt(stmt);
                }
                self.loop_depth -= 1;
                self.pop_scope();
            }
            Statement::Match(m) => {
                let expr_ty = self.check_expr(&m.expr);
                if !self.match_is_exhaustive(&expr_ty, &m.arms) {
                    self.diags.push(Diag {
                        code: "E0020",
                        msg: "non-exhaustive match".into(),
                        line: m.span.line,
                        col: m.span.col,
                    });
                }
                for arm in &m.arms {
                    self.push_scope();
                    // Bind pattern variables based on expr_ty
                    match &arm.pattern {
                        Pattern::Variant(_, _name, binds) if expr_ty == Ty::Any => {
                            // A JSON scrutinee has no static shape; its pattern
                            // payloads are validated by the evaluator.
                            for b_name in binds {
                                self.define(b_name.clone(), Ty::Any, false);
                            }
                        }
                        Pattern::Variant(_, name, binds) => {
                            if let Ty::Enum(enum_name) = &expr_ty {
                                let variant_fields = if let Some(TypeDecl {
                                    kind: TypeDeclKind::Enum(variants),
                                    ..
                                }) = self.types.get(enum_name)
                                {
                                    variants
                                        .iter()
                                        .find(|v| v.name == *name)
                                        .map(|v| v.fields.clone())
                                } else {
                                    None
                                };

                                if let Some(fields) = variant_fields {
                                    if binds.len() == fields.len() {
                                        for (b_name, field) in binds.iter().zip(fields.iter()) {
                                            let f_ty = self.resolve_type(&field.typ);
                                            self.define(b_name.clone(), f_ty, false);
                                        }
                                    } else {
                                        self.diags.push(Diag {
                                            code: "E0044",
                                            msg: format!(
                                                "variant '{}' takes {} args, {} given",
                                                name,
                                                fields.len(),
                                                binds.len()
                                            ),
                                            line: arm.span.line,
                                            col: arm.span.col,
                                        });
                                    }
                                } else {
                                    self.diags.push(Diag {
                                        code: "E0045",
                                        msg: format!(
                                            "unknown variant '{}' for enum '{}'",
                                            name, enum_name
                                        ),
                                        line: arm.span.line,
                                        col: arm.span.col,
                                    });
                                }
                            } else {
                                // For now, if matching a literal or result/option
                                if name == "ok" {
                                    if let Ty::Result(inner) = &expr_ty {
                                        if binds.len() == 1 {
                                            self.define(binds[0].clone(), *inner.clone(), false);
                                        }
                                    }
                                } else if name == "err" {
                                    if let Ty::Result(_) = &expr_ty {
                                        if binds.len() == 1 {
                                            self.define(
                                                binds[0].clone(),
                                                Ty::RecordValue(
                                                    "error".into(),
                                                    vec![("msg".into(), Ty::Str)],
                                                ),
                                                false,
                                            );
                                        }
                                    }
                                } else if name == "some" {
                                    if let Ty::Optional(inner) = &expr_ty {
                                        if binds.len() == 1 {
                                            self.define(binds[0].clone(), *inner.clone(), false);
                                        }
                                    }
                                }
                            }
                        }
                        Pattern::Wildcard(_) | Pattern::Literal(_) => {}
                    }

                    for stmt in &arm.body.stmts {
                        self.check_stmt(stmt);
                    }
                    self.pop_scope();
                }
            }
            Statement::Return(r) => {
                let ret_ty = if let Some(e) = &r.expr {
                    self.check_expr(e)
                } else {
                    Ty::Unit
                };
                if let Some(expected) = &self.current_fn_ret {
                    if !ret_ty.is_error()
                        && !expected.is_error()
                        && !types_compatible(expected, &ret_ty)
                    {
                        self.diags.push(Diag {
                            code: "E0040",
                            msg: "type mismatch in return".into(),
                            line: r.span.line,
                            col: r.span.col,
                        });
                    }
                } else {
                    self.diags.push(Diag {
                        code: "E0043",
                        msg: "return outside function".into(),
                        line: r.span.line,
                        col: r.span.col,
                    });
                }
            }
            Statement::Break(span) | Statement::Continue(span) => {
                if self.loop_depth == 0 {
                    self.diags.push(Diag {
                        code: "E0110",
                        msg: "break/continue outside loop".into(),
                        line: span.line,
                        col: span.col,
                    });
                }
            }
            Statement::Expr(e) => {
                self.check_expr(e);
            }
        }
    }

    pub fn check_let(&mut self, l: &LetStmt) {
        let init_ty = self.check_expr(&l.init);
        self.define(l.name.clone(), init_ty, l.is_mut);
    }

    pub fn check_lvalue(&mut self, lval: &LValue) -> Ty {
        let mut curr_ty = if let Some((ty, _)) = self.lookup(&lval.name) {
            ty
        } else {
            self.diags.push(Diag {
                code: "E0011",
                msg: format!("unknown variable '{}'", lval.name),
                line: lval.span.line,
                col: lval.span.col,
            });
            return Ty::Error;
        };

        for tail in &lval.tail {
            match tail {
                LValueTail::Field(f) => {
                    curr_ty = match curr_ty {
                        Ty::Record(name) => {
                            let field_typ = if let Some(TypeDecl {
                                kind: TypeDeclKind::Record(fields),
                                ..
                            }) = self.types.get(&name)
                            {
                                fields
                                    .iter()
                                    .find(|field| field.name == *f)
                                    .map(|field| field.typ.clone())
                            } else {
                                None
                            };

                            if let Some(t) = field_typ {
                                self.resolve_type(&t)
                            } else {
                                self.diags.push(Diag {
                                    code: "E0053",
                                    msg: format!("unknown field '{}' on record '{}'", f, name),
                                    line: lval.span.line,
                                    col: lval.span.col,
                                });
                                Ty::Error
                            }
                        }
                        Ty::RecordValue(name, fields) => fields
                            .iter()
                            .find(|(field, _)| field == f)
                            .map(|(_, ty)| ty.clone())
                            .unwrap_or_else(|| {
                                self.diags.push(Diag {
                                    code: "E0053",
                                    msg: format!("unknown field '{}' on record '{}'", f, name),
                                    line: lval.span.line,
                                    col: lval.span.col,
                                });
                                Ty::Error
                            }),
                        // A JSON-derived record shape is checked at runtime.
                        Ty::Any | Ty::Error => curr_ty,
                        _ => {
                            self.diags.push(Diag {
                                code: "E0054",
                                msg: "field access on non-record".into(),
                                line: lval.span.line,
                                col: lval.span.col,
                            });
                            Ty::Error
                        }
                    };
                }
                LValueTail::Index(idx_expr) => {
                    let idx_ty = self.check_expr(idx_expr);
                    curr_ty = match curr_ty {
                        Ty::List(inner) => {
                            if !idx_ty.is_error() && idx_ty != Ty::Any && idx_ty != Ty::Int {
                                self.diags.push(Diag {
                                    code: "E0055",
                                    msg: "list index must be int".into(),
                                    line: lval.span.line,
                                    col: lval.span.col,
                                });
                            }
                            *inner
                        }
                        Ty::Map(k, v) => {
                            if !idx_ty.is_error() && idx_ty != Ty::Any && idx_ty != *k {
                                self.diags.push(Diag {
                                    code: "E0056",
                                    msg: "map index type mismatch".into(),
                                    line: lval.span.line,
                                    col: lval.span.col,
                                });
                            }
                            *v
                        }
                        Ty::Any | Ty::Error => curr_ty,
                        _ => {
                            self.diags.push(Diag {
                                code: "E0057",
                                msg: "index on non-collection".into(),
                                line: lval.span.line,
                                col: lval.span.col,
                            });
                            Ty::Error
                        }
                    }
                }
            }
        }
        curr_ty
    }

    pub fn check_expr(&mut self, e: &Expr) -> Ty {
        match &e.kind {
            ExprKind::Literal(lit) => match lit {
                Literal::Int(_) => Ty::Int,
                Literal::Float(_) => Ty::Float,
                Literal::Bool(_) => Ty::Bool,
                Literal::Str(_) => Ty::Str,
                Literal::None => Ty::Infer,
            },
            ExprKind::Ident(name) => {
                if let Some((ty, _)) = self.lookup(name) {
                    ty
                } else if self.funcs.contains_key(name) {
                    let f = self.funcs.get(name).unwrap().clone();
                    let p_tys: Vec<Ty> = f
                        .params
                        .iter()
                        .map(|p| {
                            p.typ
                                .as_ref()
                                .map(|t| self.resolve_type(t))
                                .unwrap_or(Ty::Error)
                        })
                        .collect();
                    let r_ty = f
                        .ret_type
                        .as_ref()
                        .map(|t| self.resolve_type(t))
                        .unwrap_or(Ty::Unit);
                    Ty::NamedFn(
                        f.params.iter().map(|p| p.name.clone()).zip(p_tys).collect(),
                        Box::new(r_ty),
                    )
                } else {
                    self.diags.push(Diag {
                        code: "E0011",
                        msg: format!("unknown variable '{}'", name),
                        line: e.span.line,
                        col: e.span.col,
                    });
                    Ty::Error
                }
            }
            ExprKind::Binary(op, left, right) => {
                let l_ty = self.check_expr(left);
                let r_ty = self.check_expr(right);
                match op {
                    BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::Mod
                    | BinOp::Pow
                    | BinOp::FloorDiv => {
                        if l_ty == Ty::Any || r_ty == Ty::Any {
                            Ty::Any
                        } else if l_ty == Ty::Int && r_ty == Ty::Int {
                            Ty::Int
                        } else if l_ty == Ty::Float && r_ty == Ty::Float {
                            Ty::Float
                        } else if *op == BinOp::Add && l_ty == Ty::Str && r_ty == Ty::Str {
                            Ty::Str
                        } else {
                            if !l_ty.is_error() && !r_ty.is_error() {
                                self.diags.push(Diag {
                                    code: "E0040",
                                    msg: "type mismatch in binary op".into(),
                                    line: e.span.line,
                                    col: e.span.col,
                                });
                            }
                            Ty::Error
                        }
                    }
                    BinOp::Eq | BinOp::Neq => Ty::Bool,
                    BinOp::Lt | BinOp::Leq | BinOp::Gt | BinOp::Geq => {
                        // Any = not statically known; comparable operands must
                        // otherwise be two ints or two floats (SPEC §1.2: no
                        // implicit coercion).
                        let comparable = l_ty == Ty::Any
                            || r_ty == Ty::Any
                            || (l_ty == Ty::Int && r_ty == Ty::Int)
                            || (l_ty == Ty::Float && r_ty == Ty::Float);
                        if comparable {
                            Ty::Bool
                        } else {
                            if !l_ty.is_error() && !r_ty.is_error() {
                                self.diags.push(Diag {
                                    code: "E0040",
                                    msg: "type mismatch in comparison".into(),
                                    line: e.span.line,
                                    col: e.span.col,
                                });
                            }
                            Ty::Error
                        }
                    }
                    BinOp::And | BinOp::Or => {
                        let logical = l_ty == Ty::Any
                            || r_ty == Ty::Any
                            || (l_ty == Ty::Bool && r_ty == Ty::Bool);
                        if logical {
                            Ty::Bool
                        } else {
                            if !l_ty.is_error() && !r_ty.is_error() {
                                self.diags.push(Diag {
                                    code: "E0040",
                                    msg: "type mismatch in logical op".into(),
                                    line: e.span.line,
                                    col: e.span.col,
                                });
                            }
                            Ty::Error
                        }
                    }
                    BinOp::Range | BinOp::RangeInc => {
                        // An unbounded range (`0..`) carries a `none` upper
                        // bound, so only the start pins the element type.
                        let bounded = r_ty == Ty::Int
                            || matches!(right.kind, ExprKind::Literal(Literal::None));
                        if l_ty == Ty::Int && bounded {
                            Ty::List(Box::new(Ty::Int))
                        } else if l_ty == Ty::Any || r_ty == Ty::Any {
                            Ty::Any
                        } else {
                            Ty::Error
                        }
                    }
                }
            }
            ExprKind::Unary(op, inner) => {
                let inner_ty = self.check_expr(inner);
                match op {
                    UnOp::Not => {
                        if !inner_ty.is_error() && inner_ty != Ty::Bool {
                            self.diags.push(Diag {
                                code: "E0040",
                                msg: "type mismatch in not".into(),
                                line: e.span.line,
                                col: e.span.col,
                            });
                        }
                        Ty::Bool
                    }
                    UnOp::Neg => {
                        if inner_ty == Ty::Int || inner_ty == Ty::Float {
                            inner_ty
                        } else {
                            if !inner_ty.is_error() {
                                self.diags.push(Diag {
                                    code: "E0040",
                                    msg: "type mismatch in neg".into(),
                                    line: e.span.line,
                                    col: e.span.col,
                                });
                            }
                            Ty::Error
                        }
                    }
                }
            }
            ExprKind::List(elems) => {
                if elems.is_empty() {
                    Ty::List(Box::new(Ty::Infer))
                } else {
                    let mut elem_ty = Ty::Infer;
                    for elem in elems {
                        let t = self.check_expr(elem);
                        if elem_ty == Ty::Infer {
                            elem_ty = t;
                        } else if !t.is_error() && t != elem_ty {
                            self.diags.push(Diag {
                                code: "E0040",
                                msg: "list elements must have same type".into(),
                                line: e.span.line,
                                col: e.span.col,
                            });
                            elem_ty = Ty::Error;
                            break;
                        }
                    }
                    Ty::List(Box::new(elem_ty))
                }
            }
            ExprKind::Map(entries) => {
                if entries.is_empty() {
                    Ty::Map(Box::new(Ty::Infer), Box::new(Ty::Infer))
                } else {
                    let mut k_ty = Ty::Infer;
                    let mut v_ty = Ty::Infer;
                    for (k, v) in entries {
                        let t_k = self.check_expr(k);
                        let t_v = self.check_expr(v);
                        if k_ty == Ty::Infer {
                            k_ty = t_k;
                        } else if !t_k.is_error() && t_k != k_ty {
                            self.diags.push(Diag {
                                code: "E0040",
                                msg: "map keys must have same type".into(),
                                line: e.span.line,
                                col: e.span.col,
                            });
                            k_ty = Ty::Error;
                        }
                        if v_ty == Ty::Infer {
                            v_ty = t_v;
                        } else if !t_v.is_error() && t_v != v_ty {
                            self.diags.push(Diag {
                                code: "E0040",
                                msg: "map values must have same type".into(),
                                line: e.span.line,
                                col: e.span.col,
                            });
                            v_ty = Ty::Error;
                        }
                    }
                    Ty::Map(Box::new(k_ty), Box::new(v_ty))
                }
            }
            ExprKind::Field(base, field_name) => {
                let base_ty = self.check_expr(base);
                if let Some(method) = builtin_method(&base_ty, field_name) {
                    return method;
                }
                match base_ty {
                    Ty::Namespace(namespace) => {
                        let module_ty = self
                            .module_members
                            .get(&namespace)
                            .and_then(|members| members.get(field_name))
                            .cloned();
                        if let Some(ty) =
                            module_ty.or_else(|| namespace_member(&namespace, field_name))
                        {
                            ty
                        } else {
                            self.diags.push(Diag {
                                code: "E0053",
                                msg: format!("unknown field '{}' on '{}'", field_name, namespace),
                                line: e.span.line,
                                col: e.span.col,
                            });
                            Ty::Error
                        }
                    }
                    Ty::Record(name) => {
                        let field_typ = if let Some(TypeDecl {
                            kind: TypeDeclKind::Record(fields),
                            ..
                        }) = self.types.get(&name)
                        {
                            fields
                                .iter()
                                .find(|field| field.name == *field_name)
                                .map(|field| field.typ.clone())
                        } else {
                            None
                        };

                        if let Some(t) = field_typ {
                            self.resolve_type(&t)
                        } else {
                            self.diags.push(Diag {
                                code: "E0053",
                                msg: format!("unknown field '{}' on '{}'", field_name, name),
                                line: e.span.line,
                                col: e.span.col,
                            });
                            Ty::Error
                        }
                    }
                    Ty::RecordValue(name, fields) => fields
                        .iter()
                        .find(|(field, _)| field == field_name)
                        .map(|(_, ty)| ty.clone())
                        .unwrap_or_else(|| {
                            self.diags.push(Diag {
                                code: "E0053",
                                msg: format!("unknown field '{}' on '{}'", field_name, name),
                                line: e.span.line,
                                col: e.span.col,
                            });
                            Ty::Error
                        }),
                    // JSON is the sole value-producing dynamic boundary. Its
                    // shape is checked by the evaluator when a field is read.
                    Ty::Any => Ty::Any,
                    Ty::Error => Ty::Error,
                    _ => {
                        self.diags.push(Diag {
                            code: "E0053",
                            msg: format!("unknown field '{}'", field_name),
                            line: e.span.line,
                            col: e.span.col,
                        });
                        Ty::Error
                    }
                }
            }
            ExprKind::Index(base, idx) => {
                let base_ty = self.check_expr(base);
                let idx_ty = self.check_expr(idx);
                match base_ty {
                    Ty::List(inner) => {
                        if !idx_ty.is_error() && idx_ty != Ty::Any && idx_ty != Ty::Int {
                            self.diags.push(Diag {
                                code: "E0055",
                                msg: "list index must be int".into(),
                                line: e.span.line,
                                col: e.span.col,
                            });
                        }
                        *inner
                    }
                    Ty::Map(k, v) => {
                        if !idx_ty.is_error() && idx_ty != Ty::Any && idx_ty != *k {
                            self.diags.push(Diag {
                                code: "E0056",
                                msg: "map index type mismatch".into(),
                                line: e.span.line,
                                col: e.span.col,
                            });
                        }
                        *v
                    }
                    Ty::Str => {
                        if !idx_ty.is_error() && idx_ty != Ty::Any && idx_ty != Ty::Int {
                            self.diags.push(Diag {
                                code: "E0055",
                                msg: "str index must be int".into(),
                                line: e.span.line,
                                col: e.span.col,
                            });
                        }
                        Ty::Str
                    }
                    Ty::Any => Ty::Any,
                    Ty::Error => Ty::Error,
                    _ => {
                        self.diags.push(Diag {
                            code: "E0057",
                            msg: "index on non-collection".into(),
                            line: e.span.line,
                            col: e.span.col,
                        });
                        Ty::Error
                    }
                }
            }
            ExprKind::Try(inner, else_exit) => {
                let inner_ty = self.check_expr(inner);
                if !else_exit {
                    let legal = matches!(
                        (&inner_ty, &self.current_fn_ret),
                        (Ty::Result(_), Some(Ty::Result(_)))
                            | (Ty::Optional(_), Some(Ty::Optional(_)))
                    ) || self.current_fn_ret.is_none(); // frozen script-mode behavior
                    if !legal && !inner_ty.is_error() {
                        self.diags.push(Diag {
                            code: "E0114",
                            msg: "try propagation requires a compatible result-returning function"
                                .into(),
                            line: e.span.line,
                            col: e.span.col,
                        });
                    }
                }
                match inner_ty {
                    Ty::Result(ok_ty) => *ok_ty,
                    Ty::Optional(inner_ty) => *inner_ty,
                    Ty::Any => Ty::Any,
                    Ty::Error => Ty::Error,
                    _ => {
                        self.diags.push(Diag {
                            code: "E0021",
                            msg: "try on non-result/optional".into(),
                            line: e.span.line,
                            col: e.span.col,
                        });
                        Ty::Error
                    }
                }
            }
            ExprKind::InterpStr(parts) => {
                for p in parts {
                    if let InterpPart::Expr(e_in) = p {
                        self.check_expr(e_in);
                    }
                }
                Ty::Str
            }
            ExprKind::Call(callee, args) => {
                let callee_ty = self.check_expr(callee);
                let arg_tys: Vec<_> = args
                    .iter()
                    .map(|arg| match arg {
                        CallArg::Positional(a) | CallArg::Named(_, a) => self.check_expr(a),
                    })
                    .collect();
                let higher_order_return = match (&callee.kind, arg_tys.first()) {
                    (ExprKind::Field(_, method), Some(Ty::Fn(_, ret))) if method == "map" => {
                        Some(Ty::List(ret.clone()))
                    }
                    (ExprKind::Field(_, method), Some(Ty::NamedFn(_, ret))) if method == "map" => {
                        Some(Ty::List(ret.clone()))
                    }
                    _ => None,
                };
                match callee_ty {
                    Ty::Builtin(name) => self.check_builtin_call(&name, &arg_tys, e),
                    Ty::NamedFn(params, ret) => {
                        self.check_named_call(&params, args, &arg_tys, e);
                        *ret
                    }
                    Ty::RecordCtor(name, fields) => {
                        self.check_record_constructor(&name, &fields, args, &arg_tys, e)
                    }
                    Ty::EnumCtor(name, fields) => {
                        self.check_enum_constructor(&name, &fields, args, &arg_tys, e)
                    }
                    Ty::Fn(params, ret) => {
                        if args.len() != params.len() {
                            self.diags.push(Diag {
                                code: "E0109",
                                msg: format!(
                                    "function takes {} arguments, {} given",
                                    params.len(),
                                    args.len()
                                ),
                                line: e.span.line,
                                col: e.span.col,
                            });
                        } else {
                            let named_params = match &callee.kind {
                                ExprKind::Ident(name) => self.funcs.get(name).map(|f| {
                                    f.params.iter().map(|p| p.name.as_str()).collect::<Vec<_>>()
                                }),
                                _ => None,
                            };
                            for (index, (arg, actual)) in args.iter().zip(&arg_tys).enumerate() {
                                let expected = match (arg, &named_params) {
                                    (CallArg::Named(name, _), Some(names)) => names
                                        .iter()
                                        .position(|candidate| *candidate == name)
                                        .and_then(|position| params.get(position)),
                                    (CallArg::Named(_, _), None) => None,
                                    (CallArg::Positional(_), _) => params.get(index),
                                };
                                if let Some(expected) = expected {
                                    if !types_compatible(expected, actual) {
                                        self.diags.push(Diag {
                                            code: "E0040",
                                            msg: "function argument type mismatch".into(),
                                            line: e.span.line,
                                            col: e.span.col,
                                        });
                                    }
                                } else if let CallArg::Named(name, _) = arg {
                                    self.diags.push(Diag {
                                        code: "E0109",
                                        msg: format!("unknown named argument '{}'", name),
                                        line: e.span.line,
                                        col: e.span.col,
                                    });
                                }
                            }
                        }
                        higher_order_return.unwrap_or(*ret)
                    }
                    Ty::VariadicFn(param, ret) => {
                        for actual in &arg_tys {
                            if !types_compatible(&param, actual) {
                                self.diags.push(Diag {
                                    code: "E0040",
                                    msg: "function argument type mismatch".into(),
                                    line: e.span.line,
                                    col: e.span.col,
                                });
                            }
                        }
                        *ret
                    }
                    Ty::Enum(e) => Ty::Enum(e.clone()),
                    Ty::Record(r) => Ty::Record(r.clone()),
                    Ty::Any => Ty::Any,
                    Ty::Error => Ty::Error,
                    _ => {
                        self.diags.push(Diag {
                            code: "E0058",
                            msg: "call on non-function".into(),
                            line: e.span.line,
                            col: e.span.col,
                        });
                        Ty::Error
                    }
                }
            }
            ExprKind::Record(name, fields) => Ty::RecordValue(
                name.clone(),
                fields
                    .iter()
                    .map(|(field, value)| (field.clone(), self.check_expr(value)))
                    .collect(),
            ),
            ExprKind::Closure(params, ret, body) => {
                let mut param_tys = Vec::with_capacity(params.len());
                self.push_scope();
                for param in params {
                    let ty = if let Some(annotation) = &param.typ {
                        self.resolve_type(annotation)
                    } else {
                        self.diags.push(Diag {
                            code: "E0052",
                            msg: "missing type annotation for closure parameter".into(),
                            line: param.span.line,
                            col: param.span.col,
                        });
                        Ty::Error
                    };
                    param_tys.push(ty.clone());
                    self.define(param.name.clone(), ty, false);
                }
                let return_ty = ret
                    .as_ref()
                    .map(|annotation| self.resolve_type(annotation))
                    .unwrap_or(Ty::Unit);
                let previous = self.current_fn_ret.replace(return_ty.clone());
                let outer_loop_depth = std::mem::replace(&mut self.loop_depth, 0);
                let (implicit, always_returns) = self.check_function_block(body);
                if return_ty != Ty::Unit && !always_returns {
                    match implicit {
                        Some(actual) if !types_compatible(&return_ty, &actual) => {
                            self.diags.push(Diag {
                                code: "E0040",
                                msg: "implicit closure return type mismatch".into(),
                                line: e.span.line,
                                col: e.span.col,
                            })
                        }
                        None => self.diags.push(Diag {
                            code: "E0059",
                            msg: "closure may finish without returning its declared type".into(),
                            line: e.span.line,
                            col: e.span.col,
                        }),
                        _ => {}
                    }
                }
                self.current_fn_ret = previous;
                self.loop_depth = outer_loop_depth;
                self.pop_scope();
                Ty::Fn(param_tys, Box::new(return_ty))
            }
        }
    }

    fn check_named_call(
        &mut self,
        params: &[(String, Ty)],
        args: &[CallArg],
        arg_tys: &[Ty],
        expr: &Expr,
    ) {
        let mut used = std::collections::HashSet::new();
        let mut saw_named = false;
        let mut positional = 0usize;
        for (arg, actual) in args.iter().zip(arg_tys) {
            let index = match arg {
                CallArg::Positional(_) => {
                    if saw_named {
                        self.diags.push(Diag {
                            code: "E0109",
                            msg: "positional argument after named argument".into(),
                            line: expr.span.line,
                            col: expr.span.col,
                        });
                    }
                    let index = positional;
                    positional += 1;
                    Some(index)
                }
                CallArg::Named(name, _) => {
                    saw_named = true;
                    params
                        .iter()
                        .position(|(candidate, _)| candidate == name)
                        .or_else(|| {
                            self.diags.push(Diag {
                                code: "E0109",
                                msg: format!("unknown named argument '{}'", name),
                                line: expr.span.line,
                                col: expr.span.col,
                            });
                            None
                        })
                }
            };
            if let Some(index) = index {
                if index >= params.len() {
                    self.diags.push(Diag {
                        code: "E0109",
                        msg: "too many arguments".into(),
                        line: expr.span.line,
                        col: expr.span.col,
                    });
                } else if !used.insert(index) {
                    self.diags.push(Diag {
                        code: "E0109",
                        msg: format!("duplicate argument '{}'", params[index].0),
                        line: expr.span.line,
                        col: expr.span.col,
                    });
                } else if !types_compatible(&params[index].1, actual) {
                    self.diags.push(Diag {
                        code: "E0040",
                        msg: "function argument type mismatch".into(),
                        line: expr.span.line,
                        col: expr.span.col,
                    });
                }
            }
        }
        if used.len() != params.len() {
            self.diags.push(Diag {
                code: "E0109",
                msg: "missing function argument".into(),
                line: expr.span.line,
                col: expr.span.col,
            });
        }
    }

    fn check_record_constructor(
        &mut self,
        name: &str,
        fields: &[(String, Ty)],
        args: &[CallArg],
        arg_tys: &[Ty],
        expr: &Expr,
    ) -> Ty {
        let mut seen = std::collections::HashSet::new();
        if args.len() != fields.len() {
            self.diags.push(Diag {
                code: "E0109",
                msg: format!(
                    "record '{}' needs {} fields, {} given",
                    name,
                    fields.len(),
                    args.len()
                ),
                line: expr.span.line,
                col: expr.span.col,
            });
        }
        for (arg, actual) in args.iter().zip(arg_tys) {
            let CallArg::Named(field, _) = arg else {
                self.diags.push(Diag {
                    code: "E0109",
                    msg: "record fields must be named".into(),
                    line: expr.span.line,
                    col: expr.span.col,
                });
                continue;
            };
            if !seen.insert(field) {
                self.diags.push(Diag {
                    code: "E0109",
                    msg: format!("duplicate field '{}'", field),
                    line: expr.span.line,
                    col: expr.span.col,
                });
            } else if let Some((_, expected)) =
                fields.iter().find(|(candidate, _)| candidate == field)
            {
                if !types_compatible(expected, actual) {
                    self.diags.push(Diag {
                        code: "E0040",
                        msg: format!("field '{}' type mismatch", field),
                        line: expr.span.line,
                        col: expr.span.col,
                    });
                }
            } else {
                self.diags.push(Diag {
                    code: "E0109",
                    msg: format!("unknown field '{}'", field),
                    line: expr.span.line,
                    col: expr.span.col,
                });
            }
        }
        Ty::RecordValue(name.into(), fields.to_vec())
    }

    fn check_enum_constructor(
        &mut self,
        name: &str,
        fields: &[Ty],
        args: &[CallArg],
        arg_tys: &[Ty],
        expr: &Expr,
    ) -> Ty {
        if args.len() != fields.len() {
            self.diags.push(Diag {
                code: "E0109",
                msg: format!(
                    "variant takes {} arguments, {} given",
                    fields.len(),
                    args.len()
                ),
                line: expr.span.line,
                col: expr.span.col,
            });
        }
        for (arg, (expected, actual)) in args.iter().zip(fields.iter().zip(arg_tys)) {
            if matches!(arg, CallArg::Named(_, _)) {
                self.diags.push(Diag {
                    code: "E0109",
                    msg: "enum payloads are positional".into(),
                    line: expr.span.line,
                    col: expr.span.col,
                });
            }
            // Frozen v1 corpus behavior accepts an integer literal for a
            // float enum payload (`circle(10)`). This contextual constructor
            // rule does not enable coercion in ordinary expressions.
            if !types_compatible(expected, actual)
                && !matches!((expected, actual), (Ty::Float, Ty::Int))
            {
                self.diags.push(Diag {
                    code: "E0040",
                    msg: "variant argument type mismatch".into(),
                    line: expr.span.line,
                    col: expr.span.col,
                });
            }
        }
        Ty::Enum(name.into())
    }

    fn load_module_interface(
        &mut self,
        import: &str,
        base_dir: &Path,
        span: &Span,
    ) -> Option<String> {
        let with_ext = if import.ends_with(".heh") {
            import.into()
        } else {
            format!("{import}.heh")
        };
        let resolved = base_dir.join(with_ext);
        let canonical = match resolved.canonicalize() {
            Ok(path) => path,
            Err(_) => {
                self.diags.push(Diag {
                    code: "E0032",
                    msg: format!("cannot find imported file '{}'", import),
                    line: span.line,
                    col: span.col,
                });
                return None;
            }
        };
        if self.module_loading.contains(&canonical) {
            self.diags.push(Diag {
                code: "E0030",
                msg: format!("import cycle through '{}'", import),
                line: span.line,
                col: span.col,
            });
            return None;
        }
        let source = match std::fs::read_to_string(&canonical) {
            Ok(source) => source,
            Err(error) => {
                self.diags.push(Diag {
                    code: "E0032",
                    msg: format!("cannot read '{}': {}", import, error),
                    line: span.line,
                    col: span.col,
                });
                return None;
            }
        };
        let hash = crate::modules::sha256_hex(source.as_bytes());
        if let Some((cached_hash, members)) = self.module_cache.get(&canonical) {
            if cached_hash == &hash {
                let namespace = canonical.to_string_lossy().into_owned();
                self.module_members
                    .insert(namespace.clone(), members.clone());
                return Some(namespace);
            }
        }
        self.module_loading.push(canonical.clone());
        let tokens = match crate::lexer::lex(&source) {
            Ok(tokens) => tokens,
            Err(diag) => {
                self.diags.push(Diag {
                    code: "E0033",
                    msg: format!("in imported '{}': {}", import, diag.msg),
                    line: span.line,
                    col: span.col,
                });
                self.module_loading.pop();
                return None;
            }
        };
        let file = match crate::parser::Parser::new(&tokens).parse_file() {
            Ok(file) => file,
            Err(diag) => {
                self.diags.push(Diag {
                    code: "E0033",
                    msg: format!("in imported '{}': {}", import, diag.msg),
                    line: span.line,
                    col: span.col,
                });
                self.module_loading.pop();
                return None;
            }
        };
        let module_base = canonical.parent().unwrap_or(Path::new("."));
        for nested in &file.uses {
            if !nested.path.starts_with("std/") {
                self.load_module_interface(&nested.path, module_base, &nested.span);
            }
        }
        let mut resolver = Checker::new();
        for item in &file.items {
            if let TopItem::Type(decl) = item {
                resolver.types.insert(decl.name.clone(), decl.clone());
            }
        }
        let mut members = HashMap::new();
        for item in &file.items {
            match item {
                TopItem::Fn(function) => {
                    let params: Vec<Ty> = function
                        .params
                        .iter()
                        .map(|param| {
                            param
                                .typ
                                .as_ref()
                                .map(|ty| resolver.resolve_type(ty))
                                .unwrap_or(Ty::Error)
                        })
                        .collect();
                    let ret = function
                        .ret_type
                        .as_ref()
                        .map(|ty| resolver.resolve_type(ty))
                        .unwrap_or(Ty::Unit);
                    let named = function
                        .params
                        .iter()
                        .map(|param| param.name.clone())
                        .zip(params)
                        .collect();
                    members.insert(function.name.clone(), Ty::NamedFn(named, Box::new(ret)));
                }
                TopItem::Type(decl) => match &decl.kind {
                    TypeDeclKind::Record(fields) => {
                        let typed = fields
                            .iter()
                            .map(|field| (field.name.clone(), resolver.resolve_type(&field.typ)))
                            .collect();
                        members.insert(decl.name.clone(), Ty::RecordCtor(decl.name.clone(), typed));
                    }
                    TypeDeclKind::Enum(variants) => {
                        for variant in variants {
                            let typed: Vec<Ty> = variant
                                .fields
                                .iter()
                                .map(|field| resolver.resolve_type(&field.typ))
                                .collect();
                            let variant_ty = if typed.is_empty() {
                                Ty::Enum(decl.name.clone())
                            } else {
                                Ty::EnumCtor(decl.name.clone(), typed)
                            };
                            members.insert(variant.name.clone(), variant_ty);
                        }
                    }
                },
                TopItem::Let(_) | TopItem::Stmt(_) => {}
            }
        }
        self.diags.extend(resolver.diags);
        let namespace = canonical.to_string_lossy().into_owned();
        self.module_cache
            .insert(canonical.clone(), (hash.clone(), members.clone()));
        self.module_members
            .insert(namespace.clone(), members.clone());
        self.module_loading.pop();

        // Check imported code in isolated lexical scopes. Interfaces already
        // cached above make nested imports deterministic and avoid execution.
        let mut validator = Checker::new();
        validator.module_cache = self.module_cache.clone();
        validator.module_members = self.module_members.clone();
        validator.check_file_at(&file, &canonical);
        for item in &file.items {
            if let TopItem::Let(binding) = item {
                if let Some((ty, _)) = validator.lookup(&binding.name) {
                    members.insert(binding.name.clone(), ty);
                }
            }
        }
        for diag in validator.diags {
            self.diags.push(Diag {
                code: "E0033",
                msg: format!("in imported '{}': {}", import, diag.msg),
                line: span.line,
                col: span.col,
            });
        }
        self.module_cache.insert(canonical, (hash, members.clone()));
        self.module_members.insert(namespace.clone(), members);
        Some(namespace)
    }

    fn check_builtin_call(&mut self, name: &str, args: &[Ty], expr: &Expr) -> Ty {
        let arity = |checker: &mut Checker, expected: usize| {
            if args.len() != expected {
                checker.diags.push(Diag {
                    code: "E0109",
                    msg: format!(
                        "function takes {} arguments, {} given",
                        expected,
                        args.len()
                    ),
                    line: expr.span.line,
                    col: expr.span.col,
                });
                false
            } else {
                true
            }
        };
        let special_arity = match name {
            "map" | "filter" => Some(2),
            "some" | "ok" | "err" | "str" | "int" | "float" | "int_of" | "list" => Some(1),
            _ => None,
        };
        if let Some(expected) = special_arity {
            if !arity(self, expected) {
                return Ty::Error;
            }
        }
        match name {
            "map" => match (&args[0], callable_unary(&args[1])) {
                (Ty::List(element), Some((param, ret))) => {
                    if !types_compatible(element, param) {
                        self.builtin_type_error(name, expr);
                    }
                    Ty::List(Box::new(ret.clone()))
                }
                _ => {
                    self.builtin_type_error(name, expr);
                    Ty::Error
                }
            },
            "filter" => match (&args[0], callable_unary(&args[1])) {
                (Ty::List(element), Some((param, ret))) => {
                    if !types_compatible(element, param) || !types_compatible(&Ty::Bool, ret) {
                        self.builtin_type_error(name, expr);
                    }
                    Ty::List(element.clone())
                }
                _ => {
                    self.builtin_type_error(name, expr);
                    Ty::Error
                }
            },
            "some" => Ty::Optional(Box::new(args[0].clone())),
            "ok" => Ty::Result(Box::new(args[0].clone())),
            "err" => {
                self.require_builtin_arg(name, &Ty::Str, &args[0], expr);
                Ty::Result(Box::new(Ty::Infer))
            }
            "str" => Ty::Str,
            "int" => {
                if !matches!(args[0], Ty::Int | Ty::Float | Ty::Error) {
                    self.builtin_type_error(name, expr);
                }
                Ty::Int
            }
            "float" => {
                if !matches!(args[0], Ty::Int | Ty::Float | Ty::Error) {
                    self.builtin_type_error(name, expr);
                }
                Ty::Float
            }
            "int_of" => {
                self.require_builtin_arg(name, &Ty::Str, &args[0], expr);
                Ty::Result(Box::new(Ty::Int))
            }
            "list" => match &args[0] {
                Ty::List(inner) => Ty::List(inner.clone()),
                Ty::Str => Ty::List(Box::new(Ty::Str)),
                Ty::Map(key, _) => Ty::List(key.clone()),
                Ty::Any => Ty::List(Box::new(Ty::Any)),
                Ty::Error => Ty::Error,
                _ => {
                    self.builtin_type_error(name, expr);
                    Ty::Error
                }
            },
            _ => self.check_method_style_builtin(name, args, expr, arity),
        }
    }

    fn check_method_style_builtin<F>(
        &mut self,
        name: &str,
        args: &[Ty],
        expr: &Expr,
        arity: F,
    ) -> Ty
    where
        F: Fn(&mut Checker, usize) -> bool,
    {
        if args.is_empty() {
            arity(self, 1);
            return Ty::Error;
        }
        if let Some(Ty::Fn(params, ret)) = builtin_method(&args[0], name) {
            if !arity(self, params.len() + 1) {
                return *ret;
            }
            for (expected, actual) in params.iter().zip(&args[1..]) {
                self.require_builtin_arg(name, expected, actual, expr);
            }
            return *ret;
        }
        self.builtin_type_error(name, expr);
        Ty::Error
    }

    fn require_builtin_arg(&mut self, name: &str, expected: &Ty, actual: &Ty, expr: &Expr) {
        if !types_compatible(expected, actual) {
            self.builtin_type_error(name, expr);
        }
    }

    fn builtin_type_error(&mut self, name: &str, expr: &Expr) {
        self.diags.push(Diag {
            code: "E0040",
            msg: format!("invalid argument type for builtin '{}'", name),
            line: expr.span.line,
            col: expr.span.col,
        });
    }

    fn match_is_exhaustive(&self, ty: &Ty, arms: &[MatchArm]) -> bool {
        if arms
            .iter()
            .any(|arm| matches!(arm.pattern, Pattern::Wildcard(_)))
        {
            return true;
        }
        let has_variant = |wanted: &str| {
            arms.iter()
                .any(|arm| matches!(&arm.pattern, Pattern::Variant(_, name, _) if name == wanted))
        };
        let has_literal = |wanted: &Literal| {
            arms.iter()
                .any(|arm| matches!(&arm.pattern, Pattern::Literal(got) if got == wanted))
        };
        match ty {
            Ty::Enum(name) => self
                .types
                .get(name)
                .and_then(|decl| match &decl.kind {
                    TypeDeclKind::Enum(variants) => Some(variants),
                    TypeDeclKind::Record(_) => None,
                })
                .is_some_and(|variants| variants.iter().all(|v| has_variant(&v.name))),
            Ty::Optional(_) => has_variant("some") && has_literal(&Literal::None),
            Ty::Result(_) => has_variant("ok") && has_variant("err"),
            Ty::Bool => has_literal(&Literal::Bool(true)) && has_literal(&Literal::Bool(false)),
            // Runtime-dynamic JSON cannot be proven exhaustive statically.
            Ty::Any | Ty::Infer | Ty::Error => true,
            // Infinite scalar domains require a wildcard arm.
            _ => false,
        }
    }
}

fn callable_unary(ty: &Ty) -> Option<(&Ty, &Ty)> {
    match ty {
        Ty::Fn(params, ret) if params.len() == 1 => Some((&params[0], ret)),
        Ty::NamedFn(params, ret) if params.len() == 1 => Some((&params[0].1, ret)),
        _ => None,
    }
}

fn namespace_member(namespace: &str, field: &str) -> Option<Ty> {
    let function = |params: Vec<Ty>, ret: Ty| Ty::Fn(params, Box::new(ret));
    let result = |inner: Ty| Ty::Result(Box::new(inner));
    let optional = |inner: Ty| Ty::Optional(Box::new(inner));
    let list = |inner: Ty| Ty::List(Box::new(inner));
    let member = match (namespace, field) {
        // Printing and JSON serialization intentionally accept every runtime
        // value; argument inspection is performed by their runtime encoders.
        ("sys", "print") => Ty::VariadicFn(Box::new(Ty::Any), Box::new(Ty::Unit)),
        ("sys", "input") => function(vec![], result(Ty::Str)),
        ("sys", "args") => list(Ty::Str),
        ("sys", capability @ ("fs" | "net" | "env" | "clock" | "rand")) => {
            Ty::Namespace(format!("sys.{capability}"))
        }
        ("sys.fs", "read") => function(vec![Ty::Str], result(Ty::Str)),
        ("sys.fs", "read_bytes") => function(vec![Ty::Str], result(list(Ty::Int))),
        ("sys.fs", "write" | "append") => function(vec![Ty::Str, Ty::Str], result(Ty::Unit)),
        ("sys.fs", "exists") => function(vec![Ty::Str], Ty::Bool),
        ("sys.fs", "list_dir") => function(vec![Ty::Str], result(list(Ty::Str))),
        ("sys.fs", "remove") => function(vec![Ty::Str], result(Ty::Unit)),
        ("sys.net", "get") => function(vec![Ty::Str], result(Ty::Str)),
        ("sys.env", "get") => function(vec![Ty::Str], optional(Ty::Str)),
        ("sys.env", "set") => function(vec![Ty::Str, Ty::Str], Ty::Unit),
        ("sys.clock", "now") => function(vec![], Ty::Int),
        ("sys.clock", "sleep") => function(vec![Ty::Int], Ty::Unit),
        ("sys.rand", "bytes") => function(vec![Ty::Int], result(list(Ty::Int))),
        ("sys.rand", "int") => function(vec![Ty::Int, Ty::Int], result(Ty::Int)),
        ("sys.rand", "float") => function(vec![], result(Ty::Float)),
        ("std.math", "sin" | "cos" | "sqrt" | "abs" | "log" | "floor" | "ceil") => {
            function(vec![Ty::Float], Ty::Float)
        }
        ("std.math", "pow") => function(vec![Ty::Float, Ty::Float], Ty::Float),
        ("std.math", "pi" | "e") => function(vec![], Ty::Float),
        ("std.fmt", "pad_left" | "pad_right") => function(vec![Ty::Str, Ty::Int, Ty::Str], Ty::Str),
        ("std.fmt", "repeat") => function(vec![Ty::Str, Ty::Int], Ty::Str),
        ("std.fmt", "hex") => function(vec![Ty::Int], Ty::Str),
        ("std.fmt", "fixed") => function(vec![Ty::Float, Ty::Int], Ty::Str),
        // Parsed JSON has no frozen static shape/type syntax, so this is the
        // checker’s only value-producing dynamic boundary.
        ("std.json", "parse") => function(vec![Ty::Str], result(Ty::Any)),
        ("std.json", "write") => function(vec![Ty::Any], Ty::Str),
        ("std.csv", "parse") => function(vec![Ty::Str], list(list(Ty::Str))),
        ("std.csv", "write") => function(vec![list(list(Ty::Str))], Ty::Str),
        ("std.hash", "sha256" | "crc32") => function(vec![Ty::Str], Ty::Str),
        ("std.regex", "is_match") => function(vec![Ty::Str, Ty::Str], Ty::Bool),
        ("std.regex", "find") => function(vec![Ty::Str, Ty::Str], result(Ty::Str)),
        ("std.time", "format") => function(vec![Ty::Int], Ty::Str),
        ("std.time", "parts") => {
            function(vec![Ty::Int], Ty::Map(Box::new(Ty::Str), Box::new(Ty::Int)))
        }
        ("std.time", "from_parts") => function(vec![Ty::Int; 6], result(Ty::Int)),
        ("std.time", "is_leap") => function(vec![Ty::Int], Ty::Bool),
        ("std.time", "days_in_month") => function(vec![Ty::Int, Ty::Int], result(Ty::Int)),
        ("std.debug", "fault") => function(vec![Ty::Str], Ty::Unit),
        ("std.debug", "assert") => function(vec![Ty::Bool, Ty::Str], Ty::Unit),
        _ => return None,
    };
    Some(member)
}

fn builtin_method(receiver: &Ty, field: &str) -> Option<Ty> {
    let function = |params: Vec<Ty>, ret: Ty| Ty::Fn(params, Box::new(ret));
    match (receiver, field) {
        (Ty::Str, "len") => Some(function(vec![], Ty::Int)),
        (Ty::Str, "upper" | "lower" | "trim") => Some(function(vec![], Ty::Str)),
        (Ty::Str, "split") => Some(function(vec![Ty::Str], Ty::List(Box::new(Ty::Str)))),
        (Ty::Str, "replace") => Some(function(vec![Ty::Str, Ty::Str], Ty::Str)),
        (Ty::Str, "contains" | "starts_with") => Some(function(vec![Ty::Str], Ty::Bool)),
        (Ty::Str, "chars") => Some(function(vec![], Ty::List(Box::new(Ty::Str)))),
        (Ty::List(inner), "len") | (Ty::Map(_, inner), "len") => {
            let _ = inner;
            Some(function(vec![], Ty::Int))
        }
        (Ty::List(inner), "push") => Some(function(vec![*inner.clone()], Ty::Unit)),
        (Ty::List(inner), "pop") => Some(function(vec![], Ty::Result(inner.clone()))),
        (Ty::List(inner), "get") => Some(function(vec![Ty::Int], Ty::Optional(inner.clone()))),
        (Ty::List(_), "sort") => Some(function(vec![], Ty::Unit)),
        (Ty::List(inner), "map") => Some(function(
            vec![Ty::Fn(vec![*inner.clone()], Box::new(Ty::Infer))],
            Ty::List(Box::new(Ty::Infer)),
        )),
        (Ty::List(inner), "filter") => Some(function(
            vec![Ty::Fn(vec![*inner.clone()], Box::new(Ty::Bool))],
            Ty::List(inner.clone()),
        )),
        (Ty::List(_), "join") => Some(function(vec![Ty::Str], Ty::Str)),
        (Ty::Map(key, value), "get") => {
            Some(function(vec![*key.clone()], Ty::Optional(value.clone())))
        }
        (Ty::Map(key, value), "set") => {
            Some(function(vec![*key.clone(), *value.clone()], Ty::Unit))
        }
        (Ty::Map(key, _), "remove") => Some(function(vec![*key.clone()], Ty::Unit)),
        (Ty::Map(key, _), "keys") => Some(function(vec![], Ty::List(key.clone()))),
        (Ty::Map(_, value), "values") => Some(function(vec![], Ty::List(value.clone()))),
        _ => None,
    }
}

fn types_compatible(expected: &Ty, actual: &Ty) -> bool {
    if expected == actual
        || matches!(expected, Ty::Any | Ty::Infer | Ty::Error)
        || matches!(actual, Ty::Any | Ty::Infer | Ty::Error)
    {
        return true;
    }
    match (expected, actual) {
        (Ty::List(a), Ty::List(b))
        | (Ty::Optional(a), Ty::Optional(b))
        | (Ty::Result(a), Ty::Result(b)) => types_compatible(a, b),
        (Ty::Map(ak, av), Ty::Map(bk, bv)) => types_compatible(ak, bk) && types_compatible(av, bv),
        (Ty::Fn(ap, ar), Ty::Fn(bp, br)) => {
            ap.len() == bp.len()
                && ap.iter().zip(bp).all(|(a, b)| types_compatible(a, b))
                && types_compatible(ar, br)
        }
        (Ty::NamedFn(ap, ar), Ty::NamedFn(bp, br)) => {
            ap.len() == bp.len()
                && ap
                    .iter()
                    .zip(bp)
                    .all(|((_, a), (_, b))| types_compatible(a, b))
                && types_compatible(ar, br)
        }
        (Ty::Fn(ap, ar), Ty::NamedFn(bp, br)) | (Ty::NamedFn(bp, br), Ty::Fn(ap, ar)) => {
            ap.len() == bp.len()
                && ap.iter().zip(bp).all(|(a, (_, b))| types_compatible(a, b))
                && types_compatible(ar, br)
        }
        (Ty::Record(a), Ty::RecordValue(b, _))
        | (Ty::RecordValue(a, _), Ty::Record(b))
        | (Ty::RecordValue(a, _), Ty::RecordValue(b, _)) => a == b,
        _ => false,
    }
}

/// Recognise the two `none` comparisons that drive optional narrowing
/// (SPEC §6.4): returns the compared variable and whether the test was `!=`.
/// Either operand order is accepted.
pub fn none_comparison(cond: &Expr) -> Option<(&str, bool)> {
    let ExprKind::Binary(op, left, right) = &cond.kind else {
        return None;
    };
    let is_neq = match op {
        BinOp::Neq => true,
        BinOp::Eq => false,
        _ => return None,
    };
    let is_none = |e: &Expr| matches!(&e.kind, ExprKind::Literal(Literal::None));
    match (&left.kind, &right.kind) {
        (ExprKind::Ident(name), _) if is_none(right) => Some((name.as_str(), is_neq)),
        (_, ExprKind::Ident(name)) if is_none(left) => Some((name.as_str(), is_neq)),
        _ => None,
    }
}

/// Whether a block always leaves the enclosing block — the condition under
/// which `if x == none` narrows `x` for the statements that follow it.
fn block_diverges(b: &Block) -> bool {
    matches!(
        b.stmts.last(),
        Some(Statement::Return(_) | Statement::Break(_) | Statement::Continue(_))
    )
}
