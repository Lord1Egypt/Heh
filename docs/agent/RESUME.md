# Current State
Phase P2 (Parser -> AST) is COMPLETE and merged.
The lexer and parser support the full Heh grammar.
Golden AST dumps and syntax error tests have been seeded.

# Next Step
Start Phase P3 — Evaluator I.
- Draft `src/val.rs` (runtime values) and `src/eval.rs` (tree-walking evaluator).
- Implement basic types (`int`, `float`, `bool`, `str`).
- Implement basic control flow (`if`/`elif`/`else`, `while`, block scopes).
- Implement variables (`let`/`mut`).
- Stub `sys.print`.
- Introduce `tests/corpus.rs` test harness.
