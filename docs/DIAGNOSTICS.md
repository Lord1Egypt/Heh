# Diagnostic code registry

Codes and meanings are **append-only forever** (SPEC §15). Wording may
improve; a code's meaning never changes and is never reused. Add new codes at
the end of the relevant block.

Every diagnostic renders the same way: the code, the file, `line:column`, the
offending source line with a caret, and a suggestion when one is known.

## Lexing — E0001–E0009

| Code | Meaning |
|---|---|
| E0001 | tab character in indentation (use 4 spaces) |
| E0002 | invalid indentation level (bad indent or dedent) |
| E0003 | unclosed string literal / unclosed interpolation |
| E0004 | invalid character, escape sequence, or interpolation |
| E0005 | malformed number literal |
| E0006 | unclosed, unmatched, or mismatched delimiter |

## Names and bindings — E0010–E0019

| Code | Meaning |
|---|---|
| E0010 | assignment to a `let` binding (SPEC §6.2) |
| E0011 | unknown variable |

## Pattern matching and `try` — E0020–E0029

| Code | Meaning |
|---|---|
| E0020 | non-exhaustive `match` (SPEC §8.2) |
| E0021 | `try` applied to something that is not a result or optional |

## Modules — E0030–E0039

| Code | Meaning |
|---|---|
| E0030 | import cycle (SPEC §9) |
| E0031 | unknown module |
| E0032 | imported file not found |
| E0033 | error inside an imported module |

## Types — E0040–E0059

| Code | Meaning |
|---|---|
| E0040 | type mismatch: implicit coercion, assignment, operator, return, or a mixed-type list/map literal |
| E0041 | `if` / `elif` / `while` condition is not `bool` (there is no truthiness) |
| E0042 | `for` over something that is not a list, map, str, or range |
| E0043 | `return` outside a function |
| E0044 | enum variant applied to the wrong number of arguments |
| E0045 | unknown variant for an enum |
| E0050 | wrong number of type arguments (`list[T]`, `map[K, V]`) |
| E0051 | unknown type |
| E0052 | missing type annotation on a parameter (required at fn boundaries) |
| E0053 | unknown field on a record |
| E0054 | field access on a non-record |
| E0055 | index is not an int (list or str) |
| E0056 | map index does not match the key type |
| E0057 | index on something that is not a collection |
| E0058 | call on a non-function |
| E0059 | a non-unit function or closure may finish without producing its declared return type |

## Parsing — E0100–E0101

| Code | Meaning |
|---|---|
| E0100 | parse error (expected X, found Y) |
| E0101 | malformed declaration |

## Evaluation — E0102–E0114

| Code | Meaning |
|---|---|
| E0102 | invalid assignment target, or a field/index step that does not fit the value |
| E0103 | undefined variable at runtime |
| E0104 | operand type mismatch for an operator, or a non-iterable in `for` |
| E0105 | no such field |
| E0106 | index out of bounds, missing key, or value not indexable |
| E0107 | invalid control flow in `main` |
| E0109 | builtin called with the wrong number of arguments |
| E0110 | `break` / `continue` outside a loop |
| E0111 | value is not callable, or the method does not exist |
| E0112 | `try` on a non-result |
| E0114 | `try` propagated out of a function whose return type is not `_ or error` |

## Faults — E0200+

Faults are for bugs, not expected failures, and cannot be caught (SPEC §7.3).

| Code | Meaning |
|---|---|
| E0200 | division or modulo by zero |
| E0201 | negative exponent in `**` (`int` stays closed under `**`) |
