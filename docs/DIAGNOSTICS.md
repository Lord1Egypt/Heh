# Diagnostic code registry

Codes and meanings are **append-only forever** (SPEC §15). Wording may
improve; a code's meaning never changes and is never reused. Add new codes at
the end of the phase's block.

| Code | Meaning | Since |
|---|---|---|
| E0001 | tab character in indentation | P1 |
| E0002 | invalid indentation level (bad indent or dedent) | P1 |
| E0003 | unclosed string literal / unclosed interpolation | P1 |
| E0004 | invalid character, escape sequence, or interpolation | P1 |
| E0005 | malformed number literal | P1 |
| E0006 | unclosed, unmatched, or mismatched delimiter | P1 |
| E0010 | assignment to a `let` binding | reserved (SPEC §6.2, lands P6) |
| E0020 | non-exhaustive `match` | reserved (SPEC §8.2, lands P6) |
| E0030 | import cycle | reserved (SPEC §9, lands P9) |
| E0040 | implicit type coercion / type mismatch | reserved (SPEC §1.2, lands P6) |
