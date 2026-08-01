# NEEDS-MOHAMED — v1.0.0 publish checklist

Everything below is **prepared and verified locally but deliberately not
published.** Publishing needs your explicit go-ahead, one item at a time.

## Ready, waiting on you

| # | Action | State | Notes |
|---|---|---|---|
| 1 | Merge `feat/p12-v1-freeze` | PR open, tests green | Two commits: P12a (implementation fixes), P12b (freeze). |
| 2 | Tag `v1.0.0` | not created | Tag after merge, on `main`. |
| 3 | GitHub Release v1.0.0 | notes drafted | `docs/RELEASE_NOTES_v1.0.0.md`. |
| 4 | Release binaries | buildable, not built | `cargo build --release` → 1.3 MB single binary. Cross-compiling for macOS/Windows needs targets installed. |
| 5 | crates.io publish | **not attempted** | `Cargo.toml` has name/version/license/description. Would need `cargo publish` and a token. Say the word if you want it. |
| 6 | Repo About / topics | unchanged | Web-UI only. |

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
