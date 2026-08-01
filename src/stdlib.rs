use crate::bignum::BigInt;
use crate::val::Val;
use std::cell::RefCell;
use std::rc::Rc;

pub fn eval_builtin(name: &str, mut args: Vec<Val>) -> Result<Val, String> {
    match name {
        "len" => {
            if args.len() != 1 { return Err("len expects 1 arg".into()); }
            match &args[0] {
                Val::Str(s) => Ok(Val::Int(BigInt::from_i64(s.chars().count() as i64))),
                Val::List(l) => Ok(Val::Int(BigInt::from_i64(l.borrow().len() as i64))),
                Val::Map(m) => Ok(Val::Int(BigInt::from_i64(m.borrow().len() as i64))),
                _ => Err("len expects str, list, or map".into())
            }
        }
        "upper" => {
            if args.len() != 1 { return Err("upper expects 1 arg".into()); }
            if let Val::Str(s) = &args[0] {
                Ok(Val::Str(s.to_uppercase()))
            } else {
                Err("upper expects str".into())
            }
        }
        "lower" => {
            if args.len() != 1 { return Err("lower expects 1 arg".into()); }
            if let Val::Str(s) = &args[0] {
                Ok(Val::Str(s.to_lowercase()))
            } else {
                Err("lower expects str".into())
            }
        }
        "trim" => {
            if args.len() != 1 { return Err("trim expects 1 arg".into()); }
            if let Val::Str(s) = &args[0] {
                Ok(Val::Str(s.trim().to_string()))
            } else {
                Err("trim expects str".into())
            }
        }
        "split" => {
            if args.len() != 2 { return Err("split expects 2 args".into()); }
            if let (Val::Str(s), Val::Str(sep)) = (&args[0], &args[1]) {
                let parts: Vec<Val> = s.split(sep).map(|part| Val::Str(part.to_string())).collect();
                Ok(Val::List(Rc::new(RefCell::new(parts))))
            } else {
                Err("split expects str args".into())
            }
        }
        "replace" => {
            if args.len() != 3 { return Err("replace expects 3 args".into()); }
            if let (Val::Str(s), Val::Str(old), Val::Str(new_s)) = (&args[0], &args[1], &args[2]) {
                Ok(Val::Str(s.replace(old, new_s)))
            } else {
                Err("replace expects str args".into())
            }
        }
        "contains" => {
            if args.len() != 2 { return Err("contains expects 2 args".into()); }
            if let (Val::Str(s), Val::Str(sub)) = (&args[0], &args[1]) {
                Ok(Val::Bool(s.contains(sub)))
            } else {
                Err("contains expects str args".into())
            }
        }
        "starts_with" => {
            if args.len() != 2 { return Err("starts_with expects 2 args".into()); }
            if let (Val::Str(s), Val::Str(prefix)) = (&args[0], &args[1]) {
                Ok(Val::Bool(s.starts_with(prefix)))
            } else {
                Err("starts_with expects str args".into())
            }
        }
        "chars" => {
            if args.len() != 1 { return Err("chars expects 1 arg".into()); }
            if let Val::Str(s) = &args[0] {
                let chars: Vec<Val> = s.chars().map(|c| Val::Str(c.to_string())).collect();
                Ok(Val::List(Rc::new(RefCell::new(chars))))
            } else {
                Err("chars expects str".into())
            }
        }

        "push" => {
            if args.len() != 2 { return Err("push expects 2 args".into()); }
            let val = args.pop().unwrap();
            if let Val::List(l) = &args[0] {
                l.borrow_mut().push(val);
                Ok(Val::None)
            } else {
                Err("push expects list".into())
            }
        }
        "pop" => {
            if args.len() != 1 { return Err("pop expects 1 arg".into()); }
            if let Val::List(l) = &args[0] {
                if let Some(v) = l.borrow_mut().pop() {
                    Ok(Val::Ok(Box::new(v)))
                } else {
                    Ok(Val::Err("pop from empty list".into()))
                }
            } else {
                Err("pop expects list".into())
            }
        }
        // The non-faulting lookups: both return `T?` (SPEC §7.3), so a missing
        // index or key is `none` rather than a fault.
        "get" => {
            if args.len() != 2 { return Err("get expects 2 args".into()); }
            match (&args[0], &args[1]) {
                (Val::List(l), Val::Int(i)) => {
                    let b = l.borrow();
                    match i.to_usize() {
                        Some(idx) if idx < b.len() => Ok(Val::Some(Box::new(b[idx].clone()))),
                        _ => Ok(Val::None),
                    }
                }
                (Val::Map(m), k) => match m.borrow().get(k) {
                    Some(v) => Ok(Val::Some(Box::new(v.clone()))),
                    None => Ok(Val::None),
                },
                _ => Err("get expects list/int or map/any".into())
            }
        }
        "join" => {
            if args.len() != 2 { return Err("join expects 2 args".into()); }
            if let (Val::List(l), Val::Str(sep)) = (&args[0], &args[1]) {
                let b = l.borrow();
                let mut outs = Vec::new();
                for item in b.iter() {
                    outs.push(item.to_string());
                }
                Ok(Val::Str(outs.join(sep)))
            } else {
                Err("join expects list and str".into())
            }
        }
        
        "set" => {
            if args.len() != 3 { return Err("set expects 3 args".into()); }
            let val = args.pop().unwrap();
            let key = args.pop().unwrap();
            if let Val::Map(m) = &args[0] {
                m.borrow_mut().insert(key, val);
                Ok(Val::None)
            } else {
                Err("set expects map".into())
            }
        }
        "remove" => {
            if args.len() != 2 { return Err("remove expects 2 args".into()); }
            if let Val::Map(m) = &args[0] {
                m.borrow_mut().remove(&args[1]);
                Ok(Val::None)
            } else {
                Err("remove expects map".into())
            }
        }
        "keys" => {
            if args.len() != 1 { return Err("keys expects 1 arg".into()); }
            if let Val::Map(m) = &args[0] {
                let keys: Vec<Val> = m.borrow().keys().cloned().collect();
                Ok(Val::List(Rc::new(RefCell::new(keys))))
            } else {
                Err("keys expects map".into())
            }
        }
        "values" => {
            if args.len() != 1 { return Err("values expects 1 arg".into()); }
            if let Val::Map(m) = &args[0] {
                let vals: Vec<Val> = m.borrow().values().cloned().collect();
                Ok(Val::List(Rc::new(RefCell::new(vals))))
            } else {
                Err("values expects map".into())
            }
        }
        
        "str" => {
            if args.len() != 1 { return Err("str expects 1 arg".into()); }
            Ok(Val::Str(args[0].to_string()))
        }
        // Explicit numeric conversion (SPEC §5.2) — int and float never mix
        // implicitly, so these are the only bridge between them.
        "int" => {
            if args.len() != 1 { return Err("int expects 1 arg".into()); }
            match &args[0] {
                Val::Int(i) => Ok(Val::Int(i.clone())),
                Val::Float(f) => {
                    if f.is_nan() || f.is_infinite() {
                        return Err(format!("int({}) has no integer value", f));
                    }
                    Ok(Val::Int(BigInt::from_f64_trunc(*f)))
                }
                _ => Err("int expects int or float".into()),
            }
        }
        "float" => {
            if args.len() != 1 { return Err("float expects 1 arg".into()); }
            match &args[0] {
                Val::Float(f) => Ok(Val::Float(*f)),
                Val::Int(i) => Ok(Val::Float(i.to_f64())),
                _ => Err("float expects int or float".into()),
            }
        }
        // `list(0..10)` materializes a finite range (SPEC §5.5); an unbounded
        // range would never terminate, so it is rejected rather than hung on.
        "list" => {
            if args.len() != 1 { return Err("list expects 1 arg".into()); }
            match &args[0] {
                Val::List(l) => Ok(Val::List(Rc::new(RefCell::new(l.borrow().clone())))),
                Val::Str(s) => {
                    let chars: Vec<Val> = s.chars().map(|c| Val::Str(c.to_string())).collect();
                    Ok(Val::List(Rc::new(RefCell::new(chars))))
                }
                Val::Map(m) => {
                    let keys: Vec<Val> = m.borrow().keys().cloned().collect();
                    Ok(Val::List(Rc::new(RefCell::new(keys))))
                }
                Val::Range(start, end, inclusive) => {
                    let (Val::Int(from), Val::Int(to)) = (start.as_ref(), end.as_ref()) else {
                        return Err("list() needs a bounded range of ints".into());
                    };
                    let one = BigInt::from_u64(1);
                    let mut items = Vec::new();
                    let mut cur = from.clone();
                    while cur < *to || (*inclusive && cur == *to) {
                        items.push(Val::Int(cur.clone()));
                        cur = &cur + &one;
                    }
                    Ok(Val::List(Rc::new(RefCell::new(items))))
                }
                _ => Err("list expects a range, str, map, or list".into()),
            }
        }
        "int_of" => {
            if args.len() != 1 { return Err("int_of expects 1 arg".into()); }
            if let Val::Str(s) = &args[0] {
                if let Some(i) = BigInt::parse(s) {
                    Ok(Val::Ok(Box::new(Val::Int(i))))
                } else {
                    Ok(Val::Err(format!("not an integer: \"{}\"", s)))
                }
            } else {
                Ok(Val::Err("not an integer".into()))
            }
        }

        _ => {
            if let Some(res) = crate::modules::eval(name, args) {
                return res;
            }
            Err(format!("unknown builtin '{}'", name))
        }
    }
}
