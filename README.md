# Heh 𓁨 — The Immortal Programming Language

> Named for **Heh**, the Egyptian god of infinity, who holds a palm rib notched
> with millions of years. A language designed to be **small, frozen, and
> endless**: easier than Python, fast, secure by default, with no package
> servers and no expiration date.

<p align="center">
  <img src="docs/heh-demo.gif" alt="Heh: unbounded integers, infinite ranges, and capabilities that fail closed" width="820">
</p>

```heh
# hello.heh — a complete Heh program
sys.print("Heh lives forever 𓁨")
```

```heh
# the god's own numbers: integers never overflow
fn factorial(n: int) -> int
    mut acc = 1
    for i in 1..=n
        acc *= i
    acc

sys.print(factorial(1000))        # all 2,568 digits. no BigInt import. no overflow.
sys.print(2 ** 200)               # exact. always.
```

## Why Heh exists

Every "best of all languages" project dies of feature accumulation. Heh takes
the opposite bet: **selection over addition**. Nineteen keywords. A spec that
fits in 100 pages forever. A NEVER list frozen on day one.

| Pillar | How |
|---|---|
| **Easier than Python** | 19 keywords, indentation blocks, inference everywhere except fn boundaries, one obvious way |
| **Infinite by nature** | arbitrary-precision `int` (overflow does not exist), unbounded lazy ranges `0..` |
| **Secure by default** | capability-based I/O — pure code *cannot* touch fs/net/clock; no eval, no null, no exceptions |
| **Reliable** | static types + inference, errors as values (`try` / `ok` / `err`), exhaustive `match` |
| **Immortal** | zero-dep single-binary toolchain, no package registry (vendored content-addressed imports), backward compatibility as religion after v1.0 |

## Taste of the language

```heh
use std/json

type Shape = circle(r: float) or square(side: float) or dot

fn area(s: Shape) -> float
    match s
        circle(r)
            3.141592653589793 * r * r
        square(side)
            side * side
        dot
            0.0

fn parse_age(s: str) -> int or error
    let n = try int_of(s)
    if n < 0
        return err("age cannot be negative: {n}")
    ok(n)

fn main(sys: Sys)
    let text = try sys.fs.read("shapes.json") else exit
    sys.print("total: {area(circle(r: 2.0))}")
```

No null. No exceptions. No classes. No `pip install`. No overflow. Forever.

## Getting started

Heh is one binary with no dependencies. If you have a Rust compiler, you can
build the entire toolchain:

```sh
cargo install heh-lang         # installs the `heh` command
echo 'sys.print("Heh lives forever 𓁨")' > hello.heh
heh run hello.heh
```

Building from source is the same story — one command, no dependencies to fetch:

```sh
git clone https://github.com/Lord1Egypt/Heh && cd Heh
cargo build --release          # produces target/release/heh
```

A file with no `fn main` runs top to bottom with `sys` already in scope, so
hello world really is one line. Add a `fn main(sys: Sys)` when you want an
entry point.

## The toolchain

| Command | Does |
|---|---|
| `heh run <file.heh> [args]` | run a program on the bytecode VM (`--tree-walk` for the reference evaluator) |
| `heh check <file.heh>` | parse and type-check without running |
| `heh test [path]` | run every `fn test_*()` in `*_test.heh` |
| `heh fmt [--check] <path>` | canonical formatter — no options, comment-preserving |
| `heh get <url>` | vendor a dependency and pin its hashes in `heh.lock` |
| `heh ast` / `heh tokens` | dump the parse tree or token stream |

### Capabilities, in practice

Effects reach your program through the single `Sys` value handed to `main`.
A function that never receives it cannot read a file, open a socket, or even
look at the clock — so a security review is `grep` for who takes `sys`.

```heh
fn main(sys: Sys)
    let text = try sys.fs.read("notes.txt") else exit
    sys.print(text)
```

Any capability can be revoked from the outside, and revocation fails closed:

```sh
heh run app.heh --deny-net --deny-fs      # those calls now return err(...)
```

## Status

**v1.0 — the language is frozen.** [SPEC.md](SPEC.md) is authoritative and its
surface no longer changes; the conformance corpus in `tests/corpus/` defines
what it means to be a Heh implementation. Everything was built phase by phase
(P0–P12) against that spec.

| | |
|---|---|
| Package | [`heh-lang` on crates.io](https://crates.io/crates/heh-lang) — installs the `heh` command |
| Spec | [SPEC.md](SPEC.md) — authoritative, **v1.0, frozen** |
| Standard library | [docs/STDLIB.md](docs/STDLIB.md) — the complete frozen surface |
| Diagnostics | [docs/DIAGNOSTICS.md](docs/DIAGNOSTICS.md) |
| Reference implementation | Rust, **zero crates**, single binary `heh` |
| Examples | [examples/](examples/) |
| Verification | conformance corpus + `cargo test` (CI on every PR) |

```sh
cargo test                 # the gate: must be green, always
```

## The name

*Djet* and *neheh* were the two Egyptian eternities — linear time and cyclic
time — and **Heh** personified the infinite itself. His notched palm rib was
the hieroglyph for "millions of years." That is the design target: a language
you can still run, read, and reimplement in a million years — or at least a
hundred.

## License

MIT © Mohamed Mounir (Lord1Egypt)
