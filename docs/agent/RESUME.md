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

1. **crates.io — the only open item, and it is blocked on Mohamed.**
   `cargo publish` is dry-run clean but crates.io rejects the upload:
   *"A verified email address is required."* He must set and confirm an email
   at <https://crates.io/settings/profile>, then run `cargo publish` from
   `main`. Note the crate is **`heh-lang`** — `heh` was taken on crates.io in
   2022 by an unrelated hex editor; `[[bin]]`/`[lib]` keep the command and
   library named `heh`.
2. **Performance is the real open engineering work.** `benches/run.sh` exists
   and the numbers are recorded in the worklog: the VM beats the tree-walker
   everywhere (1.01x–2.27x) but reaches only 0.28x–1.02x of CPython, winning
   only on bigint. The P11 target was ≥5x CPython. The cause is structural, not
   tuning:
   - every variable access is a String-keyed HashMap lookup up a `Scope` chain
     (a real VM resolves locals to slot indices at compile time), and
   - every integer heap-allocates a `Vec<u32>`; there is no machine-word fast
     path, though SPEC §5.1 explicitly invites one.
   Fixing those two is the next milestone, and the harness can prove it.
