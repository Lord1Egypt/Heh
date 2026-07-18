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
        "get" => {
            if args.len() != 2 { return Err("get expects 2 args".into()); }
            match (&args[0], &args[1]) {
                (Val::List(l), Val::Int(i)) => {
                    let b = l.borrow();
                    if i.sign { return Ok(Val::Err("negative index".into())); }
                    let idx = i.limbs[0] as usize;
                    if idx < b.len() {
                        Ok(Val::Ok(Box::new(b[idx].clone())))
                    } else {
                        Ok(Val::Err("index out of bounds".into()))
                    }
                }
                (Val::Map(m), k) => {
                    if let Some(v) = m.borrow().get(k) {
                        Ok(Val::Ok(Box::new(v.clone())))
                    } else {
                        Ok(Val::Err("key not found".into()))
                    }
                }
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

        _ => Err(format!("unknown builtin '{}'", name))
    }
}
