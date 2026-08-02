# AGENTS.md — working on Heh

Auto-loaded by Claude Code, Gemini/Antigravity, Cursor, and opencode. Read it
before changing anything.

Heh 𓁨 is a small programming language: easier than Python, integers that never
overflow, capability-secure I/O, zero dependencies. **It reached v1.0 and its
surface is frozen** — [`SPEC.md`](SPEC.md) is authoritative, and the
implementation follows the spec rather than the other way around.

Owner: **Mohamed Mounir (Lord1Egypt)**, who reviews every PR.

---

## The one rule that matters most

**Never claim something works until you have run it and watched it pass.**
"It should work" is a failed task. The conformance corpus is the judge:
`tests/corpus/programs/*.heh` with byte-exact expected stdout, and
`tests/corpus/errors/*.heh` with expected diagnostic codes.

This is not decoration. The v1.0 spec audit found **22 places where the shipped
toolchain contradicted its own spec** — including the spec's headline example
`2 ** 200` failing outright, `heh fmt` deleting every comment in a file, and
maps iterating in a different order on every run. Every one was found by
*executing* the spec's claims. `grep TODO` found four of them.

So: to check conformance, run the document, not the code.

---

## Ground rules

1. **Zero dependencies, forever.** `[dependencies]` in `Cargo.toml` stays
   empty. Bignum, regex, JSON, SHA-256, and the HTTP client are all std-only
   by design. Adding a crate is the one instant-fail.
2. **Never guess user-visible behaviour.** Everything shipped is frozen
   (SPEC §1.2 item 12). If the spec doesn't settle something — a float
   formatting corner, sort stability, an iteration-order edge — ask Mohamed
   rather than picking silently. Wrong internals are fixable; wrong semantics
   are forever.
3. **No panics on user input.** `unwrap()`/`expect()`/raw indexing on anything
   derived from source is a bug. Lexer, parser, checker, and both engines
   return diagnostics (SPEC §15); faults print `file:line` and exit 1 without a
   backtrace (§7.3). Grep your diff for `unwrap` before committing.
4. **Branch → PR → merge.** Never push to `main`. CI (`cargo fmt --check`,
   build, `cargo test`) must be green.
5. **The corpus only grows.** Never edit an expected-output file to make a
   change pass — if output changed, understand exactly why first.

---

## Layout

| Path | What |
|---|---|
| `SPEC.md` | The language. Authoritative, frozen at v1.0. |
| `src/lexer.rs` `parser.rs` `check.rs` | Front end: layout tokens, AST, static checks |
| `src/eval.rs` | Tree-walking evaluator — the *reference* semantics |
| `src/compile.rs` `vm.rs` | Bytecode compiler + stack VM (the default engine) |
| `src/bignum.rs` | Unbounded `int`: machine word, promoting to limbs |
| `src/modules.rs` `stdlib.rs` | The eight std modules and builtin methods |
| `src/fmt.rs` | `heh fmt` — canonical, comment-preserving |
| `tests/corpus/` | The conformance definition |
| `benches/` | Benchmarks, each paired with a CPython twin |

Both engines must agree. `tests/vm.rs` diffs VM against tree-walker output
across the whole corpus; `tests/bignum_vs_python.rs` diffs integer arithmetic
against CPython across the machine-word boundaries.

---

## Traps that have already bitten someone

- **Rust fights tree-walking interpreters.** Scopes are `Rc<RefCell<Scope>>`
  and heap values are `Rc`. Do not try to make the borrow checker prove
  interpreter lifetimes, and do not reach for `unsafe`.
- **Layout is subtle.** Blank and comment-only lines emit no tokens. EOF emits
  every pending DEDENT. Inside `( [ {` newlines don't count. A tab in
  indentation is `E0001` even after valid lines.
- **`div_euclid` is not Heh's `//`.** Rust keeps the remainder non-negative;
  SPEC §6.1 wants the divisor's sign. `1 // -2` is `-1`.
- **Unary `-` binds tighter than `**`** (SPEC §6.1), so `-2 ** 4` is `16` —
  the opposite of Python. `heh fmt` keeps the parentheses for this reason.
- **Diagnostic codes are append-only.** Never reuse or repurpose one; add to
  `docs/DIAGNOSTICS.md`.
- **The VM shares the evaluator's helpers on purpose** (`call_user`,
  `field_get`/`field_set`, `index_get`/`index_set`). Duplicating logic across
  the two engines is how they silently drift.

---

## Where things stand

v1.0.2 is released and published to crates.io as `heh-lang`. All twelve build
phases are complete and nothing is open.

**The one known gap is performance.** The VM runs at roughly 0.5×–1.15× of
CPython — faster on strings and bignum, slower on loop-heavy code; the original
target was ≥5×. Two of the three structural costs are fixed: integers have a
machine-word fast path, and the interpreter's internal maps use a fast hasher
with refcounted names.

What remains: **locals still resolve by name through a scope chain on every
access.** A mature VM assigns each local a frame slot at compile time and
indexes an array. Doing that here means a resolver pass handling shadowing
across block scopes, `match` arm bindings, narrowing rebinds, and closure
capture — a subsystem, not a patch. Rushing it into a released runtime would
ship subtle scoping bugs.

Measured and rejected: replacing the scope hash map with a linear scan for
small scopes came out within noise. Don't repeat it.

---

## Commands

```sh
cargo build --release        # target/release/heh
cargo test                   # the gate — must be green
cargo fmt --check            # CI runs this first
./benches/run.sh             # local perf; uses python3 for comparison
./target/release/heh run examples/hello.heh
```
