# AGENTS.md — Operating Contract for the Heh Autonomous Build

> Auto-loaded by Antigravity / Gemini / Claude / opencode. Read it fully before
> touching anything. Deep detail is in [`docs/agent/`](docs/agent/). Read
> [`docs/agent/PLAYBOOK.md`](docs/agent/PLAYBOOK.md),
> [`docs/agent/TASK_MENU.md`](docs/agent/TASK_MENU.md), and
> [`docs/agent/AVOID_LIST.md`](docs/agent/AVOID_LIST.md) before your first change.

You are building **Heh 𓁨** — the immortal programming language: easier than
Python, infinite integers, capability-secure, zero-dependency Rust toolchain —
from **P0 scaffold to v1.0**, on your own. The authoritative behaviour is in
[`SPEC.md`](SPEC.md); the phase plan is in
[`docs/agent/TASK_MENU.md`](docs/agent/TASK_MENU.md). The owner is **Mohamed
(Lord1Egypt)**; he reviews your PRs and rates the work.

---

## 0. The one rule that matters most
**Never claim something works until you have run it and seen it pass.** Every
phase is verified by the **conformance corpus** (programs with byte-exact
expected output, bad programs with expected diagnostic codes) plus unit tests.
"It should work" is a failed task.

## 1. Which model to use
Use the **strongest reasoning tier** available (Gemini 3 Pro High / Claude
Fable-class) for all real work — language implementation is precision work
(layout algorithm, type narrowing, bignum arithmetic). Never a Flash-tier
model for product code.

## 2. The implementation is Rust, std only — FOREVER
- **Zero crates.** The `[dependencies]` table in `Cargo.toml` stays empty. If a
  task seems to need a crate (bignum, regex, JSON), the task IS building that
  piece from the standard library up — that's the immortality pillar.
- No `unsafe`. No `panic!`/`unwrap()`/`expect()` reachable from user input —
  user-facing failures are diagnostics (SPEC §15) or Heh faults (§7.3).
  `unwrap` is acceptable only for internal invariants with a comment saying why
  it cannot fail.

## 3. The core loop (every task)
Full version with commands in [`docs/agent/PLAYBOOK.md`](docs/agent/PLAYBOOK.md).
1. **Orient.** Read [`docs/agent/RESUME.md`](docs/agent/RESUME.md), then the
   SPEC section you're implementing, then run `cargo test` — must be green
   before you start.
2. **Pick the next task** from [`docs/agent/TASK_MENU.md`](docs/agent/TASK_MENU.md)
   (phases are ordered — do them in order). ONE small task per PR.
3. **Branch:** `git checkout main && git pull && git checkout -b feat/<short>`.
4. **Build the smallest correct thing.** Follow SPEC exactly; if SPEC is
   ambiguous, see §8 below.
5. **Verify:** grow the conformance corpus (`tests/corpus/`) for everything you
   built + unit tests for internals. `cargo test` and `cargo fmt --check` —
   all green.
6. **Commit + push + log** (see §4). Open a PR; let CI (`Tests (cargo)`) go green.
7. **Merge** per §5. Update `RESUME.md`. Move to the next task.

## 4. Save after every step
Commit the moment tests pass — don't batch, don't leave a dirty tree. Push
after each commit. Append an entry to
[`docs/agent/WORKLOG.md`](docs/agent/WORKLOG.md) (what, exact test command +
result, commit hash, PR#). No entry = the step didn't happen.

Commit trailer (use your own identity):
```
Co-Authored-By: Gemini <noreply@google.com>       # or your agent's identity
```

## 5. Merging
- This is a **private repo without branch protection** (GitHub free tier), so
  GitHub will NOT physically block a bad merge — the gate is your discipline +
  CI + Mohamed's review. Honour it exactly as if it were enforced.
- **Always** work on a branch, open a PR, and **wait for the `Tests (cargo)` CI
  check to go green** before merging. Never push product code straight to
  `main`; never merge with CI red or pending.
- **Default: open the PR, get CI green, then wait for Mohamed to say "merge."**
- **EXCEPTION — GOAL MODE** (`/goal`, see
  [`docs/agent/GOAL_MODE.md`](docs/agent/GOAL_MODE.md)): you MAY self-merge, but
  ONLY when `cargo test` is green locally AND the CI check is green AND
  self-review against `REVIEW_CHECKLIST.md` is clean. Never merge on red.
  After each goal-mode merge, cut a rolling tag `safe-<phase>-<date>`.

## 6. Non-negotiables
- ❌ Never claim done without a green `cargo test` you actually ran.
- ❌ Never add a crate. Never `unsafe`.
- ❌ Never weaken/delete a corpus file or test to make it pass; fix the root
  cause. The corpus only grows (SPEC §16).
- ❌ Never mark a phase done in `TASK_MENU.md` unless its gate genuinely
  passes. Honest progress — no inflation. Partial ≠ done.
- ❌ Never rewrite `AGENTS.md`/`SPEC.md`/contract files. Spec amendments before
  v1.0 need Mohamed's explicit approval in the PR. Only update `RESUME.md`
  (overwrite) and `WORKLOG.md` (append). Always `git pull` main before branching.
- ❌ Never self-merge outside GOAL MODE without Mohamed's explicit "merge".
- ❌ Laptop has ~14 GB usable RAM (WSL2): bounded tests (no million-element
  corpus programs), no benchmark that balloons memory.

## 7. Resume + memory (so "continue Heh" / `/goal` just works)
Read [`docs/agent/RESUME.md`](docs/agent/RESUME.md) at session start; overwrite
it at session end with the current state + exact next step, and commit it. Also
save a memory in your own system: *"Heh: open `/home/lordegypt/Heh`, read
docs/agent/RESUME.md, follow AGENTS.md, strongest model tier."* If they
disagree, RESUME.md wins (git = truth).

## 8. When unsure
If a design decision is genuinely ambiguous and not settled by `SPEC.md`, write
a `### NEEDS-MOHAMED` note in `WORKLOG.md` and work on a different unblocked
task. Never guess on language-visible semantics — every semantic decision is
frozen forever once shipped (SPEC §1.2 item 12), so wrong guesses are the one
mistake this project cannot absorb.
