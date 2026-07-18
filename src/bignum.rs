use std::fmt;
use std::cmp::Ordering;

#[derive(Clone, Eq)]
pub struct BigInt {
    pub sign: bool, // true if negative
    pub limbs: Vec<u32>, // base 2^32, little endian
}

impl BigInt {
    pub fn zero() -> Self {
        Self { sign: false, limbs: Vec::new() }
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
        if s.is_empty() { return None; }
        let mut sign = false;
        let mut bytes = s.as_bytes();
        if bytes[0] == b'-' {
            sign = true;
            bytes = &bytes[1..];
        } else if bytes[0] == b'+' {
            bytes = &bytes[1..];
        }

        if bytes.is_empty() { return None; }

        let mut radix = 10;
        let mut digits = bytes;
        if bytes.len() >= 2 && bytes[0] == b'0' {
            match bytes[1] {
                b'x' | b'X' => { radix = 16; digits = &bytes[2..]; }
                b'b' | b'B' => { radix = 2; digits = &bytes[2..]; }
                b'o' | b'O' => { radix = 8; digits = &bytes[2..]; }
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
            if d >= radix { return None; }
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
        Self { sign: self.sign, limbs }
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
        if self.sign { -res } else { res }
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
}

impl PartialEq for BigInt {
    fn eq(&self, other: &Self) -> bool {
        if self.is_zero() && other.is_zero() {
            return true;
        }
        self.sign == other.sign && self.limbs == other.limbs
    }
}

impl PartialOrd for BigInt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BigInt {
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

impl std::ops::Add for &BigInt {
    type Output = BigInt;
    fn add(self, other: Self) -> BigInt {
        if self.sign == other.sign {
            let limbs = self.add_abs(other);
            BigInt { sign: self.sign, limbs }
        } else {
            match self.cmp_abs(other) {
                Ordering::Equal => BigInt::zero(),
                Ordering::Greater => {
                    let limbs = self.sub_abs(other);
                    BigInt { sign: self.sign, limbs }
                }
                Ordering::Less => {
                    let limbs = other.sub_abs(self);
                    BigInt { sign: other.sign, limbs }
                }
            }
        }
    }
}

impl std::ops::Sub for &BigInt {
    type Output = BigInt;
    fn sub(self, other: Self) -> BigInt {
        if self.sign != other.sign {
            let limbs = self.add_abs(other);
            BigInt { sign: self.sign, limbs }
        } else {
            match self.cmp_abs(other) {
                Ordering::Equal => BigInt::zero(),
                Ordering::Greater => {
                    let limbs = self.sub_abs(other);
                    BigInt { sign: self.sign, limbs }
                }
                Ordering::Less => {
                    let limbs = other.sub_abs(self);
                    BigInt { sign: !self.sign, limbs }
                }
            }
        }
    }
}

impl std::ops::Mul for &BigInt {
    type Output = BigInt;
    fn mul(self, other: Self) -> BigInt {
        if self.is_zero() || other.is_zero() {
            return BigInt::zero();
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
        BigInt { sign: self.sign != other.sign, limbs }
    }
}

impl BigInt {
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
        (Self { sign: self.sign, limbs }, rem as u32)
    }

    // Binary long division for BigInt / BigInt. Not the fastest, but small code.
    pub fn div_mod(&self, other: &Self) -> (Self, Self) {
        if other.is_zero() {
            panic!("division by zero");
        }
        if self.cmp_abs(other) == Ordering::Less {
            return (Self::zero(), self.clone());
        }

        // We use binary division:
        let mut q = Self::zero();
        let mut r = Self::zero();

        // Count bits
        let self_bits = self.limbs.len() * 32 - self.limbs.last().unwrap().leading_zeros() as usize;
        let other_bits = other.limbs.len() * 32 - other.limbs.last().unwrap().leading_zeros() as usize;

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
        if !q.limbs.is_empty() {
            // Apply Heh/Python sign rules for modulo/floor div
            // Wait! The division operator in Heh:
            // // is floor division. % is sign-follows-divisor.
            // But this function just returns truncating division + remainder.
        }

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
        if self.is_zero() { return self.clone(); }
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
        Self { sign: self.sign, limbs }
    }

    fn shr_1(&self) -> Self {
        if self.is_zero() { return self.clone(); }
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
        Self { sign: self.sign, limbs }
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

impl fmt::Display for BigInt {
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

impl fmt::Debug for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BigInt({})", self)
    }
}
