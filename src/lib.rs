//! Heh 𓁨 — the immortal programming language. Reference implementation.
//! Rust standard library only, forever (SPEC §1.3).

pub mod ast;
pub mod bignum;
pub mod check;
pub mod compile;
pub mod diag;
pub mod eval;
pub mod fmt;
pub mod lexer;
pub mod modules;
pub mod parser;
pub mod stdlib;
pub mod val;
pub mod vm;
