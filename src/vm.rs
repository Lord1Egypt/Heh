//! P11 stack VM: executes the bytecode from `src/compile.rs`.
//!
//! Values use the same `Val`/`Scope`/bignum as the tree-walker, and every leaf
//! value operation delegates to the shared `Evaluator` helpers, so output is
//! byte-identical (guaranteed by the differential test in `tests/vm.rs`). The
//! VM owns control flow, function frames, and expression sequencing.

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::{BinOp, Span};
use crate::bignum::BigInt;
use crate::compile::{Chunk, Const, Op, Program};
use crate::diag::Diag;
use crate::eval::{Evaluator, Scope};
use crate::val::Val;

enum IterState {
    Range {
        next: Val,
        end: Option<Val>,
        inclusive: bool,
    },
    List {
        items: Vec<Val>,
        idx: usize,
    },
}

pub struct Vm {
    eval: Evaluator,
}

impl Vm {
    pub fn new(eval: Evaluator) -> Self {
        Vm { eval }
    }

    /// Run a compiled program: the top-level statements, then `main(sys)` if any.
    pub fn run(&mut self, program: &Program) -> Result<(), Diag> {
        let global = self.eval.global.clone();
        self.exec_chunk(&program.top_level, program, global.clone())?;

        if let Some(main_idx) = program.main {
            let chunk = &program.functions[main_idx];
            let scope = Rc::new(RefCell::new(Scope::new(Some(global.clone()))));
            if !chunk.params.is_empty() {
                if let Some(sys) = global.borrow().get("sys") {
                    scope
                        .borrow_mut()
                        .define(chunk.params[0].clone(), sys, false);
                }
            }
            match self.exec_chunk(chunk, program, scope) {
                Ok(_) => {}
                Err(d) if d.code == "E_TRY_PROPAGATE" || d.code == "E_TRY_PROPAGATE_NONE" => {
                    return Err(Diag {
                        code: "E0114",
                        msg: "try propagated outside result-returning function".into(),
                        line: 1,
                        col: 1,
                    });
                }
                Err(d) => return Err(d),
            }
        }
        Ok(())
    }

    fn exec_chunk(
        &mut self,
        chunk: &Chunk,
        program: &Program,
        scope: Rc<RefCell<Scope>>,
    ) -> Result<Val, Diag> {
        let mut stack: Vec<Val> = Vec::new();
        let mut iters: Vec<IterState> = Vec::new();
        // Block scopes opened inside this chunk; `scopes[0]` is the chunk's own.
        let mut scopes: Vec<Rc<RefCell<Scope>>> = vec![scope];
        let mut ip = 0usize;

        while ip < chunk.ops.len() {
            match &chunk.ops[ip] {
                Op::PushConst(i) => stack.push(const_to_val(&chunk.consts[*i])),
                Op::PushNone => stack.push(Val::None),
                Op::PushBool(b) => stack.push(Val::Bool(*b)),
                Op::Pop => {
                    stack.pop();
                }
                Op::Load(name) => {
                    let v = cur_scope(&scopes)
                        .borrow()
                        .get(name)
                        .unwrap_or_else(|| Val::Enum(name.clone(), vec![]));
                    stack.push(v);
                }
                Op::Define(name, is_mut) => {
                    let v = stack.pop().unwrap();
                    cur_scope(&scopes)
                        .borrow_mut()
                        .define(name.clone(), v, *is_mut);
                }
                Op::Assign(name, line, col) => {
                    let v = stack.pop().unwrap();
                    if let Err(is_mut) = cur_scope(&scopes).borrow_mut().set(name, v) {
                        return Err(assign_err(name, is_mut, *line, *col));
                    }
                }
                Op::OpAssign(name, op, line, col) => {
                    let rhs = stack.pop().unwrap();
                    let cur = cur_scope(&scopes).borrow().get(name).ok_or(Diag {
                        code: "E0103",
                        msg: format!("undefined variable '{}'", name),
                        line: *line,
                        col: *col,
                    })?;
                    let res = self
                        .eval
                        .eval_binop(op.clone(), cur, rhs, span(*line, *col))?;
                    if let Err(is_mut) = cur_scope(&scopes).borrow_mut().set(name, res) {
                        return Err(assign_err(name, is_mut, *line, *col));
                    }
                }
                Op::Binop(op, line, col) => {
                    let r = stack.pop().unwrap();
                    let l = stack.pop().unwrap();
                    if *op == BinOp::Pow {
                        if let (Val::Float(a), Val::Float(b)) = (&l, &r) {
                            stack.push(Val::Float(a.powf(*b)));
                            ip += 1;
                            continue;
                        }
                    }
                    stack.push(self.eval.eval_binop(op.clone(), l, r, span(*line, *col))?);
                }
                Op::Neg(line, col) => {
                    let v = stack.pop().unwrap();
                    match v {
                        Val::Int(i) => stack.push(Val::Int(i.negate())),
                        Val::Float(f) => stack.push(Val::Float(-f)),
                        _ => {
                            return Err(Diag {
                                code: "E0104",
                                msg: "bad type for '-'".into(),
                                line: *line,
                                col: *col,
                            })
                        }
                    }
                }
                Op::Not(line, col) => {
                    let v = stack.pop().unwrap();
                    let b = self.eval.expect_bool(v, span(*line, *col))?;
                    stack.push(Val::Bool(!b));
                }
                Op::MakeList(n) => {
                    let items = pop_n(&mut stack, *n);
                    stack.push(Val::List(Rc::new(RefCell::new(items))));
                }
                Op::MakeMap(n) => {
                    let flat = pop_n(&mut stack, *n * 2);
                    #[allow(clippy::mutable_key_type)]
                    let mut map = crate::val::OrderedMap::new();
                    let mut it = flat.into_iter();
                    while let (Some(k), Some(v)) = (it.next(), it.next()) {
                        map.insert(k, v);
                    }
                    stack.push(Val::Map(Rc::new(RefCell::new(map))));
                }
                Op::MakeRecord(name, fields) => {
                    let vals = pop_n(&mut stack, fields.len());
                    let mut map = HashMap::new();
                    for (k, v) in fields.iter().zip(vals) {
                        map.insert(k.clone(), v);
                    }
                    stack.push(Val::Record(name.clone(), Rc::new(RefCell::new(map))));
                }
                Op::WrapOk => {
                    let v = stack.pop().unwrap();
                    stack.push(Val::Ok(Box::new(v)));
                }
                Op::WrapErr => {
                    let v = stack.pop().unwrap();
                    match v {
                        Val::Str(s) => stack.push(Val::Err(s)),
                        other => stack.push(Val::Err(other.to_string())),
                    }
                }
                Op::WrapSome => {
                    let v = stack.pop().unwrap();
                    stack.push(Val::Some(Box::new(v)));
                }
                Op::ConcatStr(n) => {
                    let parts = pop_n(&mut stack, *n);
                    let mut out = String::new();
                    for p in parts {
                        out.push_str(&p.to_string());
                    }
                    stack.push(Val::Str(out));
                }
                Op::Dup => {
                    let v = stack.last().cloned().unwrap();
                    stack.push(v);
                }
                Op::Dup2 => {
                    let n = stack.len();
                    let (a, b) = (stack[n - 2].clone(), stack[n - 1].clone());
                    stack.push(a);
                    stack.push(b);
                }
                // Containers are reference values, so setting a field or slot
                // mutates the object every alias sees (SPEC §5.4).
                Op::SetField(f, line, col) => {
                    let val = stack.pop().unwrap();
                    let obj = stack.pop().unwrap();
                    self.eval.field_set(obj, f, val, *line, *col)?;
                }
                Op::SetIndex(line, col) => {
                    let val = stack.pop().unwrap();
                    let idx = stack.pop().unwrap();
                    let obj = stack.pop().unwrap();
                    self.eval.index_set(obj, idx, val, *line, *col)?;
                }
                Op::PushScope => {
                    let child = Scope::new(Some(cur_scope(&scopes).clone()));
                    scopes.push(Rc::new(RefCell::new(child)));
                }
                Op::PopScope => {
                    scopes.pop();
                }
                Op::TruncScopes(n) => {
                    scopes.truncate(n + 1);
                }
                // Optional narrowing (SPEC §6.4): rebind the name to the value
                // inside its `some(...)`, so the branch sees a plain `T`.
                Op::NarrowOption(name) => {
                    let inner = match cur_scope(&scopes).borrow().get(name) {
                        Some(Val::Some(inner)) => Some(*inner),
                        _ => None,
                    };
                    if let Some(v) = inner {
                        cur_scope(&scopes)
                            .borrow_mut()
                            .define(name.clone(), v, false);
                    }
                }
                Op::MakeClosure(idx) => {
                    let def = &chunk.closures[*idx];
                    stack.push(Val::Fn(
                        def.params.clone(),
                        def.body.clone(),
                        cur_scope(&scopes).clone(),
                    ));
                }
                Op::Field(f, line, col) => {
                    let obj = stack.pop().unwrap();
                    stack.push(self.eval.field_get(obj, f, *line, *col)?);
                }
                Op::Index(line, col) => {
                    let idx = stack.pop().unwrap();
                    let obj = stack.pop().unwrap();
                    stack.push(self.eval.index_get(obj, idx, *line, *col)?);
                }
                Op::Sqrt => {
                    let v = stack.pop().unwrap();
                    match v {
                        Val::Float(f) => stack.push(Val::Float(f.sqrt())),
                        other => stack.push(Val::Enum("sqrt".into(), vec![other])),
                    }
                }
                Op::CallUser(idx, argc, line, col) => {
                    let args = pop_n(&mut stack, *argc);
                    let callee = &program.functions[*idx];
                    if callee.params.len() != args.len() {
                        return Err(Diag {
                            code: "E0109",
                            msg: format!(
                                "expected {} args, got {}",
                                callee.params.len(),
                                args.len()
                            ),
                            line: *line,
                            col: *col,
                        });
                    }
                    // VM frames are Rust frames, so unbounded Heh recursion
                    // would abort on a native stack overflow. Fault instead.
                    if self.eval.call_depth >= crate::eval::MAX_CALL_DEPTH {
                        return Err(Diag {
                            code: "E0202",
                            msg: format!(
                                "call stack too deep (limit {}) — is this recursion unbounded?",
                                crate::eval::MAX_CALL_DEPTH
                            ),
                            line: *line,
                            col: *col,
                        });
                    }
                    let call_scope =
                        Rc::new(RefCell::new(Scope::new(Some(self.eval.global.clone()))));
                    for (p, a) in callee.params.iter().zip(args) {
                        call_scope.borrow_mut().define(p.clone(), a, false);
                    }
                    self.eval.call_depth += 1;
                    let result = self.exec_chunk(callee, program, call_scope);
                    self.eval.call_depth -= 1;
                    let ret = match result {
                        Ok(v) => v,
                        Err(d) if d.code == "E_TRY_PROPAGATE" => Val::Err(d.msg),
                        Err(d) => return Err(d),
                    };
                    stack.push(ret);
                }
                Op::CallValue(argc, named, line, col) => {
                    let args = pop_n(&mut stack, *argc);
                    let callee = stack.pop().unwrap();
                    stack.push(
                        self.eval
                            .apply_callee(callee, args, named.clone(), *line, *col)?,
                    );
                }
                Op::Try(else_exit, line, col) => {
                    let v = stack.pop().unwrap();
                    match v {
                        Val::Ok(inner) => stack.push(*inner),
                        Val::Some(inner) => stack.push(*inner),
                        Val::None => {
                            if *else_exit {
                                eprintln!("fault: none");
                                std::process::exit(1);
                            }
                            return Err(Diag {
                                code: "E_TRY_PROPAGATE_NONE",
                                msg: "none".into(),
                                line: *line,
                                col: *col,
                            });
                        }
                        Val::Err(e) => {
                            if *else_exit {
                                eprintln!("fault: {}", e);
                                std::process::exit(1);
                            }
                            return Err(Diag {
                                code: "E_TRY_PROPAGATE",
                                msg: e,
                                line: *line,
                                col: *col,
                            });
                        }
                        _ => {
                            return Err(Diag {
                                code: "E0112",
                                msg: "try on non-result".into(),
                                line: *line,
                                col: *col,
                            })
                        }
                    }
                }
                Op::Return => return Ok(stack.pop().unwrap_or(Val::None)),
                Op::Jump(t) => {
                    ip = *t;
                    continue;
                }
                Op::JumpIfFalse(t, line, col) => {
                    let v = stack.pop().unwrap();
                    if !self.eval.expect_bool(v, span(*line, *col))? {
                        ip = *t;
                        continue;
                    }
                }
                Op::TestBoolJumpFalse(t, line, col) => {
                    let v = stack.pop().unwrap();
                    if !self.eval.expect_bool(v, span(*line, *col))? {
                        ip = *t;
                        continue;
                    }
                }
                Op::TestBoolJumpTrue(t, line, col) => {
                    let v = stack.pop().unwrap();
                    if self.eval.expect_bool(v, span(*line, *col))? {
                        ip = *t;
                        continue;
                    }
                }
                Op::ToBool(line, col) => {
                    let v = stack.pop().unwrap();
                    let b = self.eval.expect_bool(v, span(*line, *col))?;
                    stack.push(Val::Bool(b));
                }
                Op::ForStart(line, col) => {
                    let iterable = stack.pop().unwrap();
                    match iterable {
                        Val::Range(start, end, inclusive) => {
                            let end = if matches!(*end, Val::None) {
                                None
                            } else {
                                Some(*end)
                            };
                            iters.push(IterState::Range {
                                next: *start,
                                end,
                                inclusive,
                            });
                        }
                        Val::List(l) => iters.push(IterState::List {
                            items: l.borrow().clone(),
                            idx: 0,
                        }),
                        Val::Map(m) => iters.push(IterState::List {
                            items: m.borrow().keys().cloned().collect(),
                            idx: 0,
                        }),
                        Val::Str(s) => iters.push(IterState::List {
                            items: s.chars().map(|c| Val::Str(c.to_string())).collect(),
                            idx: 0,
                        }),
                        _ => {
                            return Err(Diag {
                                code: "E0104",
                                msg: "not iterable (expected a list, map, str, or range)".into(),
                                line: *line,
                                col: *col,
                            })
                        }
                    }
                }
                Op::ForNext(var, end_addr) => {
                    let it = iters.last_mut().unwrap();
                    let bound_val = match it {
                        IterState::Range {
                            next,
                            end,
                            inclusive,
                        } => {
                            let in_bound = match end {
                                None => true,
                                Some(e) => {
                                    if *inclusive {
                                        (*next).partial_cmp(&*e) != Some(Ordering::Greater)
                                    } else {
                                        (*next).partial_cmp(&*e) == Some(Ordering::Less)
                                    }
                                }
                            };
                            if in_bound {
                                let cur = next.clone();
                                let inc = self.eval.eval_binop(
                                    BinOp::Add,
                                    next.clone(),
                                    Val::Int(BigInt::from_i64(1)),
                                    span(0, 0),
                                )?;
                                *next = inc;
                                Some(cur)
                            } else {
                                None
                            }
                        }
                        IterState::List { items, idx } => {
                            if *idx < items.len() {
                                let cur = items[*idx].clone();
                                *idx += 1;
                                Some(cur)
                            } else {
                                None
                            }
                        }
                    };
                    match bound_val {
                        Some(v) => cur_scope(&scopes)
                            .borrow_mut()
                            .define(var.clone(), v, false),
                        None => {
                            iters.pop();
                            ip = *end_addr;
                            continue;
                        }
                    }
                }
                Op::PopIter => {
                    iters.pop();
                }
                Op::MatchArm(pattern, next_arm) => {
                    let scrutinee = stack.last().unwrap().clone();
                    match self.eval.match_pattern(&scrutinee, pattern) {
                        Some(bindings) => {
                            for (k, v) in bindings {
                                cur_scope(&scopes).borrow_mut().define(k, v, false);
                            }
                        }
                        None => {
                            ip = *next_arm;
                            continue;
                        }
                    }
                }
                Op::PopScrutinee => {
                    stack.pop();
                }
                Op::MatchFail(line, col) => {
                    return Err(Diag {
                        code: "E0020",
                        msg: "non-exhaustive match".into(),
                        line: *line,
                        col: *col,
                    });
                }
            }
            ip += 1;
        }
        // Fell off the end (top-level chunk) — value is whatever is on top.
        Ok(stack.pop().unwrap_or(Val::None))
    }
}

fn span(line: u32, col: u32) -> Span {
    Span { line, col }
}

fn assign_err(name: &str, is_mut: bool, line: u32, col: u32) -> Diag {
    Diag {
        code: if !is_mut { "E0103" } else { "E0010" },
        msg: format!("cannot reassign variable '{}'", name),
        line,
        col,
    }
}

/// Pop `n` values, returning them in push (source) order.
fn pop_n(stack: &mut Vec<Val>, n: usize) -> Vec<Val> {
    let at = stack.len() - n;
    stack.split_off(at)
}

fn const_to_val(c: &Const) -> Val {
    match c {
        Const::Int(s) => Val::Int(BigInt::parse(s).unwrap()),
        Const::Float(s) => Val::Float(s.parse().unwrap()),
        Const::Str(s) => Val::Str(s.clone()),
        Const::Bool(b) => Val::Bool(*b),
        Const::None => Val::None,
    }
}

/// The innermost open block scope.
fn cur_scope(scopes: &[Rc<RefCell<Scope>>]) -> &Rc<RefCell<Scope>> {
    scopes.last().expect("a chunk always has its own scope")
}
