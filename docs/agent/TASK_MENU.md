# TASK MENU — Heh 𓁨, P0 → v1.0

Phases are **ordered** — do them top to bottom, ONE small task per PR. Each
phase lists its deliverable and its verification gate. A phase is `done` only
when its gate genuinely passes (green `cargo test`). Update the status boxes
here + the README status section as you go (honest — no inflation). The
authoritative behaviour for every phase is [`SPEC.md`](../../SPEC.md).

Legend: ⬜ todo · 🔧 in progress · ✅ done

## The conformance-corpus method (how every phase is verified)
`tests/corpus/programs/*.heh` each have a `*.out` file — running the program
must produce **byte-exact** stdout. `tests/corpus/errors/*.heh` each have a
`*.err` file — compiling/running must fail with that diagnostic code. A Rust
integration test (`tests/corpus.rs`, built in P3) walks the corpus and asserts
all of it. Every phase ADDS corpus files for what it built; the corpus never
shrinks and existing expectations are never edited to make a change pass
(SPEC §16). The `examples/*.heh` + `*.out` files join the corpus once P3 lands.
This is what makes "it works" provable instead of hopeful.

---

## P0 — Charter, spec, scaffold  ✅ (this PR)
- SPEC v0.1, examples corpus seed, zero-dep `heh` CLI baseline (`--version`,
  `--help`), CI, agent kit. **Gate:** `cargo test` green (3 CLI smoke tests).

## P1 — Lexer  ⬜
- `src/lexer.rs`: full token set (SPEC §2–§4): keywords, identifiers, int
  literals (all bases + `_`), float literals, strings **with interpolation
  segments** (`"a {expr} b"` lexes to parts), operators, comments; the layout
  algorithm of SPEC §3 (INDENT/DEDENT/NEWLINE, 4-space rule, tab error `E0001`,
  bracket-continuation); `\r\n` normalization; positions (line, col) on every
  token.
- `heh tokens file.heh` subcommand: one token per line, stable format.
- **Gate:** unit tests for every token class + golden token dumps for all
  `examples/*.heh` in `tests/lexer.rs`; layout edge cases (blank lines,
  comment-only lines, EOF dedents, bad dedent `E0002`, tabs `E0001`).

## P2 — Parser → AST  ⬜
- `src/ast.rs` + `src/parser.rs`: recursive-descent parser for the full v0.1
  grammar (SPEC §14): use/fn/type decls, statements, full expression
  precedence (§6.1), patterns, named record-construction args, anonymous fn.
- `heh ast file.heh`: stable s-expression-ish AST dump. Diagnostics with
  line:col + caret (SPEC §15) for syntax errors.
- **Gate:** golden AST dumps for all examples; `tests/corpus/errors/` seeded
  with syntax-error cases (`E0002`, unexpected token, unclosed string).

## P3 — Evaluator I: expressions, control flow, functions  ⬜
- `src/value.rs` + `src/interp.rs`: tree-walking evaluator for script mode
  (SPEC §11): int (see below)/float/bool/str values, all operators (§6.1,
  Python sign rules for `%`/`//`), let/mut/assign, if/elif/else, while, for
  over ranges & lists & strs, break/continue, fn decls, calls, closures,
  `return`, last-expression value, string interpolation, `sys.print` only.
- **`int` is arbitrary-precision from day one** — build `src/bigint.rs`
  (sign + `Vec<u64>` magnitude; add/sub/mul/divmod/pow/cmp/parse/display;
  i64 fast path enum `Small(i64) | Big(...)`). This is the infinity pillar —
  do NOT ship an i64-only placeholder.
- `heh run file.heh`. Faults (§7.3) print diagnostic + exit 1.
- **Gate:** `tests/corpus.rs` harness lands; `hello`, `fizzbuzz`, `infinity`
  examples pass byte-exact; bigint unit tests incl. factorial(1000),
  `2**200`, negative divmod, `1_000_000` literals.

## P4 — Records, enums, match, UFCS  ⬜
- `type` records (named-field construction, field get/set) and enums;
  `match` with literal/binding/wildcard patterns on enums, int, str, bool;
  runtime exhaustiveness fault until P6 makes it static; UFCS dispatch
  (`x.f(y)` → `f(x, y)` when no builtin method matches).
- **Gate:** `shapes` example passes byte-exact; corpus grows with
  match/record/UFCS programs + error cases.

## P5 — option, result, try, diagnostics polish  ⬜
- `T?`/`some`/`none`, `T or error`/`ok`/`err`/`error(msg)`, `try` propagation,
  `try ... else exit`, match on `ok/err/some/none`; builtin `int_of(str)`
  (error message: `not an integer: "<s>"`).
- Diagnostics: every runtime fault and compile error shows source line +
  caret + stable code (SPEC §15).
- **Gate:** `errors` example passes byte-exact; corpus error cases for `try`
  outside result-returning fn, unhandled variants.

## P6 — Static checker  ⬜
- `src/check.rs`: types for all expressions; required annotations at fn
  boundaries, inference for locals; no implicit coercion (`E0040`);
  conditions must be bool; exhaustive `match` (`E0020`); flow narrowing for
  `T?` per SPEC §6.4; `let` reassignment (`E0010`); unknown names/fields.
  Runs before execution in `heh run`, alone in `heh check`.
- **Gate:** big negative corpus (each bad program → expected code in `.err`);
  all existing corpus still green (checker must accept every good program).

## P7 — Stdlib: builtin methods + pure modules  ⬜
- Complete builtin method sets for str/list/map (SPEC §12 list), `list.sort`
  stable, `map` insertion-ordered. Modules: `std/math`, `std/fmt`,
  `std/json` (parser + writer, zero-copy not required), `std/time` (pure
  parts), `std/csv`, `std/hash` (SHA-256 per FIPS 180-4 + CRC32, with
  official test vectors), `std/regex` (non-backtracking NFA — no
  catastrophic blowup), `std/debug` (`fault`, `assert`).
- Write `docs/STDLIB.md` with the frozen signatures as you go.
- **Gate:** corpus programs exercising every module; SHA-256 FIPS vectors;
  regex unit tests incl. pathological patterns completing fast.

## P8 — Capabilities: the full Sys  ⬜
- `sys.fs` (read/read_bytes/write/append/exists/list_dir/remove — resolve
  paths, no traversal outside cwd unless absolute path given by user),
  `sys.env`, `sys.clock`, `sys.rand` (OS entropy via /dev/urandom, getrandom
  syscall not required), `sys.args`, `sys.input`; `fn main(sys: Sys)` entry;
  `--deny-fs/--deny-net/--deny-env/--deny-clock/--deny-rand` flags — denied
  ops return `err("capability denied: <cap>")`, fail closed.
- **Gate:** corpus with tempdir-driven fs programs; deny-flag tests assert
  the err value; `caps` example runs.

## P9 — sys.net + imports & vendoring  ⬜
- `sys.net.get` (HTTP/1.1 over std TcpStream; **https via shelling out to
  `curl` if available, else clean err** — std has no TLS; document).
- `use std/x`, `use "./file.heh"` (namespace binding, cycle error `E0030`),
  `use vendor/name`; `heh get <url>` (git or curl subprocess — arg-list, never
  shell string) vendoring into `vendor/` + `heh.lock` with SHA-256 of every
  file (via std/hash); verify lock on every run, mismatch = fault.
- **Gate:** multi-file corpus programs; lock tamper test (flip one byte →
  fault); cycle test.

## P10 — Tooling: heh fmt + heh test  ⬜
- `heh fmt`: canonical formatter from the AST (4-space indent, stable spacing,
  no options). Idempotent (`fmt(fmt(x)) == fmt(x)`) and semantics-preserving
  (AST-equal before/after) across the whole corpus.
- `heh test`: discovers `*_test.heh`, runs `fn test_*()` (pure, no Sys),
  `std/debug.assert` failures → test failed; summary + exit code.
- **Gate:** fmt round-trip over entire corpus; a sample `*_test.heh` suite
  passing + one deliberately failing (exit 1) in a unit test.

## P11 — Bytecode VM  ⬜
- `src/compile.rs` + `src/vm.rs`: compile AST → bytecode, stack VM;
  `heh run` uses the VM, `--tree-walk` keeps the old path for differential
  testing. `benches/` corpus (fib, string churn, map churn, bigint) with a
  small harness comparing against `python3` where available.
- **Gate:** ENTIRE corpus byte-identical under VM vs tree-walk (differential
  test in CI); VM ≥ 5× CPython on at least 3 of 4 benchmarks locally
  (record numbers in WORKLOG — benchmark gate is local, not CI).

## P12 — v1.0 freeze  ⬜
- Spec audit: SPEC.md updated to match shipped reality **exactly** (every
  deviation reconciled — with Mohamed's approval per amendment), page budget
  confirmed; `docs/STDLIB.md` complete; README rewritten for users;
  conformance corpus labelled v1.0; `heh --version` → 1.0.0.
- Release prep ONLY (binaries build, tarball, notes drafted). **The actual
  GitHub Release / any publish needs Mohamed's explicit go-ahead — STOP and
  ask.**
- **Gate:** full `cargo test` green; fresh-clone build works; a NEEDS-MOHAMED
  note listing what's ready to publish.
