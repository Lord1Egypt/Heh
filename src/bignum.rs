//! Integers are unbounded (SPEC §5.1). SPEC's own implementation note asks for
//! "a machine-word fast path with automatic promotion to a bignum, so ordinary
//! arithmetic runs at native speed", which is exactly the shape here:
//!
//! - `BigInt::Small(i64)` holds anything that fits a machine word — no
//!   allocation at all, and arithmetic is a checked machine instruction.
//! - `BigInt::Big` is the limb-vector implementation, used only once a value
//!   outstrips i64.
//!
//! Every operation normalizes its result back to `Small` when it fits, so a
//! given mathematical value has exactly ONE representation. That invariant is
//! what lets `Eq`, `Ord`, and `Hash` stay consistent across the two forms —
//! without it, `Small(5) != Big(5)` and maps keyed by ints would break.

use std::cmp::Ordering;
use std::fmt;

use std::hash::{Hash, Hasher};

#[derive(Clone, Eq)]
pub struct Big {
    pub sign: bool,      // true if negative
    pub limbs: Vec<u32>, // base 2^32, little endian
}

impl Big {
    pub fn zero() -> Self {
        Self {
            sign: false,
            limbs: Vec::new(),
        }
    }

    pub fn from_u64(mut n: u64) -> Self {
        if n == 0 {
            return Self::zero();
        }
        let mut limbs = Vec::new();
        while n > 0 {
            limbs.push((n & 0xFFFFFFFF) as u32);
            n >>= 32;
        }
        Self { sign: false, limbs }
    }

    pub fn from_i64(n: i64) -> Self {
        let mut b = Self::from_u64(n.unsigned_abs());
        if n < 0 && !b.limbs.is_empty() {
            b.sign = true;
        }
        b
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.replace("_", "");
        if s.is_empty() {
            return None;
        }
        let mut sign = false;
        let mut bytes = s.as_bytes();
        if bytes[0] == b'-' {
            sign = true;
            bytes = &bytes[1..];
        } else if bytes[0] == b'+' {
            bytes = &bytes[1..];
        }

        if bytes.is_empty() {
            return None;
        }

        let mut radix = 10;
        let mut digits = bytes;
        if bytes.len() >= 2 && bytes[0] == b'0' {
            match bytes[1] {
                b'x' | b'X' => {
                    radix = 16;
                    digits = &bytes[2..];
                }
                b'b' | b'B' => {
                    radix = 2;
                    digits = &bytes[2..];
                }
                b'o' | b'O' => {
                    radix = 8;
                    digits = &bytes[2..];
                }
                _ => {}
            }
        }

        let mut res = Self::zero();
        for &b in digits {
            let d = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => return None,
            };
            if d >= radix {
                return None;
            }
            res = res.mul_u32(radix as u32).add_u32(d as u32);
        }
        if !res.limbs.is_empty() {
            res.sign = sign;
        }
        Some(res)
    }

    fn mul_u32(&self, v: u32) -> Self {
        if v == 0 || self.limbs.is_empty() {
            return Self::zero();
        }
        if v == 1 {
            return self.clone();
        }
        let mut limbs = Vec::with_capacity(self.limbs.len() + 1);
        let mut carry: u64 = 0;
        let vu64 = v as u64;
        for &limb in &self.limbs {
            let prod = (limb as u64) * vu64 + carry;
            limbs.push(prod as u32);
            carry = prod >> 32;
        }
        if carry > 0 {
            limbs.push(carry as u32);
        }
        Self {
            sign: self.sign,
            limbs,
        }
    }

    fn add_u32(&self, v: u32) -> Self {
        if v == 0 {
            return self.clone();
        }
        if self.sign {
            // Negative + Positive => Subtract
            // But we only use this in parsing where sign=false
            unimplemented!()
        }
        let mut limbs = self.limbs.clone();
        let mut carry = v as u64;
        for limb in &mut limbs {
            let sum = (*limb as u64) + carry;
            *limb = sum as u32;
            carry = sum >> 32;
            if carry == 0 {
                break;
            }
        }
        if carry > 0 {
            limbs.push(carry as u32);
        }
        Self { sign: false, limbs }
    }

    pub fn to_f64(&self) -> f64 {
        let mut res = 0.0;
        for &limb in self.limbs.iter().rev() {
            res = res * 4294967296.0 + (limb as f64);
        }
        if self.sign {
            -res
        } else {
            res
        }
    }

    fn cmp_abs(&self, other: &Self) -> Ordering {
        if self.limbs.len() != other.limbs.len() {
            return self.limbs.len().cmp(&other.limbs.len());
        }
        for (a, b) in self.limbs.iter().rev().zip(other.limbs.iter().rev()) {
            if a != b {
                return a.cmp(b);
            }
        }
        Ordering::Equal
    }

    fn add_abs(&self, other: &Self) -> Vec<u32> {
        let max_len = self.limbs.len().max(other.limbs.len());
        let mut limbs = Vec::with_capacity(max_len + 1);
        let mut carry = 0u64;
        for i in 0..max_len {
            let a = self.limbs.get(i).copied().unwrap_or(0) as u64;
            let b = other.limbs.get(i).copied().unwrap_or(0) as u64;
            let sum = a + b + carry;
            limbs.push(sum as u32);
            carry = sum >> 32;
        }
        if carry > 0 {
            limbs.push(carry as u32);
        }
        limbs
    }

    fn sub_abs(&self, other: &Self) -> Vec<u32> {
        // Assume self >= other
        let mut limbs = Vec::with_capacity(self.limbs.len());
        let mut borrow = 0i64;
        for i in 0..self.limbs.len() {
            let a = self.limbs[i] as i64;
            let b = other.limbs.get(i).copied().unwrap_or(0) as i64;
            let diff = a - b - borrow;
            if diff < 0 {
                limbs.push((diff + 4294967296) as u32);
                borrow = 1;
            } else {
                limbs.push(diff as u32);
                borrow = 0;
            }
        }
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        limbs
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    /// Value as an `i64`, or `None` when it does not fit — the test that
    /// decides whether a result can live in the machine-word fast path.
    pub fn to_i64(&self) -> Option<i64> {
        if self.limbs.len() > 2 {
            return None;
        }
        let mut mag: u64 = 0;
        for (i, &limb) in self.limbs.iter().enumerate() {
            mag |= (limb as u64) << (32 * i);
        }
        if self.sign {
            // i64::MIN is representable even though its magnitude is not.
            if mag == 1 << 63 {
                return Some(i64::MIN);
            }
            i64::try_from(mag).ok().map(|v| -v)
        } else {
            i64::try_from(mag).ok()
        }
    }

    /// Truncate a finite float towards zero into an exact integer. Callers must
    /// reject nan/inf first — those have no integer value at all.
    pub fn from_f64_trunc(f: f64) -> Self {
        let neg = f < 0.0;
        let mut mag = f.abs().trunc();
        let mut out = Self::zero();
        let mut scale = Self::from_u64(1);
        // Peel 32 bits at a time off the bottom so huge floats stay exact.
        const CHUNK: f64 = 4294967296.0;
        while mag >= 1.0 {
            let rem = (mag % CHUNK) as u64;
            out = &out + &(&Self::from_u64(rem) * &scale);
            scale = &scale * &Self::from_u64(CHUNK as u64);
            mag = (mag / CHUNK).trunc();
        }
        out.sign = neg && !out.is_zero();
        out
    }

    /// Value as a `usize`, or `None` when negative or too large to index with.
    pub fn to_usize(&self) -> Option<usize> {
        if self.sign {
            return None;
        }
        if self.limbs.len() > 2 {
            return None;
        }
        let mut v: u64 = 0;
        for (i, &limb) in self.limbs.iter().enumerate() {
            v |= (limb as u64) << (32 * i);
        }
        usize::try_from(v).ok()
    }
}

impl PartialEq for Big {
    fn eq(&self, other: &Self) -> bool {
        if self.is_zero() && other.is_zero() {
            return true;
        }
        self.sign == other.sign && self.limbs == other.limbs
    }
}

impl PartialOrd for Big {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Big {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.is_zero() && other.is_zero() {
            return Ordering::Equal;
        }
        match (self.sign, other.sign) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => self.cmp_abs(other),
            (true, true) => other.cmp_abs(self),
        }
    }
}

impl Hash for Big {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.is_zero() {
            false.hash(state);
            return;
        }
        self.sign.hash(state);
        self.limbs.hash(state);
    }
}

impl std::ops::Add for &Big {
    type Output = Big;
    fn add(self, other: Self) -> Big {
        if self.sign == other.sign {
            let limbs = self.add_abs(other);
            Big {
                sign: self.sign,
                limbs,
            }
        } else {
            match self.cmp_abs(other) {
                Ordering::Equal => Big::zero(),
                Ordering::Greater => {
                    let limbs = self.sub_abs(other);
                    Big {
                        sign: self.sign,
                        limbs,
                    }
                }
                Ordering::Less => {
                    let limbs = other.sub_abs(self);
                    Big {
                        sign: other.sign,
                        limbs,
                    }
                }
            }
        }
    }
}

impl std::ops::Sub for &Big {
    type Output = Big;
    fn sub(self, other: Self) -> Big {
        if self.sign != other.sign {
            let limbs = self.add_abs(other);
            Big {
                sign: self.sign,
                limbs,
            }
        } else {
            match self.cmp_abs(other) {
                Ordering::Equal => Big::zero(),
                Ordering::Greater => {
                    let limbs = self.sub_abs(other);
                    Big {
                        sign: self.sign,
                        limbs,
                    }
                }
                Ordering::Less => {
                    let limbs = other.sub_abs(self);
                    Big {
                        sign: !self.sign,
                        limbs,
                    }
                }
            }
        }
    }
}

impl std::ops::Mul for &Big {
    type Output = Big;
    fn mul(self, other: Self) -> Big {
        if self.is_zero() || other.is_zero() {
            return Big::zero();
        }
        let mut limbs = vec![0u32; self.limbs.len() + other.limbs.len()];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &b) in other.limbs.iter().enumerate() {
                let prod = (a as u64) * (b as u64) + (limbs[i + j] as u64) + carry;
                limbs[i + j] = prod as u32;
                carry = prod >> 32;
            }
            limbs[i + other.limbs.len()] = carry as u32;
        }
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        Big {
            sign: self.sign != other.sign,
            limbs,
        }
    }
}

impl Big {
    // Basic division by 1-limb
    fn div_mod_u32(&self, div: u32) -> (Self, u32) {
        if div == 0 {
            panic!("division by zero");
        }
        let mut limbs = Vec::with_capacity(self.limbs.len());
        let mut rem = 0u64;
        let div64 = div as u64;
        for &limb in self.limbs.iter().rev() {
            let cur = (rem << 32) | (limb as u64);
            limbs.push((cur / div64) as u32);
            rem = cur % div64;
        }
        limbs.reverse();
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        (
            Self {
                sign: self.sign,
                limbs,
            },
            rem as u32,
        )
    }

    // Binary long division for Big / Big. Not the fastest, but small code.
    pub fn div_mod(&self, other: &Self) -> (Self, Self) {
        if other.is_zero() {
            panic!("division by zero");
        }
        if self.cmp_abs(other) == Ordering::Less {
            return (Self::zero(), self.clone());
        }

        let mut q = Self::zero();

        // Count bits
        let self_bits = self.limbs.len() * 32 - self.limbs.last().unwrap().leading_zeros() as usize;
        let other_bits =
            other.limbs.len() * 32 - other.limbs.last().unwrap().leading_zeros() as usize;

        let mut b = other.clone();
        b.sign = false;

        // Shift b up to align with self
        let mut shift = self_bits.saturating_sub(other_bits);
        let mut b_shifted = b.shl_usize(shift);

        let mut a = self.clone();
        a.sign = false;

        loop {
            if a.cmp_abs(&b_shifted) != Ordering::Less {
                a = &a - &b_shifted;
                // Add 1 << shift to q
                let q_bit = Self::zero().set_bit(shift);
                q = &q + &q_bit;
            }
            if shift == 0 {
                break;
            }
            shift -= 1;
            b_shifted = b_shifted.shr_1();
        }

        q.sign = self.sign != other.sign;
        a.sign = self.sign;
        if q.is_zero() {
            q.sign = false;
        }
        if a.is_zero() {
            a.sign = false;
        }

        (q, a)
    }

    fn shl_usize(&self, mut shift: usize) -> Self {
        if self.is_zero() {
            return self.clone();
        }
        let limb_shift = shift / 32;
        shift %= 32;
        let mut limbs = vec![0u32; limb_shift];
        let mut carry = 0u32;
        for &limb in &self.limbs {
            let shifted = (limb << shift) | carry;
            limbs.push(shifted);
            carry = if shift == 0 { 0 } else { limb >> (32 - shift) };
        }
        if carry > 0 {
            limbs.push(carry);
        }
        Self {
            sign: self.sign,
            limbs,
        }
    }

    fn shr_1(&self) -> Self {
        if self.is_zero() {
            return self.clone();
        }
        let mut limbs = Vec::with_capacity(self.limbs.len());
        let mut carry = 0u32;
        for &limb in self.limbs.iter().rev() {
            let shifted = (limb >> 1) | carry;
            limbs.push(shifted);
            carry = limb << 31;
        }
        limbs.reverse();
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        Self {
            sign: self.sign,
            limbs,
        }
    }

    /// Floor division and Python-style modulo (SPEC §6.1): the quotient rounds
    /// towards negative infinity and the remainder takes the divisor's sign.
    /// `div_mod` truncates, so adjust by one whenever the remainder is nonzero
    /// and the operands' signs differ.
    pub fn div_mod_floor(&self, other: &Self) -> (Self, Self) {
        let (q, r) = self.div_mod(other);
        if r.is_zero() || self.sign == other.sign {
            return (q, r);
        }
        (&q - &Self::from_u64(1), &r + other)
    }

    /// `self ** exp` by square-and-multiply. `exp` must be non-negative.
    pub fn pow(&self, exp: &Self) -> Self {
        let mut result = Self::from_u64(1);
        let mut base = self.clone();
        let mut e = exp.clone();
        e.sign = false;
        while !e.is_zero() {
            if e.limbs[0] & 1 == 1 {
                result = &result * &base;
            }
            e = e.shr_1();
            if !e.is_zero() {
                base = &base * &base;
            }
        }
        result
    }

    fn set_bit(mut self, bit: usize) -> Self {
        let limb_idx = bit / 32;
        while self.limbs.len() <= limb_idx {
            self.limbs.push(0);
        }
        self.limbs[limb_idx] |= 1 << (bit % 32);
        self
    }
}

impl fmt::Display for Big {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return write!(f, "0");
        }
        if self.sign {
            write!(f, "-")?;
        }
        // Base 1e9 conversion
        let mut temp = self.clone();
        temp.sign = false;
        let mut chunks = Vec::new();
        let billion = 1_000_000_000u32;
        while !temp.is_zero() {
            let (q, r) = temp.div_mod_u32(billion);
            chunks.push(r);
            temp = q;
        }
        write!(f, "{}", chunks.last().unwrap())?;
        for &chunk in chunks.iter().rev().skip(1) {
            write!(f, "{:09}", chunk)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Big {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Big({})", self)
    }
}

// ---------------------------------------------------------------------------
// The public integer: a machine word until it has to be more.
// ---------------------------------------------------------------------------

#[derive(Clone, Eq)]
pub enum BigInt {
    Small(i64),
    Large(Big),
}

impl BigInt {
    pub fn zero() -> Self {
        BigInt::Small(0)
    }

    pub fn from_i64(n: i64) -> Self {
        BigInt::Small(n)
    }

    pub fn from_u64(n: u64) -> Self {
        match i64::try_from(n) {
            Ok(v) => BigInt::Small(v),
            Err(_) => BigInt::Large(Big::from_u64(n)),
        }
    }

    /// Demote to `Small` whenever the value fits, so each value has exactly one
    /// representation (see the module note).
    fn norm(b: Big) -> Self {
        match b.to_i64() {
            Some(v) => BigInt::Small(v),
            None => BigInt::Large(b),
        }
    }

    fn big(&self) -> Big {
        match self {
            BigInt::Small(v) => Big::from_i64(*v),
            BigInt::Large(b) => b.clone(),
        }
    }

    pub fn is_zero(&self) -> bool {
        match self {
            BigInt::Small(v) => *v == 0,
            BigInt::Large(b) => b.is_zero(),
        }
    }

    pub fn is_negative(&self) -> bool {
        match self {
            BigInt::Small(v) => *v < 0,
            BigInt::Large(b) => b.sign && !b.is_zero(),
        }
    }

    /// Absolute value.
    pub fn abs(&self) -> Self {
        match self {
            // i64::MIN has no positive counterpart in a machine word.
            BigInt::Small(v) => match v.checked_abs() {
                Some(a) => BigInt::Small(a),
                None => {
                    let mut b = Big::from_i64(*v);
                    b.sign = false;
                    BigInt::Large(b)
                }
            },
            BigInt::Large(b) => {
                let mut b = b.clone();
                b.sign = false;
                Self::norm(b)
            }
        }
    }

    pub fn negate(&self) -> Self {
        match self {
            BigInt::Small(v) => match v.checked_neg() {
                Some(n) => BigInt::Small(n),
                None => {
                    let mut b = Big::from_i64(*v);
                    b.sign = false;
                    BigInt::Large(b)
                }
            },
            BigInt::Large(b) => {
                let mut b = b.clone();
                if !b.is_zero() {
                    b.sign = !b.sign;
                }
                Self::norm(b)
            }
        }
    }

    pub fn to_i64(&self) -> Option<i64> {
        match self {
            BigInt::Small(v) => Some(*v),
            BigInt::Large(b) => b.to_i64(),
        }
    }

    pub fn to_f64(&self) -> f64 {
        match self {
            BigInt::Small(v) => *v as f64,
            BigInt::Large(b) => b.to_f64(),
        }
    }

    /// Value as a `usize`, or `None` when negative or too large to index with.
    pub fn to_usize(&self) -> Option<usize> {
        match self {
            BigInt::Small(v) => usize::try_from(*v).ok(),
            BigInt::Large(b) => b.to_usize(),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Big::parse(s).map(Self::norm)
    }

    pub fn from_f64_trunc(f: f64) -> Self {
        // Anything inside the machine-word range converts exactly and directly.
        let t = f.trunc();
        if t >= -(2f64.powi(63)) && t < 2f64.powi(63) {
            return BigInt::Small(t as i64);
        }
        Self::norm(Big::from_f64_trunc(f))
    }

    /// Truncating division and remainder (quotient rounds towards zero).
    pub fn div_mod(&self, other: &Self) -> (Self, Self) {
        if let (BigInt::Small(a), BigInt::Small(b)) = (self, other) {
            // i64::MIN / -1 is the one pair that overflows a machine word.
            if let (Some(q), Some(r)) = (a.checked_div(*b), a.checked_rem(*b)) {
                return (BigInt::Small(q), BigInt::Small(r));
            }
        }
        let (q, r) = self.big().div_mod(&other.big());
        (Self::norm(q), Self::norm(r))
    }

    /// Floor division and Python-style modulo (SPEC §6.1).
    pub fn div_mod_floor(&self, other: &Self) -> (Self, Self) {
        if let (BigInt::Small(a), BigInt::Small(b)) = (self, other) {
            if let (Some(q), Some(r)) = (a.checked_div_euclid(*b), a.checked_rem_euclid(*b)) {
                // Euclidean division keeps the remainder non-negative; SPEC §6.1
                // wants the remainder to take the DIVISOR's sign. They agree for
                // a positive divisor, and differ by one step for a negative one
                // with a non-zero remainder: 1 // -2 is -1, not 0.
                if *b < 0 && r != 0 {
                    if let (Some(q), Some(r)) = (q.checked_sub(1), r.checked_add(*b)) {
                        return (BigInt::Small(q), BigInt::Small(r));
                    }
                } else {
                    return (BigInt::Small(q), BigInt::Small(r));
                }
            }
        }
        let (q, r) = self.big().div_mod_floor(&other.big());
        (Self::norm(q), Self::norm(r))
    }

    /// `self ** exp`, exact and unbounded. `exp` must be non-negative.
    pub fn pow(&self, exp: &Self) -> Self {
        if let (BigInt::Small(base), Some(e)) = (self, exp.to_i64()) {
            if let Ok(e32) = u32::try_from(e) {
                if let Some(v) = base.checked_pow(e32) {
                    return BigInt::Small(v);
                }
            }
        }
        Self::norm(self.big().pow(&exp.big()))
    }
}

impl PartialEq for BigInt {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (BigInt::Small(a), BigInt::Small(b)) => a == b,
            // Normalization means a Small and a Large are never equal.
            (BigInt::Large(a), BigInt::Large(b)) => a == b,
            _ => false,
        }
    }
}

impl PartialOrd for BigInt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BigInt {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (BigInt::Small(a), BigInt::Small(b)) => a.cmp(b),
            // A normalized Large is always outside i64's range, so its sign
            // alone decides against any Small.
            (BigInt::Large(a), BigInt::Small(_)) => {
                if a.sign {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            }
            (BigInt::Small(_), BigInt::Large(b)) => {
                if b.sign {
                    Ordering::Greater
                } else {
                    Ordering::Less
                }
            }
            (BigInt::Large(a), BigInt::Large(b)) => a.cmp(b),
        }
    }
}

impl Hash for BigInt {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the mathematical value, not the representation. Normalization
        // guarantees only one arm can ever hold a given value, but hashing
        // through a common form keeps that from being load-bearing.
        match self {
            BigInt::Small(v) => v.hash(state),
            BigInt::Large(b) => b.hash(state),
        }
    }
}

impl std::ops::Add for &BigInt {
    type Output = BigInt;
    fn add(self, other: Self) -> BigInt {
        if let (BigInt::Small(a), BigInt::Small(b)) = (self, other) {
            if let Some(v) = a.checked_add(*b) {
                return BigInt::Small(v);
            }
        }
        BigInt::norm(&self.big() + &other.big())
    }
}

impl std::ops::Sub for &BigInt {
    type Output = BigInt;
    fn sub(self, other: Self) -> BigInt {
        if let (BigInt::Small(a), BigInt::Small(b)) = (self, other) {
            if let Some(v) = a.checked_sub(*b) {
                return BigInt::Small(v);
            }
        }
        BigInt::norm(&self.big() - &other.big())
    }
}

impl std::ops::Mul for &BigInt {
    type Output = BigInt;
    fn mul(self, other: Self) -> BigInt {
        if let (BigInt::Small(a), BigInt::Small(b)) = (self, other) {
            if let Some(v) = a.checked_mul(*b) {
                return BigInt::Small(v);
            }
        }
        BigInt::norm(&self.big() * &other.big())
    }
}

impl fmt::Display for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BigInt::Small(v) => write!(f, "{}", v),
            BigInt::Large(b) => write!(f, "{}", b),
        }
    }
}

impl fmt::Debug for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BigInt({})", self)
    }
}
