# RESUME.md

# Current State
Phases 0–10 are complete and merged.
- **P7** stdlib: str/list/map methods + pure modules (math, fmt, json, csv,
  hash [SHA-256 FIPS + CRC32], regex [non-backtracking], debug).
- **P8** capabilities: full `sys` (fs/env/clock/rand/net/input/args) with
  `--deny-*` flags failing closed.
- **P9** imports + net + vendoring: `use std/x`, `use "./file.heh"`,
  `use vendor/name` (cycle error E0030); `sys.net.get`; `heh get` + `heh.lock`
  SHA-256 verification (tamper = fault).
- **P10** tooling: `heh test` (runs pure `fn test_*()` in `*_test.heh`) and
  `heh fmt` (canonical, idempotent + semantics-preserving across the corpus).

The language is fully usable via the tree-walking evaluator; the whole
corpus + examples pass. See `docs/STDLIB.md` for the frozen surface.

# Next Step
The next phase is **P11 — Bytecode VM** (`src/compile.rs` + `src/vm.rs`).
1. Compile the AST to bytecode, execute on a stack VM that reuses the
   existing `Val`, bignum, builtins, and capability records (so output stays
   byte-identical).
2. `heh run` uses the VM; keep `--tree-walk` for the old path.
3. **Gate:** a differential test asserting the ENTIRE corpus is byte-identical
   under VM vs tree-walk. Benchmarks in `benches/` (local-only perf gate).

Then **P12 — v1.0 freeze** (docs/spec/README/version; publish only with
Mohamed's explicit go-ahead).
