# RESUME

**Current State:**
- Finished and merged P5 (option, result, try propagation, diagnostics).
- Project is stable, `cargo test` and `cargo clippy` pass cleanly.
- `examples/errors.heh` and `examples/shapes.heh` are verified byte-exact.

**Next Step:**
- **START P6 — Static checker.**
- Create `src/check.rs` and wire it up to `src/main.rs` (the `check` subcommand).
- Implement type checking for all expressions.
- The checker runs *before* `eval` in the `run` command as well.
- Implement flow narrowing for `T?`, let reassignment checks, exhaustiveness for match, etc.
- Verify `cargo run -- check examples/shapes.heh` passes.
- Read `SPEC.md` to collect all `E00xx` type errors and write corpus error tests for them.
