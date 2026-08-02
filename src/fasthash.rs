//! A small, fast, non-cryptographic hasher for interpreter-internal maps.
//!
//! The standard library's default is SipHash-1-3: DoS-resistant, and far
//! stronger than anything needed to look up a variable name or a map key that
//! never leaves the process. Variable lookup happens on nearly every bytecode
//! instruction, so the hash cost is on the hot path.
//!
//! This is the rustc/FxHash mixing function: multiply-and-rotate per word.
//! Zero dependencies, as the project requires.
//!
//! Not used for anything security-relevant. `std/hash` (SHA-256) is separate
//! and unaffected.

use std::hash::{BuildHasherDefault, Hasher};

/// Chosen for its bit-diffusion properties; the same constant rustc uses.
const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Default, Clone, Copy)]
pub struct FastHasher {
    hash: u64,
}

impl FastHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FastHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut rest = bytes;
        while rest.len() >= 8 {
            let (chunk, tail) = rest.split_at(8);
            self.add(u64::from_le_bytes(chunk.try_into().unwrap()));
            rest = tail;
        }
        if rest.len() >= 4 {
            let (chunk, tail) = rest.split_at(4);
            self.add(u32::from_le_bytes(chunk.try_into().unwrap()) as u64);
            rest = tail;
        }
        for &b in rest {
            self.add(b as u64);
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(i as u64);
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(i as u64);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash
    }
}

pub type FastBuild = BuildHasherDefault<FastHasher>;

/// A `HashMap` using the fast hasher.
pub type FastMap<K, V> = std::collections::HashMap<K, V, FastBuild>;
