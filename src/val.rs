use crate::bignum::BigInt;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// A `map[K, V]` — insertion-ordered, as SPEC §5.4 requires. A plain HashMap
/// would iterate in a per-process random order, which makes otherwise
/// deterministic programs print differently on every run.
///
/// Entries live in a Vec (the order of record) with a HashMap from key to slot
/// for O(1) lookup. Removal is O(n) because it has to close the gap; that is
/// the rare operation and it keeps the structure free of tombstones.
#[derive(Debug, Clone, Default)]
pub struct OrderedMap {
    entries: Vec<(Val, Val)>,
    index: crate::fasthash::FastMap<Val, usize>,
}

impl OrderedMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, key: &Val) -> Option<&Val> {
        self.index.get(key).map(|&i| &self.entries[i].1)
    }

    /// Insert or overwrite. Overwriting keeps the key's original position, the
    /// same way Python dicts do.
    pub fn insert(&mut self, key: Val, val: Val) {
        match self.index.get(&key) {
            Some(&i) => self.entries[i].1 = val,
            None => {
                self.index.insert(key.clone(), self.entries.len());
                self.entries.push((key, val));
            }
        }
    }

    pub fn remove(&mut self, key: &Val) -> Option<Val> {
        let i = self.index.remove(key)?;
        let (_, val) = self.entries.remove(i);
        for slot in self.index.values_mut() {
            if *slot > i {
                *slot -= 1;
            }
        }
        Some(val)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Val, &Val)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &Val> {
        self.entries.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &Val> {
        self.entries.iter().map(|(_, v)| v)
    }
}

impl PartialEq for OrderedMap {
    /// Maps compare by content, not by insertion order — `{"a": 1, "b": 2}`
    /// equals `{"b": 2, "a": 1}` even though they print differently.
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}

impl FromIterator<(Val, Val)> for OrderedMap {
    fn from_iter<I: IntoIterator<Item = (Val, Val)>>(iter: I) -> Self {
        let mut map = Self::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

#[derive(Debug, Clone)]
pub enum Val {
    Int(BigInt),
    Float(f64),
    Bool(bool),
    Str(String),
    Range(Box<Val>, Box<Val>, bool), // (start, end, is_inclusive)
    // Parameter names are refcounted: binding them is per-call work.
    Fn(
        Vec<Rc<str>>,
        crate::ast::Block,
        std::rc::Rc<std::cell::RefCell<crate::eval::Scope>>,
    ),
    BuiltinFn(&'static str),
    Ok(Box<Val>),
    Err(String),
    Some(Box<Val>),
    List(Rc<RefCell<Vec<Val>>>),
    Map(Rc<RefCell<OrderedMap>>),
    Record(String, Rc<RefCell<HashMap<String, Val>>>),
    Enum(String, Vec<Val>),
    BoundMethod(Box<Val>, String),
    None,
}

impl Val {
    /// The value's type as it appears in diagnostics.
    pub fn type_name(&self) -> &'static str {
        match self {
            Val::Int(_) => "int",
            Val::Float(_) => "float",
            Val::Bool(_) => "bool",
            Val::Str(_) => "str",
            Val::Range(..) => "range",
            Val::Fn(..) | Val::BuiltinFn(_) | Val::BoundMethod(..) => "fn",
            Val::Ok(_) | Val::Err(_) => "result",
            Val::Some(_) | Val::None => "option",
            Val::List(_) => "list",
            Val::Map(_) => "map",
            Val::Record(..) => "record",
            Val::Enum(..) => "enum",
        }
    }
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
            (Val::BoundMethod(oa, ma), Val::BoundMethod(ob, mb)) => oa == ob && ma == mb,
            (Val::Ok(a), Val::Ok(b)) => a == b,
            (Val::Err(a), Val::Err(b)) => a == b,
            (Val::Some(a), Val::Some(b)) => a == b,
            (Val::List(a), Val::List(b)) => *a.borrow() == *b.borrow(),
            (Val::Map(a), Val::Map(b)) => *a.borrow() == *b.borrow(),
            (Val::Record(n1, a), Val::Record(n2, b)) => n1 == n2 && *a.borrow() == *b.borrow(),
            (Val::Enum(n1, b1), Val::Enum(n2, b2)) => n1 == n2 && b1 == b2,
            (Val::None, Val::None) => true,
            _ => false,
        }
    }
}

impl Eq for Val {}

impl Hash for Val {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Val::Int(i) => {
                0u8.hash(state);
                i.hash(state);
            }
            Val::BuiltinFn(name) => {
                7.hash(state);
                name.hash(state);
            }
            Val::BoundMethod(obj, method) => {
                8.hash(state);
                obj.hash(state);
                method.hash(state);
            }
            Val::Bool(b) => {
                1u8.hash(state);
                b.hash(state);
            }
            Val::Str(s) => {
                2u8.hash(state);
                s.hash(state);
            }
            Val::Float(f) => {
                3u8.hash(state);
                f.to_bits().hash(state);
            }
            Val::None => {
                4u8.hash(state);
            }
            Val::List(l) => {
                5u8.hash(state);
                Rc::as_ptr(l).hash(state);
            }
            Val::Map(m) => {
                6u8.hash(state);
                Rc::as_ptr(m).hash(state);
            }
            Val::Record(n, r) => {
                7u8.hash(state);
                n.hash(state);
                Rc::as_ptr(r).hash(state);
            }
            Val::Enum(n, v) => {
                8u8.hash(state);
                n.hash(state);
                v.hash(state);
            }
            Val::Ok(inner) => {
                5.hash(state);
                inner.hash(state);
            }
            Val::Some(inner) => {
                15.hash(state);
                inner.hash(state);
            }
            Val::Err(e) => {
                10u8.hash(state);
                e.hash(state);
            }
            _ => {
                255u8.hash(state);
            }
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

/// Escape a string being printed inside quotes, so `err("say \"hi\"")` cannot
/// be confused for a shorter string followed by junk.
fn escape_in_quotes(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Val::Int(i) => write!(f, "{}", i),
            // A float always shows a decimal point (SPEC §5.2), so printed
            // output distinguishes `3.0` from the int `3`. Never exponent
            // notation: plain decimal is exact and needs no threshold rule.
            Val::Float(fl) => {
                if fl.is_nan() {
                    write!(f, "nan")
                } else if fl.is_finite() && fl.fract() == 0.0 {
                    write!(f, "{:.1}", fl)
                } else {
                    write!(f, "{}", fl)
                }
            }
            Val::Bool(b) => write!(f, "{}", b),
            Val::Str(s) => write!(f, "{}", s),
            Val::Range(start, end, inc) => {
                if *inc {
                    write!(f, "{}..={}", start, end)
                } else {
                    write!(f, "{}..{}", start, end)
                }
            }
            Val::Fn(..) => write!(f, "<fn>"),
            Val::BuiltinFn(n) => write!(f, "<builtin {}>", n),
            Val::Ok(inner) => write!(f, "ok({})", inner),
            Val::Err(e) => write!(f, "err(\"{}\")", escape_in_quotes(e)),
            Val::Some(inner) => write!(f, "some({})", inner),
            Val::List(l) => {
                write!(f, "[")?;
                let b = l.borrow();
                for (i, v) in b.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    // In real Heh, lists print string elements with quotes, but we'll keep it simple for now
                    if let Val::Str(s) = v {
                        write!(f, "\"{}\"", escape_in_quotes(s))?;
                    } else {
                        write!(f, "{}", v)?;
                    }
                }
                write!(f, "]")
            }
            Val::Map(m) => {
                write!(f, "{{")?;
                let b = m.borrow();
                let mut first = true;
                for (k, v) in b.iter() {
                    if !first {
                        write!(f, ", ")?;
                    }
                    first = false;
                    if let Val::Str(s) = k {
                        write!(f, "\"{}\": ", s)?;
                    } else {
                        write!(f, "{}: ", k)?;
                    }
                    if let Val::Str(s) = v {
                        write!(f, "\"{}\"", escape_in_quotes(s))?;
                    } else {
                        write!(f, "{}", v)?;
                    }
                }
                write!(f, "}}")
            }
            Val::Record(n, r) => {
                write!(f, "{} {{", n)?;
                let b = r.borrow();
                let mut first = true;
                for (k, v) in b.iter() {
                    if !first {
                        write!(f, ", ")?;
                    }
                    first = false;
                    write!(f, "{}: ", k)?;
                    if let Val::Str(s) = v {
                        write!(f, "\"{}\"", escape_in_quotes(s))?;
                    } else {
                        write!(f, "{}", v)?;
                    }
                }
                write!(f, "}}")
            }
            Val::Enum(n, args) => {
                if args.is_empty() {
                    write!(f, "{}", n)
                } else {
                    let mut s = format!("{}(", n);
                    for (i, v) in args.iter().enumerate() {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        s.push_str(&v.to_string());
                    }
                    s.push(')');
                    write!(f, "{}", s)
                }
            }
            Val::BoundMethod(obj, m) => write!(f, "<bound method {}.{}>", obj.to_string(), m),
            Val::None => write!(f, "none"),
        }
    }
}
