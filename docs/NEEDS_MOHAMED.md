# NEEDS-MOHAMED — v1.0.0 publish checklist

**v1.0.0 SHIPPED 2026-08-01** with Mohamed's explicit approval.

## Done

| # | Action | State |
|---|---|---|
| 1 | Merge `feat/p12-v1-freeze` | ✅ PR #22 squash-merged, CI green |
| 2 | Tag `v1.0.0` | ✅ pushed, on `main` @ 85b9747 |
| 3 | GitHub Release v1.0.0 | ✅ live, notes from `docs/RELEASE_NOTES_v1.0.0.md` |
| 4 | Release assets | ✅ linux x86_64 binary (1.1 MB, stripped), source tarball, SHA256SUMS |

Assets were re-downloaded from the release and verified: checksums match, the
binary runs, and the source tarball builds and passes all 58 tests.

## Still open — needs a separate go-ahead

| # | Action | State | Notes |
|---|---|---|---|
| 5 | crates.io publish | **not attempted** | `Cargo.toml` has name/version/license/description. Needs `cargo publish` + a token. |
| 6 | macOS / Windows binaries | not built | Needs cross-compilation targets installed. |
| 7 | Repo About / topics | unchanged | Web-UI only. |
| 8 | Demo GIF for README | not made | Project standard prefers VHS. |

## Verified before handing this over

- `cargo test --release` — 58 tests green.
- Fresh `git clone` → `cargo build --release` → `cargo test` in a clean target
  directory: builds and passes with **zero crates**.
- `heh --version` → `1.0.0`.
- Every signature in `docs/STDLIB.md` executed; `std/time` and the arithmetic
  sign rules cross-checked against CPython.
- Every code snippet in `README.md` executed, including the `factorial(1000)`
  digit count.
- All 44 diagnostic codes that ship are documented (was 10).
- `heh fmt` is comment-preserving and idempotent across the whole corpus.

## Decisions you already made in this session

- `sys.net.tcp_connect` dropped from the frozen v1.0 surface.
- Floats print with a decimal point (`3.0`, not `3`).

## Known, documented, not blocking

- The bytecode VM (`--vm`, opt-in) cannot encode closures, optional narrowing,
  or field/index assignment; those programs run on the tree-walker instead.
  This is explicit in `needs_tree_walker()` — output is identical either way,
  and the differential test enforces it.
- Large floats print in full decimal (`6.02e23` → `601999999999999995805696.0`)
  rather than exponent form. Exact and round-trips; pinned in SPEC §5.2 so no
  threshold rule has to be frozen.
- No demo GIF in the README yet (project standard prefers VHS).
