# RESUME.md

# Current State
**Phases 0–12 are complete. The language is at v1.0 and the spec is frozen.**

`heh --version` → 1.0.0. Full `cargo test` green (58 tests). Fresh-clone build
verified (zero crates, ~1.3 MB binary).

## What P12 actually turned out to be

The v1.0 spec audit was not a documentation pass — it found **22 places where
the shipped toolchain did not match SPEC.md**, several in headline features.
The charter says the implementation follows the spec (§1.3), so they were
fixed in code, not written off in the document. In short:

- `int ** int` was unimplemented — the spec's own `2 ** 200` example failed.
- `//` truncated instead of flooring; `%` used Rust's sign rules, not Python's.
- `int()`, `float()`, `list()` did not exist.
- Maps were a std HashMap, so iteration order was **randomized per run**.
- `for` rejected maps and strs; closures could not be bound (`let f = fn(...)`);
  `p.x = v` / `l[i] = v` were unimplemented (and the VM compiled them to an
  assignment to the *base name*, silently corrupting it).
- Top-level `let` constants were skipped when a `fn main` existed, so
  `sys.print(NAME)` printed the string `"NAME"`.
- Neither optional-narrowing rule existed. `list.get(0)` panicked the
  interpreter. `.len()` could not be called (`.len` was a property).
- `sys.clock.now()` returned a float of seconds instead of int millis;
  `clock.sleep`, `rand.float`, and the whole `std/time` module were missing.
- **`heh fmt` deleted every comment in the file.**

All fixed, each with corpus coverage. Two divergences were resolved as spec
amendments with Mohamed's approval: `sys.net.tcp_connect` is dropped from v1.0
(a socket handle needs a resource lifecycle the language does not have), and
floats now print with a decimal point (`3.0`, not `3`).

## Method worth reusing

The gaps were found by **executing every claim in SPEC.md**, clause by clause,
rather than reading the code. Grepping for `TODO` found 4 of the 22; running
the spec's own examples found the rest. If a future phase claims conformance,
run the document.

# Next Step
Nothing is open. The remaining work is optional and needs an explicit
go-ahead where noted:

1. **Publishing** — NOT done, needs Mohamed's explicit approval:
   GitHub Release v1.0.0, release binaries/tarball, crates.io. Draft notes are
   in `docs/RELEASE_NOTES_v1.0.0.md`.
2. **VM follow-ups** (from P11) — perf benchmarks in `benches/`, and making
   `--vm` the default after soak testing. Note that `needs_tree_walker()` in
   `src/compile.rs` now routes three construct families to the tree-walker
   (closures, optional narrowing, field/index assignment); making the VM the
   default means encoding those first.
3. **A demo GIF** for the README (project standard: prefer VHS).
