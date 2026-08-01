# Heh 𓁨 v1.0.0 — the language is frozen

Heh is a small programming language designed to stop changing. Nineteen
keywords, a spec that fits in eight printed pages against a hundred-page
budget, and a NEVER list fixed on day one. This release freezes the surface:
after v1.0, code that runs today runs unchanged forever.

## What Heh is

- **Easier than Python.** Indentation blocks, inference everywhere except
  function boundaries, one obvious way to do things.
- **Infinite by nature.** `int` is arbitrary-precision — overflow does not
  exist. `2 ** 200` and `factorial(1000)` are exact, with nothing to import.
  Ranges may be unbounded: `for i in 0..` runs until you `break`.
- **Secure by default.** All I/O flows from a single `Sys` value handed to
  `main`. A function that never receives it cannot touch the filesystem,
  network, clock, or environment. Any capability can be revoked from the
  command line (`--deny-net`), and revocation fails closed.
- **Reliable.** Static types, errors as values (`ok` / `err` / `try`), no
  null, no exceptions, exhaustive `match`.
- **Immortal.** One binary, **zero crates** — Rust standard library only.
  No package registry: dependencies are vendored into your repo and pinned by
  SHA-256 in `heh.lock`, verified on every run.

## Getting it

```sh
git clone https://github.com/Lord1Egypt/Heh && cd Heh
cargo build --release          # target/release/heh — the whole toolchain
echo 'sys.print("Heh lives forever 𓁨")' > hello.heh
./target/release/heh run hello.heh
```

## The toolchain

`heh run` · `heh check` · `heh test` · `heh fmt` · `heh get` · `heh ast` ·
`heh tokens` — one binary, no configuration files, no options to argue about.
The formatter is canonical and comment-preserving. `heh run --vm` executes on
the bytecode VM, which is byte-identical to the tree-walking evaluator across
the entire conformance corpus.

## Standard library

Eight pure modules — `math`, `json`, `fmt`, `time`, `regex` (RE2-style, no
backtracking), `csv`, `hash` (SHA-256, CRC32), `debug` — plus builtin methods
on `str`, `list`, and `map`. Anything effectful lives on `Sys` instead. The
complete frozen surface is in [docs/STDLIB.md](STDLIB.md).

## What v1.0 does not have

- **No raw sockets.** A socket is a handle with a lifetime, and Heh has no
  resource-lifecycle construct; committing to one at the freeze would be a
  permanent decision made in a hurry. `sys.net.get` covers HTTP request and
  response work. Sockets remain a candidate addition against the page budget.
- **No concurrency.** When it arrives it will be structured and colorless —
  `async`/`await` function coloring is on the NEVER list.
- Everything else on the NEVER list (SPEC §1.2), permanently: null, exceptions,
  integer overflow, `eval`, classes, macros, operator overloading, global
  mutable state, a package server, implicit coercion.

## Conformance

`tests/corpus/` is the definition: an implementation that passes it is Heh.
The corpus grows and never shrinks. The spec, not the implementation, is
authoritative — where they disagreed during the v1.0 audit, the implementation
was fixed.

## License

MIT © Mohamed Mounir (Lord1Egypt)
