# Current State
Phase P4 (Evaluator II) is COMPLETE and merged.
The evaluator supports functions, closures, `try`, `match`, implicit returns, and basic list iteration.
`factorial.heh` and `errors.heh` pass the test suite.

# Next Step
Start Phase P5 — Data Structures
- Implement `Val::Record` (using a map-like structure for fields) and `Val::Enum` (custom variants).
- Enhance lists: indexing `list[i]`, `list.len()`, push/pop mutations.
- Implement Maps (`Val::Map`).
- Record initialization and field access (dot syntax `obj.field`).
- Enum instantiation and variant matching in `match`.
- Gate: `tests/corpus/programs/lists.heh`, `tests/corpus/programs/records.heh`.
