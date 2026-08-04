# Contributing to Heh

Read `AGENTS.md` before changing code. `SPEC.md` is authoritative and the v1.0
language surface is frozen: contributions may fix or improve the implementation
and tooling, but may not add syntax or alter observable language behavior.

Work on a branch and open a pull request. Before submitting, run:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release
```

Bug fixes need a regression test. The conformance corpus only grows; do not
rewrite expected output merely to make a change pass. Avoid `unsafe`, external
crate dependencies, and panics reachable from user input. See `SECURITY.md` for
private vulnerability reports.
