# RESUME.md

# Current State
**Phases 0–12 are complete. v1.0.0 is RELEASED and the spec is frozen.**

GitHub Release v1.0.0 is live (stripped linux x86_64 binary, source tarball,
SHA256SUMS — all re-downloaded and verified). `main` is clean, nothing open.

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

1. **Publishing is finished.** `heh-lang` 1.0.1 is live on crates.io
   (<https://crates.io/crates/heh-lang>) — the crate is `heh-lang` because
   `heh` was taken in 2022 by an unrelated hex editor; `[[bin]]`/`[lib]` keep
   the command and library named `heh`, so `cargo install heh-lang` gives you
   `heh`. Publishing a future version is just `cargo publish` from `main`
   after bumping `Cargo.toml`.
2. **Performance — two of the three structural costs are now fixed.**
   `int` has a machine-word fast path (SPEC §5.1), the interpreter's internal
   maps use a fast hasher, and variable names are `Rc<str>` so binding does not
   allocate. That roughly doubled the VM: it now runs at ~0.5x–1.15x of CPython
   (was ~0.3x) and is faster than CPython on `strings` and `bigint`.

   **Still open, and it is the big one:** locals resolve by *name* through a
   `Scope` chain on every access. A real VM assigns frame slots at compile time
   and indexes an array. Doing that here means a resolver pass handling
   shadowing across block scopes, `match` arm bindings, narrowing rebinds, and
   closure upvalues — a subsystem, not a patch. It is the remaining path to
   the ≥5x CPython target, and honestly 5x on loop-heavy code may need more
   than slots alone.

   Do not bother re-trying a linear-scan `Vec` for small scopes; it was
   measured against the hash map and came out within noise (see WORKLOG).
