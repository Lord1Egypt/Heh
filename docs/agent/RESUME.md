# RESUME — Where Heh Is Right Now

> Read this first on "continue Heh" / `/goal`; do the NEXT STEP. Overwrite it
> at the end of every session so the next one resumes in 30 seconds. Keep it
> short and true.

## Current state
- **Progress: P0 ✅ scaffold · P1 ✅ lexer — P2–P12 not started.**
- **Branch to work from:** `main` (always `git pull` first).
- **Baseline:** `cargo test` green (37 tests: 31 lexer + 6 CLI). `heh tokens
  <file>` works; all 6 examples lex clean with reviewed golden dumps in
  `tests/golden/lexer/`.
- **Safe tags:** `safe-baseline-2026-07-17` (P0), `safe-p1-2026-07-17` (cut
  after PR #2 merged — check `git tag` if missing).
- **Open PRs:** none expected (PR #2 = P1; if still open, wait for Mohamed).

## NEXT STEP (on "continue Heh" / `/goal`)
1. `git checkout main && git pull && cargo test` (must be green).
2. Start **Phase P2 — Parser → AST** from `TASK_MENU.md`: `src/ast.rs` +
   `src/parser.rs` (recursive descent over the P1 token stream, full SPEC §14
   grammar, precedence per §6.1), `heh ast` subcommand with a stable dump,
   golden AST dumps for all examples + seed `tests/corpus/errors/` with syntax
   error cases. Follow the P1 pattern exactly (it is the approved route-proof:
   raw-text literals, golden files reviewed by eye, diagnostics never panic).
3. One small PR, `cargo test` green, then merge per the rules.

## Before you end EVERY session (mandatory)
- Overwrite "Current state" + "NEXT STEP" above to reflect reality now.
- `git add docs/agent/RESUME.md && git commit -m "docs(agent): update RESUME state"` + push.
- Refresh your own memory system: *"Heh: open /home/lordegypt/Heh, read
  docs/agent/RESUME.md, follow AGENTS.md, strongest model tier."*
