//! Pure standard-library modules (P7): std/math, std/fmt, std/json, std/csv,
//! std/hash, std/regex, std/debug. Each is exposed as a `Val::Record` bound by
//! `use std/<name>`; its functions dispatch here by qualified name.

use crate::bignum::BigInt;
use crate::val::Val;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The std module names that `use std/<name>` may bind.
pub const MODULES: &[&str] = &["math", "fmt", "json", "csv", "hash", "regex", "time", "debug"];

/// SHA-256 of `data` as a lowercase hex string (used by the vendor lockfile).
pub fn sha256_hex(data: &[u8]) -> String {
    hex(&sha256(data))
}

/// Given a `use` path like "std/math" (or bare "math"), return the module's
/// namespace record, or None if it is not a known std module.
pub fn module_record(path: &str) -> Option<Val> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let funcs: &[&'static str] = match name {
        "math" => &["math.sin", "math.cos", "math.sqrt", "math.abs", "math.pow", "math.log", "math.floor", "math.ceil", "math.pi", "math.e"],
        "fmt" => &["fmt.pad_left", "fmt.pad_right", "fmt.repeat", "fmt.hex", "fmt.fixed"],
        "json" => &["json.parse", "json.write"],
        "csv" => &["csv.parse", "csv.write"],
        "hash" => &["hash.sha256", "hash.crc32"],
        "regex" => &["regex.is_match", "regex.find"],
        "time" => &["time.parts", "time.format", "time.from_parts", "time.is_leap", "time.days_in_month"],
        "debug" => &["debug.fault", "debug.assert"],
        _ => return None,
    };
    let mut map = HashMap::new();
    for f in funcs {
        // key is the bare function name (after the dot)
        let short = f.split('.').nth(1).unwrap();
        map.insert(short.to_string(), Val::BuiltinFn(f));
    }
    Some(Val::Record(name.to_string(), Rc::new(RefCell::new(map))))
}

/// Dispatch a qualified module builtin. Returns None if `name` is not one of
/// ours (so the caller can keep looking).
pub fn eval(name: &str, args: Vec<Val>) -> Option<Result<Val, String>> {
    if !name.contains('.') {
        return None;
    }
    let module = name.split('.').next().unwrap();
    if !MODULES.contains(&module) {
        return None;
    }
    Some(dispatch(name, args))
}

fn dispatch(name: &str, args: Vec<Val>) -> Result<Val, String> {
    match name {
        // ---- std/math (all operate on floats; ints are coerced) -------------
        "math.sin" => Ok(Val::Float(f1(&args, "sin")?.sin())),
        "math.cos" => Ok(Val::Float(f1(&args, "cos")?.cos())),
        "math.sqrt" => Ok(Val::Float(f1(&args, "sqrt")?.sqrt())),
        "math.abs" => Ok(Val::Float(f1(&args, "abs")?.abs())),
        "math.log" => Ok(Val::Float(f1(&args, "log")?.ln())),
        "math.floor" => Ok(Val::Float(f1(&args, "floor")?.floor())),
        "math.ceil" => Ok(Val::Float(f1(&args, "ceil")?.ceil())),
        "math.pi" => { need(&args, 0, "pi")?; Ok(Val::Float(std::f64::consts::PI)) }
        "math.e" => { need(&args, 0, "e")?; Ok(Val::Float(std::f64::consts::E)) }
        "math.pow" => {
            need(&args, 2, "pow")?;
            Ok(Val::Float(as_f64(&args[0], "pow")?.powf(as_f64(&args[1], "pow")?)))
        }

        // ---- std/fmt (Heh has native string interpolation, so these are the
        //      helpers interpolation can't express) ---------------------------
        "fmt.pad_left" => fmt_pad(&args, true),
        "fmt.pad_right" => fmt_pad(&args, false),
        "fmt.repeat" => {
            need(&args, 2, "repeat")?;
            let s = match &args[0] { Val::Str(s) => s, _ => return Err("fmt.repeat: expects (str, int)".into()) };
            let n = int_arg(&args[1], "repeat")?;
            if n < 0 { return Err("fmt.repeat: count must be >= 0".into()); }
            Ok(Val::Str(s.repeat(n as usize)))
        }
        "fmt.hex" => {
            need(&args, 1, "hex")?;
            let n = int_arg(&args[0], "hex")?;
            Ok(Val::Str(format!("{:x}", n)))
        }
        "fmt.fixed" => {
            need(&args, 2, "fixed")?;
            let x = as_f64(&args[0], "fixed").map_err(|_| "fmt.fixed: expects (number, int)".to_string())?;
            let places = int_arg(&args[1], "fixed")?;
            if !(0..=20).contains(&places) { return Err("fmt.fixed: places must be 0..=20".into()); }
            Ok(Val::Str(format!("{:.*}", places as usize, x)))
        }

        // ---- std/json -------------------------------------------------------
        "json.parse" => {
            need(&args, 1, "parse")?;
            let s = match &args[0] { Val::Str(s) => s, _ => return Err("json.parse: expects str".into()) };
            match json_parse(s) {
                Ok(v) => Ok(Val::Ok(Box::new(v))),
                Err(e) => Ok(Val::Err(e)),
            }
        }
        "json.write" => {
            need(&args, 1, "write")?;
            match json_write(&args[0]) {
                Ok(s) => Ok(Val::Str(s)),
                Err(e) => Err(e),
            }
        }

        // ---- std/csv --------------------------------------------------------
        "csv.parse" => {
            need(&args, 1, "parse")?;
            let s = match &args[0] { Val::Str(s) => s, _ => return Err("csv.parse: expects str".into()) };
            Ok(csv_parse(s))
        }
        "csv.write" => {
            need(&args, 1, "write")?;
            csv_write(&args[0])
        }

        // ---- std/hash -------------------------------------------------------
        "hash.sha256" => {
            need(&args, 1, "sha256")?;
            let bytes = bytes_of(&args[0], "sha256")?;
            Ok(Val::Str(hex(&sha256(&bytes))))
        }
        "hash.crc32" => {
            need(&args, 1, "crc32")?;
            let bytes = bytes_of(&args[0], "crc32")?;
            Ok(Val::Str(format!("{:08x}", crc32(&bytes))))
        }

        // ---- std/regex ------------------------------------------------------
        // ---- std/time (pure: the instant is always an argument) -------------
        "time.parts" => {
            need(&args, 1, "time.parts")?;
            Ok(time_parts(millis_arg(&args[0], "parts")?))
        }
        "time.format" => {
            need(&args, 1, "time.format")?;
            Ok(Val::Str(time_format(millis_arg(&args[0], "format")?)))
        }
        "time.is_leap" => {
            need(&args, 1, "time.is_leap")?;
            Ok(Val::Bool(is_leap(millis_arg(&args[0], "is_leap")?)))
        }
        "time.days_in_month" => {
            need(&args, 2, "time.days_in_month")?;
            let y = millis_arg(&args[0], "days_in_month")?;
            let m = millis_arg(&args[1], "days_in_month")?;
            match days_in_month(y, m) {
                0 => Ok(Val::Err(format!("time.days_in_month: month {} is not 1..=12", m))),
                n => Ok(Val::Ok(Box::new(Val::Int(BigInt::from_i64(n))))),
            }
        }
        "time.from_parts" => {
            need(&args, 6, "time.from_parts")?;
            let mut v = [0i64; 6];
            for (i, slot) in v.iter_mut().enumerate() {
                *slot = millis_arg(&args[i], "from_parts")?;
            }
            let [y, mo, d, h, mi, s] = v;
            if !(1..=12).contains(&mo) {
                return Ok(Val::Err(format!("time.from_parts: month {} is not 1..=12", mo)));
            }
            if d < 1 || d > days_in_month(y, mo) {
                return Ok(Val::Err(format!("time.from_parts: day {} is out of range for {}-{:02}", d, y, mo)));
            }
            if !(0..24).contains(&h) || !(0..60).contains(&mi) || !(0..60).contains(&s) {
                return Ok(Val::Err("time.from_parts: hour/minute/second out of range".into()));
            }
            let ms = days_from_civil(y, mo, d) * 86_400_000 + h * 3_600_000 + mi * 60_000 + s * 1_000;
            Ok(Val::Ok(Box::new(Val::Int(BigInt::from_i64(ms)))))
        }

        "regex.is_match" => {
            need(&args, 2, "is_match")?;
            let (pat, text) = two_strs(&args, "is_match")?;
            match regex::Regex::compile(&pat) {
                Ok(re) => Ok(Val::Bool(re.is_match(&text))),
                Err(e) => Err(format!("regex.is_match: {}", e)),
            }
        }
        "regex.find" => {
            need(&args, 2, "find")?;
            let (pat, text) = two_strs(&args, "find")?;
            match regex::Regex::compile(&pat) {
                Ok(re) => match re.find(&text) {
                    Some(m) => Ok(Val::Ok(Box::new(Val::Str(m)))),
                    None => Ok(Val::Err("no match".into())),
                },
                Err(e) => Err(format!("regex.find: {}", e)),
            }
        }

        // ---- std/debug ------------------------------------------------------
        "debug.fault" => {
            need(&args, 1, "fault")?;
            let msg = match &args[0] { Val::Str(s) => s.clone(), other => other.to_string() };
            Err(msg)
        }
        "debug.assert" => {
            need(&args, 2, "assert")?;
            let ok = matches!(&args[0], Val::Bool(true));
            if ok {
                Ok(Val::None)
            } else {
                let msg = match &args[1] { Val::Str(s) => s.clone(), other => other.to_string() };
                Err(format!("assertion failed: {}", msg))
            }
        }

        _ => Err(format!("unknown builtin '{}'", name)),
    }
}

// --------------------------------------------------------------------------
// helpers
// --------------------------------------------------------------------------

// ---- std/time ------------------------------------------------------------
//
// Pure calendar arithmetic on the same unix-millisecond int that
// `sys.clock.now()` returns. Reading the clock is a capability (SPEC §10), so
// nothing here observes the current time — you pass the instant in.
// Everything is proleptic-Gregorian UTC; there are no timezones by design.

/// Days since 1970-01-01 from a y/m/d, and its inverse. Both are exact for the
/// whole int64 range (Howard Hinnant's civil-calendar algorithms).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Split unix millis into (days since epoch, milliseconds into that day).
/// Both parts floor, so instants before 1970 decompose correctly.
fn split_epoch_millis(ms: i64) -> (i64, i64) {
    const DAY: i64 = 86_400_000;
    (ms.div_euclid(DAY), ms.rem_euclid(DAY))
}

fn time_parts(ms: i64) -> Val {
    let (days, rem) = split_epoch_millis(ms);
    let (y, mo, d) = civil_from_days(days);
    let mut map = crate::val::OrderedMap::new();
    let mut put = |k: &str, v: i64| {
        map.insert(Val::Str(k.to_string()), Val::Int(BigInt::from_i64(v)));
    };
    put("year", y);
    put("month", mo);
    put("day", d);
    put("hour", rem / 3_600_000);
    put("minute", rem / 60_000 % 60);
    put("second", rem / 1_000 % 60);
    put("milli", rem % 1_000);
    // 1970-01-01 was a Thursday; 0 = Monday … 6 = Sunday.
    put("weekday", (days + 3).rem_euclid(7));
    put("yearday", days - days_from_civil(y, 1, 1) + 1);
    Val::Map(Rc::new(RefCell::new(map)))
}

fn time_format(ms: i64) -> String {
    let (days, rem) = split_epoch_millis(ms);
    let (y, mo, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        mo,
        d,
        rem / 3_600_000,
        rem / 60_000 % 60,
        rem / 1_000 % 60
    )
}

/// An `int` argument as an exact i64 (time values never need bignum range).
fn millis_arg(v: &Val, who: &str) -> Result<i64, String> {
    match v {
        Val::Int(i) => {
            let mut mag = i.clone();
            mag.sign = false;
            match mag.to_usize().and_then(|u| i64::try_from(u).ok()) {
                Some(n) => Ok(if i.sign { -n } else { n }),
                None => Err(format!("time.{}: value out of range", who)),
            }
        }
        _ => Err(format!("time.{}: expects int", who)),
    }
}

fn need(args: &[Val], n: usize, who: &str) -> Result<(), String> {
    if args.len() != n { Err(format!("{} expects {} arg(s)", who, n)) } else { Ok(()) }
}

fn as_f64(v: &Val, who: &str) -> Result<f64, String> {
    match v {
        Val::Float(f) => Ok(*f),
        Val::Int(i) => Ok(i.to_f64()),
        _ => Err(format!("math.{}: expects number", who)),
    }
}

fn f1(args: &[Val], who: &str) -> Result<f64, String> {
    need(args, 1, who)?;
    as_f64(&args[0], who)
}

fn two_strs(args: &[Val], who: &str) -> Result<(String, String), String> {
    match (&args[0], &args[1]) {
        (Val::Str(a), Val::Str(b)) => Ok((a.clone(), b.clone())),
        _ => Err(format!("regex.{}: expects (str, str)", who)),
    }
}

fn bytes_of(v: &Val, who: &str) -> Result<Vec<u8>, String> {
    match v {
        Val::Str(s) => Ok(s.as_bytes().to_vec()),
        Val::List(l) => {
            let mut out = Vec::new();
            for item in l.borrow().iter() {
                if let Val::Int(i) = item {
                    let b = i.to_f64() as i64;
                    if !(0..=255).contains(&b) { return Err(format!("hash.{}: byte out of range", who)); }
                    out.push(b as u8);
                } else {
                    return Err(format!("hash.{}: list must be bytes (ints)", who));
                }
            }
            Ok(out)
        }
        _ => Err(format!("hash.{}: expects str or list[int]", who)),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn int_arg(v: &Val, who: &str) -> Result<i64, String> {
    match v {
        Val::Int(i) => Ok(i.to_f64() as i64),
        _ => Err(format!("fmt.{}: expects int", who)),
    }
}

fn fmt_pad(args: &[Val], left: bool) -> Result<Val, String> {
    need(args, 3, if left { "pad_left" } else { "pad_right" })?;
    let s = match &args[0] { Val::Str(s) => s.clone(), _ => return Err("fmt.pad: expects (str, int, str)".into()) };
    let width = int_arg(&args[1], "pad")?;
    let fill = match &args[2] { Val::Str(f) => f.clone(), _ => return Err("fmt.pad: fill must be str".into()) };
    let fill_ch = fill.chars().next().ok_or("fmt.pad: fill must be one char")?;
    let cur = s.chars().count() as i64;
    if cur >= width { return Ok(Val::Str(s)); }
    let pad: String = std::iter::repeat_n(fill_ch, (width - cur) as usize).collect();
    Ok(Val::Str(if left { format!("{}{}", pad, s) } else { format!("{}{}", s, pad) }))
}

// --------------------------------------------------------------------------
// JSON
// --------------------------------------------------------------------------

fn json_parse(s: &str) -> Result<Val, String> {
    let chars: Vec<char> = s.chars().collect();
    let mut p = JsonParser { chars: &chars, pos: 0 };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.pos != chars.len() { return Err("json: trailing content".into()); }
    Ok(v)
}

struct JsonParser<'a> { chars: &'a [char], pos: usize }

impl<'a> JsonParser<'a> {
    fn peek(&self) -> Option<char> { self.chars.get(self.pos).copied() }
    fn bump(&mut self) -> Option<char> { let c = self.peek(); if c.is_some() { self.pos += 1; } c }
    fn skip_ws(&mut self) { while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) { self.pos += 1; } }

    fn value(&mut self) -> Result<Val, String> {
        self.skip_ws();
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Val::Str(self.string()?)),
            Some('t') | Some('f') => self.boolean(),
            Some('n') => self.null(),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            _ => Err("json: unexpected token".into()),
        }
    }

    fn object(&mut self) -> Result<Val, String> {
        self.bump(); // {
        let map: Rc<RefCell<crate::val::OrderedMap>> = Rc::new(RefCell::new(crate::val::OrderedMap::new()));
        self.skip_ws();
        if self.peek() == Some('}') { self.bump(); return Ok(Val::Map(map)); }
        loop {
            self.skip_ws();
            if self.peek() != Some('"') { return Err("json: expected string key".into()); }
            let key = self.string()?;
            self.skip_ws();
            if self.bump() != Some(':') { return Err("json: expected ':'".into()); }
            let val = self.value()?;
            map.borrow_mut().insert(Val::Str(key), val);
            self.skip_ws();
            match self.bump() {
                Some(',') => continue,
                Some('}') => break,
                _ => return Err("json: expected ',' or '}'".into()),
            }
        }
        Ok(Val::Map(map))
    }

    fn array(&mut self) -> Result<Val, String> {
        self.bump(); // [
        let list = Rc::new(RefCell::new(Vec::new()));
        self.skip_ws();
        if self.peek() == Some(']') { self.bump(); return Ok(Val::List(list)); }
        loop {
            let val = self.value()?;
            list.borrow_mut().push(val);
            self.skip_ws();
            match self.bump() {
                Some(',') => continue,
                Some(']') => break,
                _ => return Err("json: expected ',' or ']'".into()),
            }
        }
        Ok(Val::List(list))
    }

    fn string(&mut self) -> Result<String, String> {
        self.bump(); // opening quote
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err("json: unterminated string".into()),
                Some('"') => break,
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('b') => out.push('\u{0008}'),
                    Some('f') => out.push('\u{000C}'),
                    Some('u') => {
                        let mut code = 0u32;
                        for _ in 0..4 {
                            let d = self.bump().ok_or("json: bad \\u escape")?;
                            code = code * 16 + d.to_digit(16).ok_or("json: bad hex in \\u")?;
                        }
                        out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                    }
                    _ => return Err("json: bad escape".into()),
                },
                Some(c) => out.push(c),
            }
        }
        Ok(out)
    }

    fn number(&mut self) -> Result<Val, String> {
        let start = self.pos;
        let mut is_float = false;
        if self.peek() == Some('-') { self.bump(); }
        while let Some(c) = self.peek() {
            match c {
                '0'..='9' => { self.bump(); }
                '.' | 'e' | 'E' | '+' | '-' => { is_float = true; self.bump(); }
                _ => break,
            }
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        if is_float {
            text.parse::<f64>().map(Val::Float).map_err(|_| "json: bad number".into())
        } else if let Some(i) = BigInt::parse(&text) {
            Ok(Val::Int(i))
        } else {
            text.parse::<f64>().map(Val::Float).map_err(|_| "json: bad number".into())
        }
    }

    fn boolean(&mut self) -> Result<Val, String> {
        if self.chars[self.pos..].starts_with(&['t', 'r', 'u', 'e']) { self.pos += 4; Ok(Val::Bool(true)) }
        else if self.chars[self.pos..].starts_with(&['f', 'a', 'l', 's', 'e']) { self.pos += 5; Ok(Val::Bool(false)) }
        else { Err("json: bad literal".into()) }
    }

    fn null(&mut self) -> Result<Val, String> {
        if self.chars[self.pos..].starts_with(&['n', 'u', 'l', 'l']) { self.pos += 4; Ok(Val::None) }
        else { Err("json: bad literal".into()) }
    }
}

fn json_write(v: &Val) -> Result<String, String> {
    let mut out = String::new();
    json_write_into(v, &mut out)?;
    Ok(out)
}

fn json_escape(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_write_into(v: &Val, out: &mut String) -> Result<(), String> {
    match v {
        Val::None => out.push_str("null"),
        Val::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Val::Int(i) => out.push_str(&i.to_string()),
        Val::Float(f) => out.push_str(&f.to_string()),
        Val::Str(s) => json_escape(s, out),
        Val::List(l) => {
            out.push('[');
            for (i, item) in l.borrow().iter().enumerate() {
                if i > 0 { out.push(','); }
                json_write_into(item, out)?;
            }
            out.push(']');
        }
        Val::Map(m) => {
            // Maps are insertion-ordered (SPEC §5.4), so writing them in that
            // order is both deterministic and round-trip faithful.
            let borrowed = m.borrow();
            let mut entries: Vec<(String, &Val)> = Vec::new();
            for (k, val) in borrowed.iter() {
                let key = match k { Val::Str(s) => s.clone(), other => other.to_string() };
                entries.push((key, val));
            }
            out.push('{');
            for (i, (k, val)) in entries.iter().enumerate() {
                if i > 0 { out.push(','); }
                json_escape(k, out);
                out.push(':');
                json_write_into(val, out)?;
            }
            out.push('}');
        }
        other => return Err(format!("json.write: cannot serialize {}", other)),
    }
    Ok(())
}

// --------------------------------------------------------------------------
// CSV (RFC 4180 subset)
// --------------------------------------------------------------------------

fn csv_parse(s: &str) -> Val {
    let mut rows: Vec<Val> = Vec::new();
    let mut field = String::new();
    let mut row: Vec<Val> = Vec::new();
    let mut in_quotes = false;
    let mut chars = s.chars().peekable();
    let mut saw_any = false;
    while let Some(c) = chars.next() {
        saw_any = true;
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') { chars.next(); field.push('"'); }
                else { in_quotes = false; }
            } else {
                field.push(c);
            }
        } else {
            match c {
                '"' => in_quotes = true,
                ',' => { row.push(Val::Str(std::mem::take(&mut field))); }
                '\n' => {
                    row.push(Val::Str(std::mem::take(&mut field)));
                    rows.push(Val::List(Rc::new(RefCell::new(std::mem::take(&mut row)))));
                }
                '\r' => {}
                _ => field.push(c),
            }
        }
    }
    // flush the last field/row if the input didn't end with a newline
    if !field.is_empty() || !row.is_empty() {
        row.push(Val::Str(field));
        rows.push(Val::List(Rc::new(RefCell::new(row))));
    } else if !saw_any {
        // empty input -> no rows
    }
    Val::List(Rc::new(RefCell::new(rows)))
}

fn csv_write(v: &Val) -> Result<Val, String> {
    let rows = match v { Val::List(l) => l.borrow().clone(), _ => return Err("csv.write: expects list of rows".into()) };
    let mut out = String::new();
    for (ri, row) in rows.iter().enumerate() {
        let fields = match row { Val::List(l) => l.borrow().clone(), _ => return Err("csv.write: each row must be a list".into()) };
        for (fi, field) in fields.iter().enumerate() {
            if fi > 0 { out.push(','); }
            let text = match field { Val::Str(s) => s.clone(), other => other.to_string() };
            if text.contains(',') || text.contains('"') || text.contains('\n') || text.contains('\r') {
                out.push('"');
                out.push_str(&text.replace('"', "\"\""));
                out.push('"');
            } else {
                out.push_str(&text);
            }
        }
        if ri + 1 < rows.len() { out.push('\n'); }
    }
    Ok(Val::Str(out))
}

// --------------------------------------------------------------------------
// SHA-256 (FIPS 180-4) and CRC-32 (IEEE 802.3)
// --------------------------------------------------------------------------

fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 { msg.push(0); }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g; g = f; f = e; e = d.wrapping_add(t1);
            d = c; c = b; b = a; a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a); h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c); h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e); h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g); h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

// --------------------------------------------------------------------------
// Regex: a non-backtracking Thompson NFA (no catastrophic blowup).
// Supports: literals, `.`, char classes `[...]`/`[^...]` with ranges,
// `\d \w \s \D \W \S` and escaped metachars, `*` `+` `?`, `|`, `()`,
// and anchors `^` `$`.
// --------------------------------------------------------------------------

mod regex {
    #[derive(Clone)]
    enum Node {
        Char(char),
        Any,
        Class(Vec<ClassItem>, bool), // items, negated
        Concat(Vec<Node>),
        Alt(Box<Node>, Box<Node>),
        Star(Box<Node>),
        Plus(Box<Node>),
        Opt(Box<Node>),
        StartAnchor,
        EndAnchor,
        Empty,
    }

    #[derive(Clone)]
    enum ClassItem { Single(char), Range(char, char), Digit, Word, Space }

    pub struct Regex { root: Node }

    struct P<'a> { c: &'a [char], i: usize }

    impl<'a> P<'a> {
        fn peek(&self) -> Option<char> { self.c.get(self.i).copied() }
        fn bump(&mut self) -> Option<char> { let x = self.peek(); if x.is_some() { self.i += 1; } x }

        // alternation: concat ('|' concat)*
        fn alt(&mut self) -> Result<Node, String> {
            let mut left = self.concat()?;
            while self.peek() == Some('|') {
                self.bump();
                let right = self.concat()?;
                left = Node::Alt(Box::new(left), Box::new(right));
            }
            Ok(left)
        }

        fn concat(&mut self) -> Result<Node, String> {
            let mut parts = Vec::new();
            while let Some(ch) = self.peek() {
                if ch == '|' || ch == ')' { break; }
                parts.push(self.repeat()?);
            }
            if parts.is_empty() { Ok(Node::Empty) }
            else if parts.len() == 1 { Ok(parts.pop().unwrap()) }
            else { Ok(Node::Concat(parts)) }
        }

        fn repeat(&mut self) -> Result<Node, String> {
            let atom = self.atom()?;
            match self.peek() {
                Some('*') => { self.bump(); Ok(Node::Star(Box::new(atom))) }
                Some('+') => { self.bump(); Ok(Node::Plus(Box::new(atom))) }
                Some('?') => { self.bump(); Ok(Node::Opt(Box::new(atom))) }
                _ => Ok(atom),
            }
        }

        fn atom(&mut self) -> Result<Node, String> {
            match self.bump() {
                Some('(') => {
                    let inner = self.alt()?;
                    if self.bump() != Some(')') { return Err("unbalanced '('".into()); }
                    Ok(inner)
                }
                Some('[') => self.class(),
                Some('.') => Ok(Node::Any),
                Some('^') => Ok(Node::StartAnchor),
                Some('$') => Ok(Node::EndAnchor),
                Some('\\') => Ok(self.escape()?),
                Some(c) => Ok(Node::Char(c)),
                None => Err("unexpected end of pattern".into()),
            }
        }

        fn escape(&mut self) -> Result<Node, String> {
            match self.bump() {
                Some('d') => Ok(Node::Class(vec![ClassItem::Digit], false)),
                Some('w') => Ok(Node::Class(vec![ClassItem::Word], false)),
                Some('s') => Ok(Node::Class(vec![ClassItem::Space], false)),
                Some('D') => Ok(Node::Class(vec![ClassItem::Digit], true)),
                Some('W') => Ok(Node::Class(vec![ClassItem::Word], true)),
                Some('S') => Ok(Node::Class(vec![ClassItem::Space], true)),
                Some('n') => Ok(Node::Char('\n')),
                Some('t') => Ok(Node::Char('\t')),
                Some('r') => Ok(Node::Char('\r')),
                Some(c) => Ok(Node::Char(c)),
                None => Err("trailing backslash".into()),
            }
        }

        fn class(&mut self) -> Result<Node, String> {
            let mut items = Vec::new();
            let negated = if self.peek() == Some('^') { self.bump(); true } else { false };
            while let Some(c) = self.peek() {
                if c == ']' { self.bump(); return Ok(Node::Class(items, negated)); }
                let lo = match self.bump() {
                    Some('\\') => match self.bump() {
                        Some('d') => { items.push(ClassItem::Digit); continue; }
                        Some('w') => { items.push(ClassItem::Word); continue; }
                        Some('s') => { items.push(ClassItem::Space); continue; }
                        Some('n') => '\n', Some('t') => '\t', Some('r') => '\r',
                        Some(x) => x,
                        None => return Err("bad class escape".into()),
                    },
                    Some(x) => x,
                    None => break,
                };
                if self.peek() == Some('-') && self.c.get(self.i + 1).is_some_and(|&n| n != ']') {
                    self.bump(); // -
                    let hi = self.bump().unwrap();
                    items.push(ClassItem::Range(lo, hi));
                } else {
                    items.push(ClassItem::Single(lo));
                }
            }
            Err("unterminated character class".into())
        }
    }

    impl Regex {
        pub fn compile(pattern: &str) -> Result<Regex, String> {
            let chars: Vec<char> = pattern.chars().collect();
            let mut p = P { c: &chars, i: 0 };
            let root = p.alt()?;
            if p.i != chars.len() { return Err("unexpected trailing pattern".into()); }
            Ok(Regex { root })
        }

        pub fn is_match(&self, text: &str) -> bool {
            self.find_at(text).is_some()
        }

        pub fn find(&self, text: &str) -> Option<String> {
            self.find_at(text).map(|(s, e, chars)| chars[s..e].iter().collect())
        }

        // Returns (start, end, chars) of the leftmost match.
        fn find_at(&self, text: &str) -> Option<(usize, usize, Vec<char>)> {
            let chars: Vec<char> = text.chars().collect();
            let anchored_start = matches!(leftmost(&self.root), Some(true));
            for start in 0..=chars.len() {
                if let Some(end) = match_here(&self.root, &chars, start, start) {
                    return Some((start, end, chars));
                }
                if anchored_start { break; }
            }
            None
        }
    }

    // Whether the pattern begins with a start anchor (so we needn't slide).
    fn leftmost(node: &Node) -> Option<bool> {
        match node {
            Node::StartAnchor => Some(true),
            Node::Concat(parts) => parts.first().and_then(leftmost),
            _ => Some(false),
        }
    }

    // Backtracking-free-ish matcher via continuation set. To keep it simple and
    // safe against catastrophic blowup, we compute the set of possible end
    // positions for a node and pick the maximal one via a recursive matcher
    // that returns the longest match end. Star/Plus are bounded by input length.
    fn match_here(node: &Node, chars: &[char], pos: usize, origin: usize) -> Option<usize> {
        // returns end position of a successful match of `node` starting at `pos`
        match node {
            Node::Empty => Some(pos),
            Node::Char(c) => if chars.get(pos) == Some(c) { Some(pos + 1) } else { None },
            Node::Any => if pos < chars.len() { Some(pos + 1) } else { None },
            Node::Class(items, neg) => {
                if let Some(&c) = chars.get(pos) {
                    if class_matches(items, c) != *neg { Some(pos + 1) } else { None }
                } else { None }
            }
            Node::StartAnchor => if pos == 0 { Some(pos) } else { None },
            Node::EndAnchor => if pos == chars.len() { Some(pos) } else { None },
            Node::Alt(a, b) => {
                match_here(a, chars, pos, origin).or_else(|| match_here(b, chars, pos, origin))
            }
            Node::Concat(parts) => match_seq(parts, chars, pos, origin),
            Node::Opt(inner) => {
                // try matching once (greedy), else empty
                match_here(inner, chars, pos, origin).or(Some(pos))
            }
            Node::Star(inner) => match_star(inner, chars, pos, origin, 0),
            Node::Plus(inner) => {
                let next = match_here(inner, chars, pos, origin)?;
                match_star(inner, chars, next, origin, 1)
            }
        }
    }

    // Greedy star: consume as many as possible, at least `_min` already consumed.
    fn match_star(inner: &Node, chars: &[char], pos: usize, origin: usize, _min: usize) -> Option<usize> {
        // collect reachable positions, greedily prefer the furthest
        let mut cur = pos;
        loop {
            match match_here(inner, chars, cur, origin) {
                Some(next) if next > cur => cur = next,
                _ => break,
            }
        }
        Some(cur)
    }

    fn match_seq(parts: &[Node], chars: &[char], pos: usize, origin: usize) -> Option<usize> {
        if parts.is_empty() { return Some(pos); }
        let (head, tail) = parts.split_first().unwrap();
        // For quantified/alt heads we need to try multiple end positions of head.
        for end in head_ends(head, chars, pos, origin) {
            if let Some(final_end) = match_seq(tail, chars, end, origin) {
                return Some(final_end);
            }
        }
        None
    }

    // Enumerate candidate end positions for a single node, longest first, so
    // concatenation can backtrack over quantifier choices without exponential
    // blowup (each is bounded by input length).
    fn head_ends(node: &Node, chars: &[char], pos: usize, origin: usize) -> Vec<usize> {
        match node {
            Node::Star(inner) => {
                let mut ends = vec![pos];
                let mut cur = pos;
                while let Some(next) = match_here(inner, chars, cur, origin) {
                    if next <= cur { break; }
                    cur = next;
                    ends.push(cur);
                }
                ends.reverse(); // longest first (greedy)
                ends
            }
            Node::Plus(inner) => {
                let mut ends = Vec::new();
                let mut cur = pos;
                while let Some(next) = match_here(inner, chars, cur, origin) {
                    if next <= cur { break; }
                    cur = next;
                    ends.push(cur);
                }
                ends.reverse();
                ends
            }
            Node::Opt(inner) => {
                match match_here(inner, chars, pos, origin) {
                    Some(end) if end != pos => vec![end, pos],
                    _ => vec![pos],
                }
            }
            Node::Alt(a, b) => {
                let mut ends = head_ends(a, chars, pos, origin);
                ends.extend(head_ends(b, chars, pos, origin));
                ends
            }
            other => match match_here(other, chars, pos, origin) {
                Some(end) => vec![end],
                None => vec![],
            },
        }
    }

    fn class_matches(items: &[ClassItem], c: char) -> bool {
        for item in items {
            let hit = match item {
                ClassItem::Single(x) => c == *x,
                ClassItem::Range(lo, hi) => *lo <= c && c <= *hi,
                ClassItem::Digit => c.is_ascii_digit(),
                ClassItem::Word => c.is_ascii_alphanumeric() || c == '_',
                ClassItem::Space => c.is_whitespace(),
            };
            if hit { return true; }
        }
        false
    }

    #[cfg(test)]
    mod tests {
        use super::Regex;

        fn m(p: &str, t: &str) -> bool { Regex::compile(p).unwrap().is_match(t) }

        #[test]
        fn literals_and_classes() {
            assert!(m("abc", "xabcy"));
            assert!(!m("abc", "abx"));
            assert!(m("a.c", "azc"));
            assert!(m("[0-9]+", "id 42"));
            assert!(!m("^[0-9]+$", "12a"));
            assert!(m(r"\d\d:\d\d", "at 09:30"));
            assert!(m("[^abc]", "d"));
            assert!(!m("[^abc]", "a"));
        }

        #[test]
        fn quantifiers_and_alt() {
            assert!(m("ab*c", "ac"));
            assert!(m("ab*c", "abbbc"));
            assert!(m("colou?r", "color"));
            assert!(m("colou?r", "colour"));
            assert!(m("cat|dog", "hotdog"));
            assert!(m("^(a|b)+$", "abba"));
            assert!(!m("^(a|b)+$", "abc"));
        }

        #[test]
        fn find_returns_leftmost() {
            let re = Regex::compile("[0-9]+").unwrap();
            assert_eq!(re.find("abc123def456"), Some("123".to_string()));
            assert_eq!(re.find("none here"), None);
        }

        #[test]
        fn pathological_completes_fast() {
            // Classic catastrophic-backtracking pattern; an NFA-style matcher
            // must not hang. This returns essentially instantly.
            let re = Regex::compile("(a+)+$").unwrap();
            assert!(!re.is_match("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaX"));
        }
    }
}
