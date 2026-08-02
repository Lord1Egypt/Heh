# Changelog

The v1.0 language surface is frozen (SPEC §1.2 item 12): code written for any
version below runs unchanged on every later one. Releases after v1.0 change the
implementation, never the language.

Full notes for each release are on the
[Releases page](https://github.com/Lord1Egypt/Heh/releases).

## v1.0.3 — 2026-08-02

A correctness and infrastructure release. No language change.

- **Fixed: the crate did not build on the Rust version it claimed to support.**
  `Cargo.toml` promises `rust-version = "1.70"`, but the code used
  `std::iter::repeat_n` (stable only since 1.82) and compared `ExitCode`
  values. Anyone on Rust 1.70–1.81 running `cargo install heh-lang` hit a
  compile error. Both are replaced, and the claim is now proven on every PR by
  a CI job that builds with a real 1.70 toolchain.
- CI runs the full suite on **macOS and Windows** as well as Linux — release
  binaries ship for all three, and only one had ever been tested. Doing so
  found three Windows-only harness bugs (CRLF in byte-exact fixtures, CRLF from
  CPython in the arithmetic differential, and malformed `file://` URLs), all
  fixed. `.gitattributes` now pins LF for sources and fixtures.
- CI also enforces `cargo clippy -- -D warnings`; the tree is clippy-clean.
- Documentation pruned from 18 markdown files to 6: the agent handoff kit and
  per-release notes files were scaffolding for a build that is finished. Their
  content lives on in `AGENTS.md` and `CHANGELOG.md`. Fixed a dangling
  `docs/agent/TASK_MENU.md` reference in `heh --help`.

## v1.0.2 — 2026-08-02

Performance. Roughly twice as fast, with no change in behaviour.

- **Integers use a machine word until they need more.** Every integer used to
  heap-allocate a vector of limbs, so a loop counter cost an allocation per
  step. SPEC §5.1's implementation note asks for exactly this fast path.
  Integers remain unbounded.
- **A fast hasher for interpreter-internal maps.** Variable lookup was running
  SipHash on nearly every instruction. `std/hash` (SHA-256) is untouched.
- **Binding a name no longer allocates.** Variable names are refcounted.
- Integer arithmetic is now verified against CPython by a differential test of
  ~13,000 comparisons across the machine-word and limb boundaries. It caught a
  real bug before release: `1 // -2` returned `1` instead of `-1`.
- Fixed: `sys.rand.bytes` and `sys.rand.int` read only the lowest limb of their
  arguments, so large values silently produced wrong bounds.

| benchmark | v1.0.1 | v1.0.2 | vs CPython |
|---|---|---|---|
| fib | 104ms | 72ms | 0.35× → 0.62× |
| loop_sum | 751ms | 373ms | 0.27× → 0.57× |
| strings | 58ms | 41ms | 0.91× → **1.15×** |
| maps | 205ms | 108ms | 0.38× → 0.51× |
| bigint | 11ms | 10ms | **3.00×** |

## v1.0.1 — 2026-08-02

- **The bytecode VM is the default** (`--tree-walk` selects the reference
  evaluator) and now covers the whole language: closures, optional narrowing,
  and field/index assignment all compile. Output stays byte-identical to the
  tree-walker across the conformance corpus.
- **Runaway recursion is a fault, not a crash.** It used to abort the process
  with a native stack overflow; both engines now report `E0202`. Legitimate
  deep recursion (9,000 frames) works.
- Added `benches/`, each benchmark paired with a CPython twin whose answer must
  match before a timing is reported.
- Fixed: `heh fmt` mangled a closure nested inside a function, producing output
  that would not re-parse.
- First crates.io publish, as `heh-lang` — the name `heh` was taken in 2022 by
  an unrelated project. The installed command is still `heh`.

## v1.0.0 — 2026-08-01

The language is frozen. 19 keywords, an eight-page spec against a hundred-page
budget, and a NEVER list fixed on day one.

Reaching it required reconciling **22 divergences between SPEC.md and the
shipped toolchain**, found by executing every claim in the spec. Among them:
`int ** int` was unimplemented, so the spec's own `2 ** 200` example failed;
maps used a hash map with per-run randomized iteration order; `heh fmt` deleted
every comment in a file; `list.get(0)` panicked the interpreter; and top-level
constants were skipped whenever a `fn main` existed.

Two deliberate amendments were made before the freeze: `sys.net.tcp_connect`
was dropped (a socket handle needs a resource lifecycle the language does not
have), and floats now print with a decimal point so `3.0` is never mistaken for
the integer `3`.
