# Heh 𓁨 — The Immortal Programming Language

> Named for **Heh**, the Egyptian god of infinity, who holds a palm rib notched
> with millions of years. A language designed to be **small, frozen, and
> endless**: easier than Python, fast, secure by default, with no package
> servers and no expiration date.

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

## Status

**Phase P7 — Standard library done** (builtins for str, list, map). The spec
([SPEC.md](SPEC.md)) and plan are complete; the interpreter is built phase by
phase (P2 parser → P12 v1.0 freeze) by autonomous AI agents following
[AGENTS.md](AGENTS.md). Progress lives in
[docs/agent/TASK_MENU.md](docs/agent/TASK_MENU.md).

| | |
|---|---|
| Spec | [SPEC.md](SPEC.md) — authoritative, v0.1 |
| Reference implementation | Rust, **zero crates**, single binary `heh` |
| Plan (P0–P12) | [docs/agent/TASK_MENU.md](docs/agent/TASK_MENU.md) |
| Examples | [examples/](examples/) |
| Verification | conformance corpus + `cargo test` (CI on every PR) |

## Building

```sh
cargo build --release      # produces target/release/heh — the whole toolchain
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
