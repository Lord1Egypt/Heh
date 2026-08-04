use crate::ast::*;
use crate::diag::Diag;
use std::collections::HashMap;

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
    VariadicFn(Box<Ty>, Box<Ty>),
    Namespace(String),
    Record(String),
    Enum(String),
    Unit,
    Any,   // Fallback for unresolved types
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
        }
    }

    pub fn check_file(&mut self, file: &File) {
        // Collect types and functions
        self.define("sys".to_string(), Ty::Namespace("sys".into()), false);
        self.define("int_of".to_string(), Ty::Any, false);
        self.define("float_of".to_string(), Ty::Any, false);
        self.define("str".to_string(), Ty::Any, false);
        // The explicit conversions (SPEC §5.2, §5.5). These carry real result
        // types so `int(x) + 1` still checks as int arithmetic.
        self.define(
            "int".to_string(),
            Ty::Fn(vec![Ty::Any], Box::new(Ty::Int)),
            false,
        );
        self.define(
            "float".to_string(),
            Ty::Fn(vec![Ty::Any], Box::new(Ty::Float)),
            false,
        );
        self.define("list".to_string(), Ty::Any, false);
        self.define("print".to_string(), Ty::Any, false); // In case they use it directly
        self.define("exit".to_string(), Ty::Any, false);
        self.define("len".to_string(), Ty::Any, false);
        self.define("push".to_string(), Ty::Any, false);
        self.define("pop".to_string(), Ty::Any, false);
        self.define("keys".to_string(), Ty::Any, false);
        self.define("values".to_string(), Ty::Any, false);
        self.define("read".to_string(), Ty::Any, false);
        self.define("write".to_string(), Ty::Any, false);
        self.define("split".to_string(), Ty::Any, false);
        self.define("join".to_string(), Ty::Any, false);

        self.define("ok".to_string(), Ty::Any, false);
        self.define("err".to_string(), Ty::Any, false);
        self.define("some".to_string(), Ty::Any, false);
        self.define("none".to_string(), Ty::Any, false);

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
                Ty::Any
            };
            self.define(bare, ty, false);
        }

        for item in &file.items {
            match item {
                TopItem::Type(t) => {
                    self.types.insert(t.name.clone(), t.clone());
                    // The constructor carries the type it builds, so
                    // `P(x: 1).x` and `p.x = 2` both know `p` is a record.
                    let built = match t.kind {
                        TypeDeclKind::Record(_) => Ty::Record(t.name.clone()),
                        TypeDeclKind::Enum(_) => Ty::Enum(t.name.clone()),
                    };
                    self.define(t.name.clone(), built, false);

                    if let TypeDeclKind::Enum(variants) = &t.kind {
                        for v in variants {
                            self.define(v.name.clone(), Ty::Enum(t.name.clone()), false);
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
        self.check_block(&f.body);
        self.current_fn_ret = prev_ret;
        self.pop_scope();
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

                if !lhs_ty.is_error()
                    && !rhs_ty.is_error()
                    && lhs_ty != Ty::Any
                    && rhs_ty != Ty::Any
                    && lhs_ty != rhs_ty
                {
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
                self.check_block(&w.body);
            }
            Statement::For(f) => {
                let iter_ty = self.check_expr(&f.iter);
                let elem_ty = match iter_ty {
                    Ty::List(inner) => *inner,
                    // Iterating a map yields its keys (SPEC §6.3).
                    Ty::Map(key, _) => *key,
                    Ty::Str => Ty::Str,
                    Ty::Any | Ty::Error => Ty::Any,
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
                // Can't use self.check_block directly because it pushes a scope.
                for stmt in &f.body.stmts {
                    self.check_stmt(stmt);
                }
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
                            // Scrutinee type is unknown (e.g. a module fn returning
                            // Any); bind every pattern variable permissively.
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
                                            self.define(binds[0].clone(), Ty::Str, false);
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
                        && ret_ty != *expected
                        && ret_ty != Ty::Any
                        && *expected != Ty::Any
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
            Statement::Break(_) | Statement::Continue(_) => {
                // Flow check is technically runtime or could be static, but E0110 handles it mostly.
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
                        // Any = not statically known (e.g. through a closure);
                        // the evaluator checks these at runtime.
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
                Literal::None => Ty::Any, // will be coerced or narrowed
            },
            ExprKind::Ident(name) => {
                if let Some((ty, _)) = self.lookup(name) {
                    ty
                } else if self.funcs.contains_key(name) {
                    let f = self.funcs.get(name).unwrap().clone();
                    let p_tys = f
                        .params
                        .iter()
                        .map(|p| {
                            p.typ
                                .as_ref()
                                .map(|t| self.resolve_type(t))
                                .unwrap_or(Ty::Any)
                        })
                        .collect();
                    let r_ty = f
                        .ret_type
                        .as_ref()
                        .map(|t| self.resolve_type(t))
                        .unwrap_or(Ty::Unit);
                    Ty::Fn(p_tys, Box::new(r_ty))
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
                    Ty::List(Box::new(Ty::Any))
                } else {
                    let mut elem_ty = Ty::Any;
                    for elem in elems {
                        let t = self.check_expr(elem);
                        if elem_ty == Ty::Any {
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
                    Ty::Map(Box::new(Ty::Any), Box::new(Ty::Any))
                } else {
                    let mut k_ty = Ty::Any;
                    let mut v_ty = Ty::Any;
                    for (k, v) in entries {
                        let t_k = self.check_expr(k);
                        let t_v = self.check_expr(v);
                        if k_ty == Ty::Any {
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
                        if v_ty == Ty::Any {
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
                        if let Some(ty) = namespace_member(&namespace, field_name) {
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
                            // Might be UFCS method call
                            Ty::Any
                        }
                    }
                    Ty::Any | Ty::Error => Ty::Any,
                    _ => {
                        // Might be UFCS method call (e.g. float.sqrt)
                        Ty::Any
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
                    Ty::Any | Ty::Error => Ty::Any,
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
            ExprKind::Try(inner, _) => {
                let inner_ty = self.check_expr(inner);
                match inner_ty {
                    Ty::Result(ok_ty) => *ok_ty,
                    Ty::Optional(inner_ty) => *inner_ty,
                    Ty::Any | Ty::Error => Ty::Any,
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
                match callee_ty {
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
                        *ret
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
                    Ty::Any | Ty::Error => Ty::Any,
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
            ExprKind::Record(name, fields) => {
                for (_, e_in) in fields {
                    self.check_expr(e_in);
                }
                Ty::Record(name.clone())
            }
            ExprKind::Closure(_params, _ret, _body) => {
                // Closures type-check to Any for now (parameter/return inference
                // is future work); their bodies are checked at call sites.
                Ty::Any
            }
        }
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
            // An unknown type is deliberately not rejected: imported and
            // capability-returned values currently enter the checker as Any.
            Ty::Any | Ty::Error => true,
            // Infinite scalar domains require a wildcard arm.
            _ => false,
        }
    }
}

fn namespace_member(namespace: &str, field: &str) -> Option<Ty> {
    let function = |params: Vec<Ty>, ret: Ty| Ty::Fn(params, Box::new(ret));
    let result = |inner: Ty| Ty::Result(Box::new(inner));
    let optional = |inner: Ty| Ty::Optional(Box::new(inner));
    let list = |inner: Ty| Ty::List(Box::new(inner));
    let member = match (namespace, field) {
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
    expected == actual
        || matches!(expected, Ty::Any | Ty::Error)
        || matches!(actual, Ty::Any | Ty::Error)
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
