# Roadmap execution checkpoint

Last updated: 2026-08-04

This file is the durable handoff for continuing `docs/ROADMAP_TO_10.md`.
The language surface in `SPEC.md` remains frozen.

## Completed and merged

- PR #41: Phase 0 evidence baseline and reproducible audit tooling.
- PRs #42–#47: typed capabilities, builtins, modules, constructors, closures,
  higher-order calls, and named calls.
- PR #48: function completion and lexical control-flow checking.
- PR #49: confined `Ty::Any` to documented JSON/printing boundaries and added
  precise structural record values.
- PR #50: exact, cross-platform diagnostic snapshots with explicit
  compile-versus-runtime phase and exit-code assertions.

## Current completed increment

- Every diagnostic code emitted by `src/check.rs` has a minimal negative corpus
  fixture with exact stderr, source location, exit code, and compile-phase
  verification through both `heh check` and `heh run`.
- The corpus test reads the checker diagnostic registry from source and fails if
  a code is added or removed without corresponding coverage.
- Polymorphic builtin arity failures now emit one diagnostic instead of falling
  through and emitting a duplicate.

Local verification completed for this increment:

```text
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release
./tools/phase0_audit.py generate
./tools/phase0_audit.py check
git diff --check
```

## Exact resume point

1. Confirm the checker-corpus PR is merged after Linux, macOS, Windows, MSRV,
   lint, benchmark-smoke, and security checks pass.
2. Audit the remaining Phase 1 exit gates directly:
   - prove `heh check` rejects every locally knowable error;
   - confirm the original corpus and VM/tree-walker differential are unchanged;
   - record Phase 1 completion only if all four exit-gate statements have direct
     evidence.
3. Begin Phase 2 with the existing safety inventory in
   `docs/evidence/safety-type-inventory.json`; classify user-reachable panics,
   raw indexing, conversions, recursion, and input-driven allocation before
   changing code.

Do not mark the overall roadmap complete: Phases 2–7 and the final
requirement-by-requirement audit remain outstanding.
