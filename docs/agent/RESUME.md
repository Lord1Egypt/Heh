# RESUME.md

# Current State
Phases 0–9 are complete.
- **P7** stdlib: str/list/map methods **and** the pure modules std/math, fmt,
  json, csv, hash (SHA-256 FIPS + CRC32), regex (non-backtracking), debug.
- **P8** capabilities: full `sys` (fs/env/clock/rand/net/input/args) with
  `--deny-*` flags failing closed.
- **P9** imports + net + vendoring:
  - `use std/x`, `use "./file.heh"`, `use vendor/name` (namespace binding,
    cycle error E0030).
  - `sys.net.get` (HTTP via TcpStream, HTTPS via curl), `--deny-net`.
  - `heh get <url>` vendors into `vendor/` + `heh.lock` (SHA-256 per file);
    `heh run` verifies the lock and faults on any tamper.

See `docs/STDLIB.md` for the frozen stdlib + capability surface.

# Next Step
The next phase is **P10 — Tooling: heh fmt + heh test**.
1. Read `docs/agent/TASK_MENU.md` P10.
2. `heh fmt`: canonical AST formatter, idempotent + semantics-preserving
   across the whole corpus.
3. `heh test`: discover `*_test.heh`, run `fn test_*()` (pure, no Sys),
   `std/debug.assert` failures → test failed; summary + exit code.
