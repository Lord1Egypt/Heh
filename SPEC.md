# Heh 𓁨 — Language Specification v1.0

> **Heh** (ḥḥ) — the Egyptian god of infinity and eternity, holding a palm rib
> notched with millions of years. This language is named for him and designed
> like him: **small, frozen, and endless**.
>
> This spec is the single source of truth. The implementation follows the spec,
> never the other way around. **This is v1.0: the surface below is frozen.**
> Additions may still be proposed against the ≤100-page budget (§1.3); nothing
> here may change meaning or be removed (§1.2 item 12).

---

## 1. Design charter

### 1.1 The five pillars

1. **Easier than Python.** 19 keywords, one obvious way to do things,
   indentation blocks, no ceremony. A beginner reads real Heh code on day one.
2. **Infinite by nature.** Integers are arbitrary-precision — **overflow does
   not exist**. Ranges may be unbounded. Nothing in the language has an
   artificial numeric cliff.
3. **Secure by default.** Capability-based I/O: pure code physically cannot
   touch files, network, clock, or environment. No `eval`, no null, no
   exceptions flying across the program.
4. **Reliable.** Static types with inference (annotations only at function
   boundaries), errors as values, exhaustive `match`, no undefined behavior.
5. **Immortal.** Spec ≤ 100 pages forever. Zero-dependency single-binary
   toolchain. No package registry — imports are vendored and content-addressed.
   After v1.0, **backward compatibility is religion**: code from year 1 runs
   in year 30.

### 1.2 The NEVER list (frozen now, forever)

Heh will **never** have:

1. `null` / nil pointer values (absence is `T?` — see §6.4)
2. Exceptions / try-catch control flow (errors are values — §7)
3. Integer overflow or wrapping (integers are unbounded — §5.1)
4. `eval` or runtime code loading
5. Classes, inheritance, or metaclasses (records + enums + functions — §8)
6. Macros or compile-time metaprogramming
7. Operator overloading
8. Global mutable state (all sharing is explicit via parameters)
9. A central package registry or package server
10. `async`/`await` function coloring (concurrency, when it comes post-1.0,
    will be structured and colorless)
11. Implicit type coercion (`1 + "2"` is a compile error)
12. Breaking changes after v1.0

### 1.3 Immortality mechanics

- The spec must stay printable at ≤ 100 pages. Every feature proposal competes
  for that budget; when the budget is spent, the language is finished.
- The reference implementation is **Rust, standard library only, zero crates**,
  a single static binary `heh`. Anyone with a Rust compiler can rebuild the
  toolchain from source forever; anyone with this document can reimplement the
  language from scratch.
- The conformance corpus (`tests/corpus/`) is part of the spec: an
  implementation that passes the corpus **is** Heh.

---

## 2. Source text

- Source files are **UTF-8**, extension `.heh`.
- Line endings: `\n`; `\r\n` is accepted and normalized to `\n` by the lexer.
- Comments: `#` to end of line. No block comments.
- Identifiers: `[a-zA-Z_][a-zA-Z0-9_]*`. Convention: `snake_case` values and
  functions, `CapWords` types. A leading `_` marks "internal by convention"
  (the language does not enforce privacy).

## 3. Blocks and layout

Blocks are expressed by **indentation**, exactly **4 spaces** per level.
Tab characters in indentation are a **compile error** (diagnostic `E0001`).

The lexer converts layout to synthetic `INDENT` / `DEDENT` / `NEWLINE` tokens:

1. Track a stack of indentation widths, starting `[0]`.
2. At each physical line that is not blank and not only a comment, measure the
   leading spaces `w`.
   - `w >` top of stack: push `w`, emit `INDENT`. `w` must equal top + 4.
   - `w <` top of stack: pop and emit `DEDENT` until top == `w`
     (if no exact match: error `E0002`).
   - `w ==` top: no token.
3. Emit `NEWLINE` at each logical line end. A line ending inside an unclosed
   `(`, `[`, or `{` continues onto the next physical line (implicit joining;
   the continuation's indentation is not measured).
4. At end of file, emit `DEDENT` for every stacked level above 0.

Blank lines and comment-only lines never produce layout tokens.

## 4. Keywords

Exactly **19 keywords**, frozen:

```
and  break  continue  elif  else  fn  for  if  in  let
match  mut  not  or  return  try  type  use  while
```

`true`, `false`, `none` are literals. `ok`, `err`, `some` are builtin
constructors. None of these six are keywords, but all are reserved names
(cannot be shadowed).

## 5. Values and types

Static types, checked before execution. Annotations are **required on function
parameters and return types**, **inferred everywhere else** (locals never need
annotations).

### 5.1 `int` — the infinite integer

`int` is a **signed arbitrary-precision integer**. There is no overflow, no
wrap-around, no `i32/i64` zoo. `2 ** 200` just works.

> Implementation note (non-normative): implementations should use a machine-word
> fast path with automatic promotion to a bignum, so ordinary arithmetic runs at
> native speed. Semantics are always unbounded.

Literals: `0`, `42`, `1_000_000`, `0xFF`, `0b1010`, `0o755`. Base prefixes are
lowercase; `_` separators must sit between two digits.

### 5.2 `float`

IEEE-754 binary64. A float literal has a decimal point (with digits on both
sides), a lowercase-`e` exponent, or both: `1.5`, `2.0`, `1e5`, `6.02e23`,
`1.5e-3`. `float` has `inf`,
`-inf`, and `nan` values (produced by arithmetic; there are no literals for
them — use `std/math`). `int` and `float` never mix implicitly: converting is
explicit via the builtins `int(x)` (truncates towards zero; a fault on nan/inf)
and `float(x)` (exact for small ints; rounds for huge ones).

**Display.** A float always prints with a decimal point, so `3.0` is never
mistaken for the int `3`. Plain decimal notation is always used — never an
exponent — because it is exact and needs no threshold rule. `nan`, `inf`, and
`-inf` print as those names.

### 5.3 `bool`, `str`

- `bool`: `true` / `false`. Conditions must be `bool` — no truthiness
  (`if list` is a compile error; write `if list.len() > 0`).
- `str`: immutable UTF-8 string. Literals use double quotes with escapes
  `\n \t \\ \" \{ \u{1F40D}`. **Interpolation** is built in: `"sum is {a + b}"`
  evaluates the expression and formats it. `{` in a literal is escaped `\{`.

### 5.4 Collections

- `list[T]` — dynamic array literal `[1, 2, 3]`.
- `map[K, V]` — ordered hash map, literal `{"a": 1, "b": 2}`. Key types: `int`,
  `str`, `bool`. **Insertion order is preserved** (like Python dicts) and is the
  order used by iteration, `keys()`, `values()`, printing, and `json.write` —
  so a program's output does not depend on hash seeding. Re-assigning an
  existing key keeps its original position. Map equality ignores order.
- Collections are reference values (aliasing is visible, like Python). `let`
  vs `mut` controls **rebinding** of the name, not deep mutation; `str` is
  always immutable.

### 5.5 Ranges — including infinite ones

`a..b` is a lazy range value (`a` inclusive, `b` exclusive). `a..=b` includes
`b`. **`a..` is an unbounded range** — it yields values forever:

```heh
for i in 0..                # the god's own loop
    if i * i > 10_000
        break
```

Ranges are lazy: an infinite range costs nothing until iterated. Materialize a
finite range with `list(0..10)`.

### 5.6 `option` and `result`

- `T?` is an optional: either a value of `T` or `none`. Constructed with
  `some(v)` or `none`. See §6.4 for narrowing.
- `T or error` is a result type: `ok(v)` or `err("message")`. See §7.

### 5.7 Functions are values

`fn(int) -> int` is a type. Functions close over their environment (closures).

## 6. Expressions and statements

### 6.1 Operators (highest to lowest precedence)

| Level | Operators |
|---|---|
| 1 | `x.field` `x.method(...)` `f(...)` `x[i]` |
| 2 | unary `-` `not` |
| 3 | `**` (power, right-assoc) |
| 4 | `*` `/` `%` `//` (floor div) |
| 5 | `+` `-` |
| 6 | `..` `..=` |
| 7 | `==` `!=` `<` `<=` `>` `>=` |
| 8 | `and` (short-circuit) |
| 9 | `or` (short-circuit) |

`/` on two ints yields `float` (like Python 3); `//` is integer floor division
(the quotient rounds towards negative infinity, so `-7 // 2 == -4`); `%` takes
the sign of the divisor, so `-7 % 3 == 2` and `7 % -3 == -2`. Division and
modulo by zero are a runtime **fault** (§7.3). `**` on two ints is exact and
unbounded; a negative exponent is a fault, since `int` stays closed under `**`
(use floats for fractional powers).

> **Unary `-` binds tighter than `**`** (levels 2 and 3 above), so `-2 ** 4` is
> `(-2) ** 4` = `16`. This is deliberate — one rule, no exception carved out for
> one operator — but it is the opposite of Python and of ordinary mathematical
> notation, where `-2**4` is `-16`. `heh fmt` keeps the parentheses in
> `(-2) ** 4` for exactly this reason.

### 6.2 Bindings

```heh
let name = "Heh"        # immutable binding: cannot be reassigned
mut count = 0           # mutable binding
count = count + 1
count += 1              # also -= *= /=
```

Assignment to a `let` name is a compile error `E0010`.

### 6.3 Control flow

```heh
if x > 0
    sys.print("positive")
elif x == 0
    sys.print("zero")
else
    sys.print("negative")

while queue.len() > 0
    process(queue.pop())

for item in items          # iterates lists, maps (keys), ranges, strs (chars)
    sys.print(item)
```

`break` / `continue` as usual. `if/elif/else`, `match`, and blocks are
statements; the **last expression of a function body is its return value**
(§6.5).

### 6.4 Optionals and narrowing

No null. `T?` must be narrowed before use:

```heh
fn find(users: list[str], who: str) -> int?
    for i in 0..users.len()
        if users[i] == who
            return some(i)
    none

let idx = find(names, "ra")
if idx != none
    sys.print("found at {idx}")    # idx is int inside this block
else
    sys.print("not found")
```

Narrowing rules (complete list): inside the true-branch of `if x != none`,
and after `if x == none` + `return`/`break`/`continue`, the binding `x` has
type `T`. `match` also narrows (§8.2).

### 6.5 Functions

```heh
fn area(w: float, h: float) -> float
    w * h                          # last expression = return value

fn greet(name: str) -> str
    "hello, {name} 𓁨"

let double = fn(x: int) -> int     # anonymous fn (closure)
    x * 2
```

`return expr` exits early; `return` alone returns the unit value (a function
with no `->` clause returns unit). Unit is not a value you can bind usefully;
it prints as `none`.

A closure's body is an indented block, so a closure is written as a statement's
own value — `let double = fn(x: int) -> int` and then its body — and passed on
by name: `[1, 2, 3].map(double)`. Layout is suppressed inside `(` `[` `{`
(§3), so a block-bodied closure cannot be written inline inside a call's
parentheses; bind it first.

**UFCS (uniform function call syntax):** `x.f(y)` is exactly `f(x, y)` when
`x`'s type has no builtin method `f`. This gives method-style chaining without
classes:

```heh
fn shout(s: str) -> str
    s.upper() + "!"

sys.print("heh".shout().shout())   # HEH!!
```

## 7. Errors — values, not explosions

### 7.1 The `error` type

`error` is a builtin record: `error(msg: str)`. A function that can fail
returns `T or error`:

```heh
fn parse_age(s: str) -> int or error
    let n = try int_of(s)               # propagate if int_of failed
    if n < 0
        return err("age cannot be negative: {n}")
    ok(n)
```

### 7.2 `try`

`try expr` where `expr: T or error`: if `ok(v)`, the value is `v`; if
`err(e)`, the enclosing function returns `err(e)` immediately. `try` is only
legal inside a function whose return type is `_ or error`. Handling instead of
propagating uses `match`:

```heh
match parse_age(input)
    ok(age)
        sys.print("age {age}")
    err(e)
        sys.print("bad input: {e.msg}")
```

### 7.3 Faults

A **fault** stops the program with a diagnostic (file, line, message): index
out of bounds, division by zero, explicit `std/debug.fault(msg)`, or a bug in
the interpreter. Faults are for **bugs**, not for expected failures — there is
deliberately no way to catch one. APIs that can fail in normal operation
return `T or error` or `T?`. The non-faulting lookups are `list.get(i) -> T?`
and `map.get(k) -> V?`: a missing index or key is `none`, never a fault.

## 8. Records, enums, `match`

### 8.1 Records

```heh
type Point
    x: float
    y: float

let p = Point(x: 1.0, y: 2.0)      # construction uses field names
let q = Point(x: 0.0, y: p.y)
```

Records are reference values with public fields. No methods — use UFCS:

```heh
fn dist(a: Point, b: Point) -> float
    ((a.x - b.x) ** 2.0 + (a.y - b.y) ** 2.0).sqrt()

sys.print(p.dist(q))
```

### 8.2 Enums (sum types)

```heh
type Shape = circle(r: float) or square(side: float) or dot

fn area(s: Shape) -> float
    match s
        circle(r)
            3.141592653589793 * r * r
        square(side)
            side * side
        dot
            0.0
```

`match` must be **exhaustive** (compile error `E0020` if a variant is
unhandled); `_` is the wildcard arm. `match` also works on `int`, `str`,
`bool`, `T?` (`some(x)` / `none`), and `T or error` (`ok(x)` / `err(e)`)
values with literal and binding patterns.

## 9. Modules and imports — no package servers, ever

```heh
use std/json                       # stdlib (ships inside the heh binary)
use "./geometry.heh"               # relative file, its top-level fns/types
use vendor/inkfish                 # vendored third-party module
```

- `use std/x` loads a stdlib module; `use "./f.heh"` loads a sibling file.
  Both bind a namespace: `json.parse(...)`, `geometry.dist(...)`.
- Third-party code lives **inside your repo** under `vendor/`, fetched once by
  `heh get <git-url-or-https-url>`, which records the source URL and the
  **SHA-256 of every file** in `heh.lock`. On every run, vendored files are
  verified against the lock — a mismatch is a fault. No registry, no resolver,
  no dependency graph: vendor what you use, commit it, own it forever.
- Everything a module defines is importable; `_underscore` names are
  internal by convention only.
- Import cycles are a compile error `E0030`.

## 10. Capabilities — pure by default

Heh has **no ambient authority**. All effects flow from a single `Sys` value
handed to the entry point; a function that never receives a capability cannot
perform I/O, read the clock, or observe randomness. Security review = grep for
who receives `sys`.

```heh
fn main(sys: Sys)
    let text = try sys.fs.read("notes.txt") else exit
    sys.print(text)
```

`Sys` and its sub-capabilities (each independently passable):

| Capability | Powers |
|---|---|
| `sys.print(x)` / `sys.input() -> str or error` | stdio |
| `sys.args -> list[str]` | CLI arguments |
| `sys.fs` | `read(path) -> str or error`, `read_bytes`, `write`, `append`, `exists`, `list_dir`, `remove` |
| `sys.net` | `get(url) -> str or error` (HTTP/HTTPS) |
| `sys.env` | `get(name) -> str?`, `set(name, value)` |
| `sys.clock` | `now() -> int` (unix millis), `sleep(ms)` |
| `sys.rand` | `int(a, b) -> int or error`, `float() -> float`, `bytes(n)` — OS entropy |

*(v1.0 has no raw-socket capability. A socket is a handle with a lifetime —
open, read, write, close — and Heh has no resource-lifecycle construct to hang
that on; inventing one at the freeze would be a permanent commitment made in a
hurry. `sys.net.get` covers request/response work. Sockets remain a candidate
addition against the §1.3 page budget.)*

The CLI can shrink the root capability: `heh run app.heh --deny-net --deny-fs`
makes the corresponding sub-capability's operations return
`err("capability denied: net")`. Deny-flags fail closed.

*(`try expr else exit` is sugar: on `err(e)`, print the error diagnostic to
stderr and exit with code 1 — for scripts and `main`.)*

## 11. Script mode

If a file contains no `fn main`, its top-level statements execute in order
with `sys` bound implicitly — so hello world is one line:

```heh
sys.print("Heh lives forever 𓁨")
```

Script mode is sugar for wrapping the file in `fn main(sys: Sys)`. Files with
`fn main` must not have other top-level statements (only `use`, `type`, `fn`,
and top-level `let` of constants).

## 12. Standard library v1 (frozen surface)

Batteries included, small forever. Builtin methods on core types
(`str.upper/lower/split/trim/replace/contains/starts_with/len/chars/...`,
`list.push/pop/get/len/sort/map/filter/join/...`,
`map.get/set/remove/keys/values/len/...`) plus exactly eight modules:

`std/math` · `std/json` · `std/fmt` · `std/time` · `std/regex` (RE2-style, no
backtracking) · `std/csv` · `std/hash` (SHA-256, CRC32) · `std/debug`
(`fault`, `assert`)

Every one of these is **pure** — anything effectful lives on `Sys`. `std/time`
is calendar arithmetic over the same unix-millisecond int `sys.clock.now()`
returns; it never reads the clock itself, so the instant is always an argument.
Builtin methods are called with parentheses, including `len`: `x.len()`.
The full frozen signatures are in `docs/STDLIB.md`.

## 13. Tooling (one binary)

| Command | Does |
|---|---|
| `heh run file.heh [args] [--deny-*]` | run a program (`--vm` selects the bytecode VM) |
| `heh check file.heh` | parse + type-check only |
| `heh fmt [--check] <path>` | canonical formatter, no options, idempotent; a directory formats the whole tree |
| `heh test [path]` | runs every `fn test_*()` in `*_test.heh` files |
| `heh tokens/ast file.heh` | dump lexer/parser output (dev + conformance) |
| `heh get <url>` | vendor a module + record hashes in `heh.lock` |

`heh test`: test functions take no params and no `Sys` (tests are pure);
`std/debug.assert(cond, msg)` failures mark the test failed. Deterministic
order, summary line, exit 1 on any failure.

`heh fmt` is comment-preserving: a comment is never deleted. One sharing a line
with code stays on that line; otherwise it keeps its own line.

## 14. Grammar (EBNF, v0.1)

Layout tokens INDENT/DEDENT/NEWLINE per §3. Lowercase = lexer tokens.

```ebnf
file        = { use_decl } , { top_item } ;
use_decl    = "use" , ( path_ident | string ) , NEWLINE ;
top_item    = fn_decl | type_decl | let_stmt | statement ;   (* §11 rules *)

fn_decl     = "fn" , [ ident , "." ] , ident , "(" , [ params ] , ")" ,
              [ "->" , type_expr ] , block ;
params      = param , { "," , param } ;
param       = ident , [ ":" , type_expr ] ;                  (* main(sys) may omit *)

type_decl   = "type" , IDENT ,
              ( "=" , variant , { "or" , variant } , NEWLINE
              | block_fields ) ;
variant     = ident , [ "(" , fields , ")" ] ;
block_fields= NEWLINE , INDENT , { ident , ":" , type_expr , NEWLINE } , DEDENT ;
fields      = ident , ":" , type_expr , { "," , ident , ":" , type_expr } ;

type_expr   = base_type , [ "?" ] , [ "or" , "error" ] ;
base_type   = ident , [ "[" , type_expr , { "," , type_expr } , "]" ]
            | "fn" , "(" , [ type_expr , { "," , type_expr } ] , ")" ,
              [ "->" , type_expr ] ;

block       = NEWLINE , INDENT , statement , { statement } , DEDENT ;
statement   = let_stmt | assign_stmt | if_stmt | while_stmt | for_stmt
            | match_stmt | return_stmt | "break" NEWLINE | "continue" NEWLINE
            | expr , NEWLINE ;
let_stmt    = ( "let" | "mut" ) , ident , "=" , expr , NEWLINE ;
assign_stmt = lvalue , ( "=" | "+=" | "-=" | "*=" | "/=" ) , expr , NEWLINE ;
lvalue      = ident , { "." , ident | "[" , expr , "]" } ;
if_stmt     = "if" , expr , block , { "elif" , expr , block } ,
              [ "else" , block ] ;
while_stmt  = "while" , expr , block ;
for_stmt    = "for" , ident , "in" , expr , block ;
match_stmt  = "match" , expr , NEWLINE , INDENT ,
              pattern_arm , { pattern_arm } , DEDENT ;
pattern_arm = pattern , block ;
pattern     = "_" | literal | ident , [ "(" , ident , { "," , ident } , ")" ] ;
return_stmt = "return" , [ expr ] , NEWLINE ;

expr        = or_expr ;
or_expr     = and_expr , { "or" , and_expr } ;
and_expr    = cmp_expr , { "and" , cmp_expr } ;
cmp_expr    = range_expr , [ ( "=="|"!="|"<"|"<="|">"|">=" ) , range_expr ] ;
range_expr  = add_expr , [ ( ".." | "..=" ) , [ add_expr ] ] ;
add_expr    = mul_expr , { ( "+" | "-" ) , mul_expr } ;
mul_expr    = pow_expr , { ( "*" | "/" | "//" | "%" ) , pow_expr } ;
pow_expr    = unary_expr , [ "**" , pow_expr ] ;
unary_expr  = [ "-" | "not" ] , postfix ;
postfix     = primary , { "." , ident , [ call_args ] | call_args
                        | "[" , expr , "]" } ;
call_args   = "(" , [ arg , { "," , arg } ] , ")" ;
arg         = [ ident , ":" ] , expr ;                        (* named for records *)
primary     = literal | ident | "(" , expr , ")"
            | "[" , [ expr , { "," , expr } ] , "]"
            | "{" , [ expr , ":" , expr , { "," , expr , ":" , expr } ] , "}"
            | "fn" , "(" , [ params ] , ")" , [ "->" , type_expr ] , block
            | "try" , expr , [ "else" , "exit" ] ;
literal     = int_lit | float_lit | string | "true" | "false" | "none" ;
```

## 15. Diagnostics

Every compile/runtime diagnostic shows: stable code (`E0001`…), file,
line:column, the offending source line with a caret, and a one-line
suggestion when known. Diagnostic **codes and meanings are append-only**
(immortality applies to error codes too); exact wording may improve.

## 16. Conformance

`tests/corpus/` holds `programs/*.heh` with expected stdout in `*.out`, and
`errors/*.heh` with expected diagnostic codes in `*.err`. **The corpus as of
this document is the v1.0 conformance corpus.** It grows and never shrinks; an
implementation that passes it is Heh. **Passing the corpus is the definition of
being a Heh implementation.** `examples/` mirrors the flagship programs.
