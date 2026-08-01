# RESUME.md

# Current State
Phase 8 (Capabilities: the full Sys) is complete. `sys.fs` (read/read_bytes/
write/append/exists/list_dir/remove with `..` traversal guards), `sys.env`,
`sys.clock`, `sys.rand`, `sys.args`, `sys.input`, and the
`--deny-fs/-net/-env/-clock/-rand` flags all fail closed. Corpus programs
`sys_fs` and `sys_deny` and the `caps` example pass. See `docs/STDLIB.md`
"Capabilities" for the frozen surface.

# Next Step
The next phase is **P9 — sys.net + imports & vendoring**.
1. Read `docs/agent/TASK_MENU.md` P9.
2. `sys.net.get` (HTTP/1.1 over std TcpStream; https by shelling out to `curl`).
3. `use std/x` / `use "./file.heh"` namespace binding (cycle error E0030).
4. `heh get <url>` vendoring into `vendor/` + `heh.lock` (SHA-256 per file,
   verified on every run; mismatch = fault).
