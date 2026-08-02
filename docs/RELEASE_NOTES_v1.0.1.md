# Heh 𓁨 v1.0.1

A patch release. The frozen v1.0 language surface is unchanged — this is
entirely implementation work, plus the first crates.io publish.

## The bytecode VM is now the default, and covers the whole language

`heh run` executes on the bytecode VM; `--tree-walk` selects the reference
tree-walking evaluator. The VM previously punted three construct families back
to the tree-walker. It now encodes all of them:

- **Closures** become the same function value a named function is, capturing
  the live scope.
- **Optional narrowing** gets real block scopes, including the case where a
  `break` or `continue` jumps out of a narrowed block.
- **Field and index assignment** (`p.x = v`, `l[i] = v`, and nesting). Compound
  forms duplicate the container and index rather than re-evaluating the index,
  so an index expression's side effects cannot run twice.

Both engines share one call path and one set of field/index accessors, so they
cannot drift. Output remains byte-identical across the entire conformance
corpus, enforced by a differential test.

## Runaway recursion is a fault, not a crash

Deep recursion used to abort the process with `fatal runtime error: stack
overflow` and a core dump. SPEC §7.3 says a fault stops the program *with a
diagnostic*. Programs now run on a dedicated 256 MB stack with a call-depth
limit of 10,000, and both engines report **E0202** identically. Legitimate deep
recursion — 9,000 frames — works fine.

## Performance, honestly

`benches/run.sh` runs five benchmarks, each paired with an equivalent CPython
program whose answer must match before a timing is reported.

| benchmark | VM | tree-walk | CPython | VM vs tree-walk | VM vs CPython |
|---|---|---|---|---|---|
| fib | 112ms | 254ms | 37ms | 2.27x | 0.33x |
| loop_sum | 714ms | 967ms | 223ms | 1.35x | 0.31x |
| strings | 52ms | 56ms | 53ms | 1.08x | 1.02x |
| maps | 206ms | 208ms | 58ms | 1.01x | 0.28x |
| bigint | 10ms | 11ms | 31ms | 1.10x | 3.10x |

The VM beats the tree-walker on every benchmark, and beats CPython only on
arbitrary-precision arithmetic. The original design target was ≥5× CPython, and
**this release does not meet it.** The reason is structural rather than a
matter of tuning: every variable access is a string-keyed hash lookup up a
scope chain, and every integer heap-allocates, with no machine-word fast path —
something SPEC §5.1 explicitly invites an implementation to add. That is the
next performance milestone; the measurement harness now exists to prove it.

## Also fixed

- `heh fmt` mangled a closure nested inside a function, emitting a body at the
  wrong indentation that would not re-parse.

## Install

```sh
cargo install heh-lang     # the crate is heh-lang; the command is heh
```

Or download a binary below — linux x86_64, Windows x86_64, macOS arm64 and
x86_64. Checksums in `SHA256SUMS.txt`.

## License

MIT © Mohamed Mounir (Lord1Egypt)
