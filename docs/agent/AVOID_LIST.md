# AVOID LIST — known traps, ordered by severity

Append new traps you hit (one entry, the fix, why). Never delete entries.

## 1. Semantic guesses are forever
Every user-visible behaviour ships frozen (SPEC §1.2 item 12). If SPEC doesn't
settle it (float formatting corner, sort stability, map iteration order edge),
DO NOT pick silently — `NEEDS-MOHAMED` in WORKLOG and switch tasks. Wrong
internal code is fixable; wrong semantics are not.

## 2. The crate temptation
Bignum, regex, JSON, SHA-256 will all tempt you toward crates.io. **Adding a
crate is the one instant-fail.** Each of these is a designed task in
TASK_MENU — std-only implementations with official test vectors.

## 3. Rust fights tree-walking interpreters
Use `Rc<RefCell<Environment>>` for scopes and `Rc` for heap values
(list/map/record) — do NOT try to make the borrow checker prove interpreter
lifetimes; that path costs days. No `unsafe` as the escape hatch either.

## 4. Panics reaching users
`unwrap()`/`expect()`/indexing on anything derived from user source is a bug:
lexer, parser, checker, and interpreter must return diagnostics (SPEC §15).
Faults (§7.3) print file:line + message and exit 1 — they never backtrace.
Grep your diff for `unwrap` before every commit.

## 5. Layout algorithm edge cases (P1)
Blank lines and comment-only lines emit NOTHING (no NEWLINE). EOF emits all
pending DEDENTs. Inside `( [ {` brackets, newlines don't count. Tabs in
indentation = `E0001` even after a valid line. `\r\n` normalized before
measuring. Get these wrong and every later phase's corpus is noise.

## 6. Byte-exact means byte-exact
Corpus `.out` files include the trailing newline. Float display must be
deterministic — use Rust's `{}` (shortest round-trip) and spec-check whole
floats print without `.0` (e.g. `dist = 5` in shapes.out). Never compare
trimmed output "to be safe" — that hides real formatting bugs.

## 7. Bigint correctness corners (P3)
Negative divmod must follow Python sign rules (`-7 // 2 == -4`,
`-7 % 2 == 1`). `**` with huge exponents on non-±1/0 bases can OOM the 14 GB
laptop — cap corpus exponents (~10⁴ digits results max). Test the
Small→Big promotion boundary (i64::MAX ± 1) explicitly, both directions.

## 8. Interpolation lexes, not parses
`"a {x + 1} b"` is handled in the LEXER as string parts + embedded expression
token streams (SPEC §5.3). Trying to regex it later, or re-lexing at parse
time, creates position-info chaos. `\{` escapes a literal brace.

## 9. Subprocess hygiene (P9)
`heh get` shells out to `git`/`curl`: **arg-list APIs only**
(`Command::new("curl").arg(url)`), never a shell string — URLs are untrusted
input. Verify vendored file hashes with our own std/hash SHA-256, fail closed
on any mismatch.

## 10. This machine
WSL2, ~14 GB usable. `cargo test` is fine; avoid corpus programs that
allocate GBs (see trap 7). Classifier sometimes blocks agents self-merging
their own PRs — if `gh pr merge` is refused, leave the PR open for Mohamed
(that's the default rule anyway outside GOAL MODE).
