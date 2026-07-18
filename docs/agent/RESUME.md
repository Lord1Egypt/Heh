# Current State
Phase P3 (Evaluator I) is COMPLETE and merged.
The evaluator supports basic arithmetic, bignums, block scopes, let/mut, if/elif/else, while, for loops with ranges, and `sys.print`.

# Next Step
Start Phase P4 — Evaluator II.
- Implement first-class functions and closures.
- Implement option narrowing (`T?` narrowing inside `if x != none` blocks).
- Implement `match` statements (value matching, enum variants, wildcards).
- Implement `try` expressions and faults.
- Gate: Execute `tests/corpus/programs/factorial.heh` (needs to be created from examples/infinity.heh probably) and `errors.heh`.
