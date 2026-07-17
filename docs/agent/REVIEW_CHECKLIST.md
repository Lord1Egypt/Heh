# REVIEW CHECKLIST — the pre-merge gate

Self-review every PR against this before merging (GOAL MODE) or requesting
merge. Mohamed rates your work against the same list.

## Correctness
- [ ] `cargo test` green locally — output captured in WORKLOG (exact count).
- [ ] `cargo fmt --check` clean; CI `Tests (cargo)` green on the PR.
- [ ] New behaviour matches the cited SPEC section **exactly** (quote the
      section number in the PR body). No un-spec'd user-visible behaviour.
- [ ] Corpus grew: new `.heh` + `.out`/`.err` files covering the happy path
      AND the failure modes of what you built. No existing corpus file edited.
- [ ] Edge cases tested where relevant: empty input, unicode (𓁨 in strings!),
      huge ints (promotion boundary), deep nesting, EOF mid-token.

## Immortality & security
- [ ] `Cargo.toml` `[dependencies]` still empty. No `unsafe`.
- [ ] No `unwrap`/`expect`/`panic!` reachable from user source (grep the diff);
      internal-invariant unwraps carry a why-safe comment.
- [ ] External input treated as hostile: paths canonicalized, subprocess calls
      are arg-lists, vendored files hash-verified, deny-flags fail closed.
- [ ] No secrets, no network access outside SPEC'd capabilities.

## Craft & honesty
- [ ] Smallest correct change — no stubs, no dead code, no drive-by refactors,
      no features beyond the task.
- [ ] Diagnostics added for every new failure mode (stable code, line:col,
      caret) — a user never sees a Rust panic or a bare error string.
- [ ] `TASK_MENU.md` status updated truthfully (partial ≠ done).
- [ ] `WORKLOG.md` entry appended; `RESUME.md` will be overwritten at session
      end.
- [ ] PR body: what, SPEC sections, gate evidence, corpus files added.
