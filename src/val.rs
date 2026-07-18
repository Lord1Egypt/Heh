use std::fmt;
use std::cmp::Ordering;
use std::rc::Rc;
use std::cell::RefCell;
use crate::bignum::BigInt;

#[derive(Debug, Clone)]
pub enum Val {
    Int(BigInt),
    Float(f64),
    Bool(bool),
    Str(String),
    Range(Box<Val>, Box<Val>, bool), // (start, end, is_inclusive)
    Fn(Vec<String>, crate::ast::Block, std::rc::Rc<std::cell::RefCell<crate::eval::Scope>>),
    BuiltinFn(&'static str),
    Ok(Box<Val>),
    Err(String),
    List(Rc<RefCell<Vec<Val>>>),
    None,
}

impl PartialEq for Val {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Val::Int(a), Val::Int(b)) => a == b,
            (Val::Float(a), Val::Float(b)) => {
                if a.is_nan() && b.is_nan() {
                    false // IEEE 754: NaN != NaN
                } else {
                    a == b
                }
            }
            (Val::Bool(a), Val::Bool(b)) => a == b,
            (Val::Str(a), Val::Str(b)) => a == b,
            (Val::Range(a1, b1, i1), Val::Range(a2, b2, i2)) => a1 == a2 && b1 == b2 && i1 == i2,
            (Val::BuiltinFn(a), Val::BuiltinFn(b)) => a == b,
            (Val::Ok(a), Val::Ok(b)) => a == b,
            (Val::Err(a), Val::Err(b)) => a == b,
            (Val::List(a), Val::List(b)) => *a.borrow() == *b.borrow(),
            (Val::None, Val::None) => true,
            _ => false,
        }
    }
}

impl PartialOrd for Val {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Val::Int(a), Val::Int(b)) => Some(a.cmp(b)),
            (Val::Float(a), Val::Float(b)) => a.partial_cmp(b),
            (Val::Bool(a), Val::Bool(b)) => Some(a.cmp(b)),
            (Val::Str(a), Val::Str(b)) => Some(a.cmp(b)),
            (Val::None, Val::None) => Some(Ordering::Equal),
            _ => None, // cannot compare mixed types (except maybe int and float later?)
        }
    }
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Val::Int(i) => write!(f, "{}", i),
            Val::Float(fl) => write!(f, "{}", fl),
            Val::Bool(b) => write!(f, "{}", b),
            Val::Str(s) => write!(f, "{}", s),
            Val::Range(start, end, inc) => {
                if *inc { write!(f, "{}..={}", start, end) }
                else { write!(f, "{}..{}", start, end) }
            }
            Val::Fn(..) => write!(f, "<fn>"),
            Val::BuiltinFn(n) => write!(f, "<builtin {}>", n),
            Val::Ok(v) => write!(f, "ok({})", v),
            Val::Err(e) => write!(f, "err(\"{}\")", e),
            Val::List(l) => {
                write!(f, "[")?;
                let b = l.borrow();
                for (i, v) in b.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    // In real Heh, lists print string elements with quotes, but we'll keep it simple for now
                    if let Val::Str(s) = v { write!(f, "\"{}\"", s)?; }
                    else { write!(f, "{}", v)?; }
                }
                write!(f, "]")
            }
            Val::None => write!(f, "none"),
        }
    }
}
