# Toolchain evidence

This directory is the machine-readable starting point for the
[`ROADMAP_TO_10.md`](../ROADMAP_TO_10.md) gates.

- `benchmark-baseline.json` records the machine, toolchains, verified workload
  answers, seven raw samples per engine, medians, median absolute deviations,
  relative median absolute deviations,
  and VM speed ratios. Reproduce it with `cargo build --release` followed by
  `benches/run.sh --samples 7 --warmups 1 --output <path>`.
- `spec-coverage.json` conservatively inventories normative prose in `SPEC.md`.
  A mapped entry names likely current tests; all entries remain uncovered until
  a focused assertion is verified. A section-level mapping is not proof that every
  detail in that section is tested, so later phases must refine broad entries
  into focused fixtures before marking the final specification gate complete.
- `safety-type-inventory.json` records every conservatively detected production
  dynamic-type, unchecked-assumption, indexing, recursion, allocation, process,
  network, and filesystem site. False positives remain until reviewed rather than being
  silently discarded.
- `scorecard.json` summarizes coverage, unresolved boundaries, and corpus size.

Run `tools/phase0_audit.py check` after changing `src/`, `SPEC.md`, or the
corpus. Run `tools/phase0_audit.py generate` only after reviewing why evidence
changed. Generated files are deterministic except for the separately captured
benchmark baseline.

The initial run showed VM relative MAD from roughly 0.6% to 6.3% depending on the
workload, while individual outliers were wider. The protocol therefore uses
medians for comparisons, retains every raw observation, reports MAD in both
milliseconds and percent, and never accepts timing unless all three engines
produce the same answer on every sample.
