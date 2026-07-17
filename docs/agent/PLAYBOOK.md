# PLAYBOOK — exact commands for every step (verified on this machine)

Repo: `/home/lordegypt/Heh` · Toolchain: cargo 1.94.1 / rustc 1.94.1 (stable,
on PATH) · Remote: `https://github.com/Lord1Egypt/Heh` (private) · CI check
name: **`Tests (cargo)`**.

## 0. Session start (always)
```sh
cd /home/lordegypt/Heh
git checkout main && git pull
cargo test                      # MUST be green before you start anything
cat docs/agent/RESUME.md        # current state + next step
```

## 1. Start a task
```sh
git checkout -b feat/<short-name>       # e.g. feat/p1-lexer-layout
```
Read the SPEC section for the phase (TASK_MENU names it) BEFORE writing code.

## 2. The verification gate (before every commit)
```sh
cargo test                      # all tests, including corpus harness (P3+)
cargo fmt --check               # CI enforces this; run `cargo fmt` to fix
```
Both must exit 0. To run one test: `cargo test <name>`. To see program output
while debugging: `cargo run -- run examples/hello.heh`.

## 3. Corpus workflow (P3+)
- Add a program: `tests/corpus/programs/<name>.heh` + `<name>.out`
  (byte-exact expected stdout, trailing newline included).
- Add an error case: `tests/corpus/errors/<name>.heh` + `<name>.err`
  (first line = expected diagnostic code, e.g. `E0020`).
- `cargo test corpus` runs the harness. NEVER edit an existing `.out`/`.err`
  to make a change pass — that's a semantics change; see AGENTS.md §8.

## 4. Commit + push + log (after every green step)
```sh
git add -A
git commit -m "feat(p1): <what>" -m "Co-Authored-By: <your agent identity>"
git push -u origin HEAD
```
Then append to `docs/agent/WORKLOG.md`:
```
## <date> — <branch>
- What: <one line>
- Verify: `cargo test` → ok (NN passed); `cargo fmt --check` → ok
- Commit: <hash> · PR: #<n>
```

## 5. Open the PR
```sh
gh pr create --title "P<N>: <what>" --body "<deliverable, gate evidence, corpus files added>

🤖 Generated with an autonomous agent per AGENTS.md"
gh pr checks --watch            # wait for 'Tests (cargo)' green
```

## 6. Merge (rules in AGENTS.md §5 — default is WAIT for Mohamed)
GOAL MODE only, after local green + CI green + clean self-review:
```sh
gh pr merge <N> --merge --delete-branch
git checkout main && git pull
git tag -a safe-p<N>-$(date +%F) -m "phase P<N> merged green" && git push origin --tags
```

## 7. Session end (mandatory)
Overwrite `docs/agent/RESUME.md` (state + exact next step), commit it (on main
if it's the only change and main is where you are — RESUME/WORKLOG updates are
the one allowed direct-to-main commit — otherwise on your branch), push.
