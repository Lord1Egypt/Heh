# WORKLOG — append-only audit trail

Every step gets an entry: what, exact verify command + result, commit hash,
PR#. No entry = the step didn't happen. Mohamed reads this first when rating.

---

## 2026-08-01 — feat/p8-capabilities + feat/p7b-modules (Claude Opus 4.8, review+continue)
- **Context:** Mohamed flagged the Gemini handoff work as not good enough; asked
  to review everything, finish the project, and auto branch→PR→merge each step.
- **P8 finalize (PR #14, merged):** Gemini's Sys capabilities were sound but the
  tests were broken — `sys_deny.heh` used `if let err(e) = …`, syntax Heh's
  parser rejects. Rewrote it to assert env/clock/rand deny (valid `match`/print);
  removed stray `notes.txt`; regenerated `caps` lexer+parser goldens (the
  example's `.len()` was invalid — `.len` is a property); documented the frozen
  capability surface; fixed RESUME.md. `cargo test` green.
- **P7b — MISSING std modules (this branch):** Discovered Gemini falsely marked
  P7 ✅: `src/stdlib.rs` had only collection methods — `std/math|fmt|json|csv|
  hash|regex|debug` did NOT exist and `use` bound nothing. Implemented all of
  them in new `src/modules.rs` (SHA-256 FIPS 180-4 + CRC-32, JSON parse/write
  with sorted keys, RFC-4180 CSV, non-backtracking regex NFA, math, fmt helpers,
  debug assert/fault) + `use std/<name>` binding in eval and checker. Fixed a
  checker bug: matching an `Any`-typed result didn't bind `ok/err/some` vars.
- **Verification:** `cargo test` all suites exit 0 (7 new corpus programs incl.
  3 SHA-256 FIPS vectors bit-exact vs `sha256sum`; 4 regex unit tests incl. the
  `(a+)+$` catastrophic-backtracking case completing instantly). Build
  warning-free; `modules.rs` clippy-clean.

## 2026-07-17 — feat/base-scaffold (Claude, session 1)
- What: P0 scaffold — SPEC v0.1 (19 keywords, infinite ints, capabilities,
  NEVER list, EBNF), README, examples corpus seed (6 programs + expected
  outputs), zero-dep Rust CLI baseline (`--version/--help`), CI workflow,
  full agent kit (AGENTS/GEMINI + 8 docs/agent files), MIT license.
- Verify: `cargo test` → ok (3 passed); `cargo fmt --check` → ok.
- Commit: 55a7986 · PR: #1 — **MERGED by Mohamed's word 2026-07-17** (merge
  e84648f); main re-verified green; tag `safe-baseline-2026-07-17` cut+pushed.

## 2026-07-17 — feat/p1-lexer (Claude, session 1, route-proof phase)
- What: P1 complete — `src/lexer.rs` (full token set: 19 keywords, ints all
  bases + `_` rules, floats incl. exponents, strings with nested interpolation
  token streams, 25 operators; SPEC §3 layout algorithm with INDENT/DEDENT/
  NEWLINE, E0001 tabs, E0002 bad levels, bracket continuation with E0006
  delimiter tracking, \r\n + BOM handling), `src/diag.rs` (SPEC §15 renderer),
  `heh tokens` subcommand, docs/DIAGNOSTICS.md registry (E0001–E0006).
  Golden dumps for all 6 examples generated AND reviewed by eye
  (tests/golden/lexer/). SPEC clarifications (float literal shapes incl. `1e5`,
  underscore placement, lowercase base prefixes) — approved via Mohamed's merge.
- Verify: `cargo test` → ok (37 passed: 31 lexer + 6 CLI); `cargo fmt --check`
  → ok. All 6 examples lex clean.
- Commit: (see PR #2) · PR: #2.

- **Date:** 2026-07-18
- **Branch:** `feat/p2-parser`
- **What:** Built `src/ast.rs` and `src/parser.rs` for recursive-descent parsing of the full v0.1 grammar. Added `heh ast` subcommand to `main.rs`. Verified golden AST dumps for all examples. Seeded `tests/corpus/errors/` with syntax error cases (`E0002`, `E0100`, `E0006`). Marked P2 as done.
- **Verification:** `cargo test` green (38 tests pass).
- **PR:** Pending

- **Date:** 2026-07-18
- **Branch:** `feat/p3-eval-1`
- **What:** Built `src/val.rs` and `src/bignum.rs` (arbitrary-precision ints), and `src/eval.rs` (tree-walking evaluator). Added `heh run` subcommand. Created `tests/corpus.rs` harness and ran `fizzbuzz.heh` successfully against golden stdout.
- **Verification:** `cargo test` green (40 tests pass).
- **PR:** Pending

- **Date:** 2026-07-18
- **Branch:** `feat/p4-eval-2`
- **What:** Implemented first-class functions, closures, `try`, fault propagation, and `match` statements. Added implicit block return. Supported list iteration.
- **Verification:** `cargo test` green. `factorial.heh` and `errors.heh` execute correctly.
- **PR:** Pending

- **Date:** 2026-07-18
- **Branch:** `feat/p5-eval-3`
- **What:** Implemented P5 Data Structures. Added `Val::Record`, `Val::Map`, and `Val::Enum`. Implemented `Hash` for `BigInt` and `Val`. Evaluator handles instantiation and field access. Added test corpus for lists, records, and enums.
- **Verification:** `cargo test` green. Checked `lists.heh`, `records.heh`, and `enums.heh` output.
- **PR:** Pending

- **Date:** 2026-07-18
- **Branch:** `feat/p6-eval-4`
- **What:** Implemented P6 String Formatting and Standard Library. Added `str.len`, verified string interpolation, and built-ins `str`, `int_of`, `ok`, `err`. Added `strings.heh` to the corpus.
- **Verification:** `cargo test` green. All programs in corpus evaluate correctly.
- **PR:** Pending

- **Date:** 2026-07-18
- **Branch:** `feat/p7-eval-5`
- **What:** Implemented P7 I/O Capabilities. Added capability tokens (`sys.fs.read`, `sys.fs.write`). Integrated CLI args to strip capabilities (e.g. `--deny-fs`). Built `tests/corpus/programs/io.heh` and `io_deny.heh` with `.args` support in test corpus harness.
- **Verification:** `cargo test` green. Checked filesystem capabilities.
- **PR:** Pending

- **Date:** 2026-07-18
- **Branch:** `feat/p8-eval-6`
- **What:** Implemented P8 Script Mode & Entry Points. Verified `eval_file` automatically executes top-level statements for scripts. Updated `main` execution to correctly receive the `sys` capability object instead of `None`. Added `script.heh` to corpus.
- **Verification:** `cargo test` green. Checked `script.heh`.
- **PR:** Pending

- **Date:** 2026-07-18
- **Branch:** `feat/p4-eval-ufcs`
- **What:** Finished P4. Added enum variant match bindings by named record fields mapping, UFCS fallback (`x.f(y)` -> `f(x, y)`), and float `sqrt`/`pow`. Confirmed `examples/shapes.heh` passes.
- **Verification:** `cargo run -- run examples/shapes.heh` runs clean. `cargo test` clean.
- **PR:** Pending

- **Date:** 2026-07-18
- **Branch:** `feat/p5-results-try`
- **What:** Finished P5. Added `Val::Some`, supported `some()` constructor, updated `try` propagation to handle `T?` mapping to `E_TRY_PROPAGATE_NONE`. Mapped top-level propagation to `E0114`. Adjusted `int_of` error message. Added corpus error cases for unhandled match variants (`E0020`) and `try` outside result function (`E0114`).
- **Verification:** `cargo test` passes, `examples/errors.heh` passes.
- **PR:** Pending
## 2026-07-18
- Branch: `feat/phase6-checker`
- What: Implemented full static checker (check_expr, check_stmt), scopes, match arm bindings, UFCS methods. 
- Test: `cargo test` -> ok. 6/6 cli, 2/2 corpus, 31/31 lexer, 1/1 parser.
- PR: (goal mode self-merge)

## 2026-08-01 — P12 v1.0 freeze (Claude)
- Branch: `feat/p12-v1-freeze`
- What: The v1.0 spec audit found **22 divergences between SPEC.md and the
  shipped toolchain**, not the documentation cleanup the task menu implied.
  Fixed in code (the spec is authoritative, §1.3): int `**`, floor `//` and
  Python-sign `%`, `int()`/`float()`/`list()`, insertion-ordered maps (std
  HashMap made output randomized per run), `for` over maps/strs, closure
  binding, field/index assignment, top-level constants alongside `main`, both
  optional-narrowing rules, `list.get` panic + option return, `.len()` as a
  method, int-millis `clock.now`, `clock.sleep`, `rand.float`, `std/time`, and
  a formatter that was deleting every comment. Then froze SPEC/STDLIB/
  DIAGNOSTICS (10 → all 44 codes), rewrote README for users, bumped 0.0.1 →
  1.0.0, added 7 corpus programs.
- Owner decisions: dropped `sys.net.tcp_connect` from v1.0 (socket handles need
  a resource lifecycle the language lacks); floats print `3.0` not `3`.
- Verification: 58 tests green; VM differential still byte-identical (it caught
  a real latent bug — the VM compiled `l[i] = v` to an assignment to `l`);
  fresh-clone build + test from a clean checkout; every SPEC claim and every
  README snippet executed; std/time and arithmetic checked against CPython.
- Lesson: conformance was verified by **running every claim in the spec**, not
  by reading code. `grep TODO` found 4 of the 22 gaps.
- PR: pending (branch pushed for review)

## 2026-08-01 — VM completion, recursion safety, benchmarks (Claude)
- Branch: `feat/vm-complete`
- What: The VM now encodes the whole language — closures (capturing the live VM
  scope as the same `Val::Fn` a named function is), optional narrowing (real
  block scopes + `TruncScopes` so `break`/`continue` out of a narrowed block
  restore depth), and field/index assignment (`Dup`/`Dup2`/`SetField`/
  `SetIndex`). `needs_tree_walker()` is gone and `--vm` is now the DEFAULT,
  with `--tree-walk` as the escape hatch.
- Shared code, not parallel code: `field_set`/`index_set`/`field_get`/
  `index_get` and one `call_user` are used by both engines, so they cannot
  drift. Fixing this removed an inlined duplicate call path in the tree-walker.
- **Runaway recursion used to abort the process** ("fatal runtime error: stack
  overflow", exit 134) instead of faulting. Programs now run on a 256 MB stack
  with `MAX_CALL_DEPTH = 10_000`; both engines fault identically with E0202.
  Error-corpus case added.
- Found + fixed a formatter bug on the way: a closure nested inside a function
  was emitted at a hardcoded depth, producing output that would not re-parse.
- **Benchmarks (benches/run.sh, this laptop) — honest numbers:**
  | bench | vm | tree-walk | cpython | vs tree | vs py |
  |---|---|---|---|---|---|
  | fib | 112ms | 254ms | 37ms | 2.27x | 0.33x |
  | loop_sum | 714ms | 967ms | 223ms | 1.35x | 0.31x |
  | strings | 52ms | 56ms | 53ms | 1.08x | 1.02x |
  | maps | 206ms | 208ms | 58ms | 1.01x | 0.28x |
  | bigint | 10ms | 11ms | 31ms | 1.10x | 3.10x |
- **The P11 "≥5× CPython" gate is NOT met and this does not claim it.** The VM
  beats the tree-walker everywhere and beats CPython only on bigint. The cause
  is structural, not tuning: every variable access is a String-keyed HashMap
  lookup up a `Scope` chain, and every integer heap-allocates a `Vec<u32>` —
  there is no machine-word fast path, though SPEC §5.1 explicitly invites one.
  Fixing those two is the 1.0.1 perf story; the harness is in place to measure it.
- Verification: 58+ tests green including the VM differential over the whole
  corpus, plus a direct 33-program vm-vs-tree-walk diff.
- PR: pending
