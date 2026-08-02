# Heh 𓁨 v1.0.2

A performance release. The frozen v1.0 language surface is unchanged — programs
behave exactly as before, just faster.

## Roughly twice as fast

Three costs sat on the interpreter's hot path.

**Integers now use a machine word until they need more.** Every integer used to
allocate a heap vector of limbs, so a loop counter cost an allocation per step.
SPEC §5.1's implementation note asks for "a machine-word fast path with
automatic promotion to a bignum, so ordinary arithmetic runs at native speed" —
that is now what happens. Semantics are unchanged: integers are still
unbounded, and `2 ** 200` is still exact.

**Interpreter-internal maps use a fast hasher.** Variable lookup was running
SipHash — cryptographic strength, on nearly every instruction. `std/hash`
(SHA-256) is untouched and still a real hash.

**Binding a name no longer allocates.** Variable names are refcounted, so
binding a loop variable is a pointer bump instead of a string copy — two
million fewer allocations in a two-million-iteration loop.

| benchmark | v1.0.1 | v1.0.2 | vs CPython |
|---|---|---|---|
| fib | 104ms | 72ms | 0.35x → 0.62x |
| loop_sum | 751ms | 373ms | 0.27x → 0.57x |
| strings | 58ms | 41ms | 0.91x → **1.15x** |
| maps | 205ms | 108ms | 0.38x → 0.51x |
| bigint | 11ms | 10ms | **3.00x** |

Heh is now faster than CPython on string work and on arbitrary-precision
arithmetic, and roughly half its speed on loop-heavy code. The original design
target was ≥5× CPython, and **this release still does not meet it** — see below.

## Integer arithmetic is now verified against CPython

`tests/bignum_vs_python.rs` runs every binary operator over 23 operands chosen
to sit exactly on the machine-word and limb boundaries — about 13,000
comparisons against CPython, whose integers are also unbounded and whose `//`
and `%` sign rules the spec adopts by name.

It caught a real bug in the first draft of the fast path: `1 // -2` returned
`1` instead of `-1`. That test is now part of the suite.

## Also fixed

`sys.rand.bytes` and `sys.rand.int` read only the lowest limb of their
arguments, so large values silently produced wrong bounds. They now use the
whole value and reject out-of-range input.

## What is still slow, and why

Local variables are still looked up **by name** through a chain of scopes on
every access. A mature VM assigns each local a frame slot at compile time and
indexes an array instead. Doing that here needs a resolver pass that handles
shadowing across block scopes, `match` arm bindings, narrowing rebinds, and
closure capture — a subsystem rather than a patch, and exactly the kind of
change that ships subtle scoping bugs when rushed. It is the remaining path
toward the original performance target.

## Install

```sh
cargo install heh-lang     # the crate is heh-lang; the command is heh
```

Or download a binary below — linux x86_64, Windows x86_64, macOS arm64 and
x86_64. Checksums in `SHA256SUMS.txt`.

## License

MIT © Mohamed Mounir (Lord1Egypt)
