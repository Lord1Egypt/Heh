# GOAL MODE — Autonomous P1→v1.0 Build

Entered with the **`/goal`** command. You build Heh phase by phase on your own
— build, verify, self-review, self-merge — looping until v1.0 or a STOP
condition. Mohamed reviews your trail between phases and can interrupt anytime.
Autonomy is earned by discipline; the rails below are absolute.

## The loop (repeat until STOP)
1. `git checkout main && git pull && cargo test` (green, else STOP + log).
2. Read `RESUME.md` + `TASK_MENU.md`. **STOP-check** (below).
3. Pick the **next ordered phase task** from `TASK_MENU.md`. Branch off main.
4. Read the relevant `SPEC.md` sections. Build the smallest correct thing
   (Rust std only, zero crates, no unsafe).
5. **Verify:** grow the conformance corpus (byte-exact `.out`, diagnostic-code
   `.err`) + unit tests for internals. `cargo test` + `cargo fmt --check` —
   all green. Capture the output.
6. If red: fix the ROOT CAUSE. Max **3** attempts per task; if still failing,
   abandon the branch, write a `### NEEDS-MOHAMED` note in `WORKLOG.md`, STOP.
7. Self-review against `REVIEW_CHECKLIST.md`. Update `TASK_MENU.md` + README
   status (honestly). Commit + push + `WORKLOG.md` entry.
8. Open PR; wait for the `Tests (cargo)` CI check to be **green**.
9. **Self-merge ONLY if:** local `cargo test` green AND CI green AND
   self-review clean. Then `gh pr merge <N> --merge --delete-branch`, sync
   main, and cut a tag `safe-p<N>-<YYYY-MM-DD>`.
10. Overwrite `RESUME.md`; loop.

> This private repo has **no branch protection** (free tier), so GitHub won't
> physically block a bad merge. The rails are enforced by your discipline, CI,
> and Mohamed's review — treat them as absolute anyway.

## HARD RAILS (never violate)
- ❌ Never merge on a failing test or red/pending CI. Red = fix or STOP.
- ❌ Never add a crate. Never `unsafe`. Never rewrite `AGENTS.md`/`SPEC.md`/
  contract files (only `RESUME.md` overwrite + `WORKLOG.md`/`AVOID_LIST.md`
  append). Spec amendments = NEEDS-MOHAMED, not self-service.
- ❌ Never edit an existing corpus `.out`/`.err` to make a change pass; never
  weaken or delete a test. The corpus only grows.
- ❌ Never inflate `TASK_MENU.md` (partial ≠ done).
- ❌ Never force-push, rewrite main history, or delete tags.
- ❌ Never blow past the laptop's ~14 GB (bounded corpus programs; see
  AVOID_LIST trap 7).
- ❌ One phase task per PR — never batch unrelated changes.
- ❌ Never publish anything (releases, packages, pages) — v1.0 release prep
  STOPS for Mohamed's explicit go-ahead.

## STOP conditions (halt, summarize in WORKLOG + RESUME, wait for Mohamed)
- All phases P0–P12 are ✅ (**v1.0 ready — write a final summary**).
- A task fails after 3 attempts, or CI is red for a cause you can't fix at
  the root.
- A language-semantics decision is not settled by `SPEC.md` (log
  `NEEDS-MOHAMED` — semantic guesses are forbidden, AVOID_LIST trap 1).
- Any action would require something on the HARD RAILS list.

## Recovery net
Every merged phase leaves a `safe-p<N>-*` tag. Worst case, Mohamed resets to a
tag. That safety is why autonomy is allowed — not a reason to skip a gate.
