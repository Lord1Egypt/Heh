# RESUME — Where Heh Is Right Now

> Read this first on "continue Heh" / `/goal`; do the NEXT STEP. Overwrite it
> at the end of every session so the next one resumes in 30 seconds. Keep it
> short and true.

## Current state
- **Progress: P0 ✅ (scaffold) — P1–P12 not started.**
- **Branch to work from:** `main` (always `git pull` first).
- **Baseline:** `cargo test` green (3 CLI smoke tests). Binary `heh` builds,
  `--version` → 0.0.1.
- **Safe tag:** `safe-baseline-2026-07-17` (P0 scaffold, main @ e84648f).
- **Open PRs:** none.

## NEXT STEP (on "continue Heh" / `/goal`)
1. `git checkout main && git pull && cargo test` (must be green).
2. Start **Phase P1 — Lexer** from `TASK_MENU.md`: `src/lexer.rs` (full token
   set + the SPEC §3 layout algorithm), `heh tokens` subcommand,
   `tests/lexer.rs` with golden token dumps for all `examples/*.heh` +
   layout edge cases (tabs `E0001`, bad dedent `E0002`, brackets, EOF).
3. Follow the loop in `PLAYBOOK.md` (in `/goal` mode: `GOAL_MODE.md`). One
   small PR, `cargo test` green, then merge per the rules.

## Before you end EVERY session (mandatory)
- Overwrite "Current state" + "NEXT STEP" above to reflect reality now.
- `git add docs/agent/RESUME.md && git commit -m "docs(agent): update RESUME state"` + push.
- Refresh your own memory system: *"Heh: open /home/lordegypt/Heh, read
  docs/agent/RESUME.md, follow AGENTS.md, strongest model tier."*
