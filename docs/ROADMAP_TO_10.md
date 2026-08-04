# Roadmap to a 10/10 Heh Toolchain

Heh v1.0.4 is a strong, released implementation of a frozen language. This
roadmap defines the work required before calling the toolchain 10/10. It is not
a language-feature roadmap: `SPEC.md` remains authoritative and the v1.0
surface remains frozen.

The score is earned by evidence, not by completing a list. Every phase has an
exit gate, and no release may claim a gate until it has run successfully on the
supported platforms.

## Definition of 10/10

Heh reaches 10/10 only when all of these are true:

| Area | Required evidence |
|---|---|
| Specification | Every normative executable claim in `SPEC.md` has a conformance test; both engines agree byte-for-byte. |
| Static correctness | All locally knowable type errors are rejected by `heh check`; `Any` exists only at explicitly documented dynamic boundaries. |
| Safety | No panic, abort, traversal escape, or uncontrolled resource use is reachable through user source, paths, lockfiles, network responses, or CLI arguments. |
| Performance | VM benchmark geometric mean is at least 5x CPython, no ordinary benchmark is below 2x, and correctness remains identical to the tree walker. |
| Maintainability | Front end, runtime, VM, modules, HTTP, vendoring, and CLI have explicit internal boundaries and focused tests; no cyclic internal architecture. |
| Portability | Linux, Windows, Intel macOS, ARM macOS, and Rust 1.70 pass CI and release verification. |
| Supply chain | Reproducible tag-driven releases include checksums, provenance/signatures, verified crate publication, and rollback documentation. |
| Tooling | Editor syntax support and a minimal LSP cover diagnostics, formatting, symbols, hover types, and go-to-definition. |
| Validation | At least three non-trivial external programs and one independent implementation/security review validate real use beyond the repository corpus. |

## Non-negotiable constraints

- Keep `[dependencies]` empty, including convenience dependencies introduced
  only to accelerate development.
- Do not change frozen syntax, semantics, standard-library signatures,
  iteration order, formatting, diagnostics, or capability behavior.
- Keep the tree walker as the semantic reference.
- Do not optimize before a differential test covers the behavior being changed.
- Do not split files and change behavior in the same PR.
- Every phase uses branch → PR → protected CI → merge.

## Phase 0 — Establish the scorecard and baselines

Purpose: make regressions and completion measurable before architectural work.

Work:

1. Add a checked-in benchmark baseline containing hardware/OS metadata,
   workload answers, median timings, variance, and VM/tree-walker/CPython ratios.
2. Run each benchmark enough times to report median and dispersion instead of a
   single timing.
3. Inventory every normative `SPEC.md` statement and map it to a corpus test.
4. Inventory all production `Any`, `unwrap`, `expect`, `panic!`, raw indexing,
   recursive traversal, process launch, network, and filesystem boundary sites.
5. Add CI summaries for test count, corpus count, binary sizes, and benchmark
   smoke limits.

Exit gate:

- A machine-readable scorecard identifies every uncovered spec claim and every
  unresolved safety/type boundary.
- Benchmark variance is understood and the baseline is reproducible.
- No user-visible behavior changes in this phase.

Target release: documentation/CI only; no crate release required.

## Phase 1 — Complete static type precision

Purpose: make “static types” fully true for everything knowable at compile time.

### 1.1 Typed builtins and capabilities

- Replace the checker’s `Sys` placeholder with internal structural types for
  `sys`, `sys.fs`, `sys.net`, `sys.env`, `sys.clock`, and `sys.rand`.
- Register every frozen builtin and method with its exact signature from
  `docs/STDLIB.md`.
- Validate builtin arity, positional/named argument rules, receiver types,
  return types, and capability method chains statically.

### 1.2 Typed modules

- Parse/check imported files into module interfaces before checking consumers.
- Export typed function, constructor, constant, and type signatures.
- Cache interfaces by canonical path and content hash.
- Preserve import-cycle diagnostics and never execute a module to discover its
  type surface.

### 1.3 Typed closures and calls

- Resolve closure parameter and return types to `Ty::Fn`.
- Check closure bodies at declaration, not at runtime call sites.
- Validate named arguments by parameter identity, including duplicates,
  unknown names, positional-after-named cases, and missing parameters.
- Type-check record/enum constructors and all pattern-arm bindings exactly.

### 1.4 Complete control-flow typing

- Verify exhaustive `match` for enums, booleans, optionals, results, and
  wildcard-required infinite domains.
- Check all arm result types where a match contributes a value.
- Model narrowing and divergence across `if`, `match`, `try`, `return`, loops,
  `break`, and `continue`.
- Diagnose missing returns on every reachable function path.

Exit gate:

- No unexplained `Ty::Any` remains. Each necessary dynamic boundary has an
  adjacent invariant comment and a focused test.
- A negative type corpus covers every checker diagnostic with exact stderr,
  location, exit code, and compile-vs-runtime phase.
- `heh check` rejects all locally knowable errors without executing code.
- The original corpus and VM/tree-walker differential remain unchanged.

Target release: v1.0.5.

## Phase 2 — Eliminate user-reachable crashes and resource hazards

Purpose: prove the promise that hostile input produces diagnostics, never host
process failures or boundary escapes.

Work:

1. Classify every `unwrap`, `expect`, `panic!`, raw index, integer conversion,
   recursion point, and allocation driven by input.
2. Replace user-reachable assumptions with checked operations and stable
   diagnostics; keep only documented, mechanically proven internal invariants.
3. Add deterministic, dependency-free generative tests for lexer, parser,
   checker, formatter, compiler, VM, bignum, regex, JSON, HTTP framing, import
   graphs, and lockfile parsing.
4. Persist every discovered failure as a minimal regression fixture.
5. Add limits and tests for source size, nesting, recursion, response headers,
   chunk sizes, declared body lengths, redirects, subprocess output, and
   vendored tree size where unbounded work could exhaust the host.
6. Verify formatter idempotence and parse-equivalence over generated programs.
7. Test symlink races and canonical-path containment at each filesystem
   boundary, not only recursive discovery.

Exit gate:

- At least one million generated/adversarial cases complete without panic,
  abort, hang, or unbounded allocation.
- Platform CI runs a bounded adversarial smoke suite.
- Every retained panic site has a written invariant and a test that establishes
  it before the site can execute.
- A focused independent security review has no unresolved high/critical issue.

Target release: v1.0.6 if fixes are user-relevant; otherwise combine with the
next implementation release.

## Phase 3 — Frame-slot VM and closure capture

Purpose: remove the dominant runtime cost while preserving reference semantics.

### 3.1 Resolver intermediate representation

- Add a resolver pass between checking and bytecode compilation.
- Assign stable local slots per lexical scope.
- Represent accesses explicitly as local slot, captured upvalue, module/global,
  builtin, or unresolved-error states.
- Build scope tables for blocks, loops, match arms, narrowing rebindings,
  functions, closures, and recursion.

### 3.2 Frame and upvalue runtime

- Replace VM local name-map lookup with indexed frame storage.
- Implement closure capture deliberately: immutable values may copy/share;
  mutable captured bindings require cells with defined lifetime.
- Keep global/module lookup separate from locals so dynamic global behavior does
  not leak into the fast path.
- Preserve the tree walker unchanged as the oracle.

### 3.3 Optimization ladder

After slots are correct, profile before each additional optimization:

1. compact bytecode operands and constant/name pools;
2. specialized integer, float, boolean, and comparison opcodes;
3. eliminate avoidable stack/value clones;
4. cache stable field and method resolution;
5. reduce call-frame and closure allocation;
6. optimize loops only when profiles show the remaining cost.

Do not add speculative optimization or unsafe code.

Exit gate:

- Resolver unit tests cover shadowing, mutation, recursion, nested closures,
  deep capture, loop variables, match bindings, and optional narrowing.
- VM and tree walker agree across the full corpus plus generated programs.
- Benchmark answers match CPython before timings are accepted.
- Geometric-mean VM speed is at least 5x CPython; no ordinary benchmark is
  below 2x; bigint remains at least 3x; binary size growth stays below 20%
  unless explicitly justified.

Target release: v1.0.7.

## Phase 4 — Decompose internals without semantic change

Purpose: reduce review surface and ownership ambiguity after behavior is locked
by the expanded tests.

Proposed boundaries:

- `runtime/value`, `runtime/scope`, `runtime/call`, `runtime/ops`;
- `net/url`, `net/http1`, `net/curl_tls`;
- `vendor/fetch`, `vendor/lock`, `vendor/walk`;
- `checker/types`, `checker/flow`, `checker/patterns`, `checker/modules`;
- `vm/bytecode`, `vm/frame`, `vm/execute`, `vm/verify`;
- `cli/run`, `cli/check`, `cli/test`, `cli/fmt`, `cli/get`.

Rules:

- One mechanical move per PR, followed by a separate improvement PR if needed.
- Shared semantic operations stay single-sourced between both engines.
- Module boundaries follow invariants and ownership, not arbitrary line limits.
- Add architecture tests preventing forbidden dependency directions.

Exit gate:

- Each subsystem exposes a small documented internal API.
- No cyclic module dependency or duplicated semantic implementation exists.
- Core files are reviewable in isolation and tests are colocated with their
  owning subsystem.
- Full behavior and benchmark baselines remain within tolerance.

Target release: normally no crate release unless behavior fixes are included.

## Phase 5 — Supply-chain and release assurance

Purpose: make “immortal” releases independently verifiable.

Work:

1. Generate source archives and binaries entirely from the protected tag.
2. Add GitHub artifact attestations/provenance for every binary.
3. Sign the checksum manifest and document verification commands.
4. Verify packaged crate contents, version/tag agreement, MSRV, binary version,
   and clean-tree rebuilds before publication.
5. Add a post-release job that installs from crates.io and downloads each
   platform artifact for checksum/version smoke verification.
6. Document failed-release cleanup, rollback, yanking, and security-advisory
   procedures.
7. Pin third-party GitHub Actions to reviewed commit SHAs with an explicit
   update policy.

Exit gate:

- A third party can trace every artifact to the protected source tag and verify
  integrity without trusting a maintainer workstation.
- A release cannot become public if any asset, checksum, provenance statement,
  crate verification, or supported-platform test is missing.

Target release: v1.0.8.

## Phase 6 — First-class tooling without language expansion

Purpose: make the frozen language practical for sustained real-world use.

Work:

- Publish syntax definitions for VS Code/TextMate and common terminal editors.
- Implement an std-only `heh lsp` mode reusing lexer/parser/checker spans.
- Support diagnostics, document formatting, symbols, hover types, definition,
  and references before considering completion or rename.
- Add protocol transcript tests and editor fixtures; malformed client messages
  must never panic the server.
- Write a guided tutorial that builds one real capability-limited application,
  vendors a module, tests it, formats it, and packages it.

Exit gate:

- LSP behavior is tested without an editor process and works in at least VS
  Code plus one independent editor client.
- Diagnostics and formatting match the CLI byte-for-byte.
- The tutorial is executed in CI from a clean directory.

Target release: v1.0.9.

## Phase 7 — External validation and the 10/10 audit

Purpose: ensure the score reflects use outside the implementation’s own tests.

Required validation projects:

1. a filesystem-heavy CLI operating under explicit capabilities;
2. a network/JSON client with deny-net and malformed-response tests;
3. a vendored multi-module program using enums, errors, closures, formatting,
   and tests;
4. an independent small interpreter or parser that runs a meaningful subset of
   the conformance corpus.

Final audit:

- Re-run every executable claim in `SPEC.md`, `README.md`, `docs/STDLIB.md`,
  `docs/DIAGNOSTICS.md`, and release documentation.
- Re-run the complete corpus on both engines and every supported platform.
- Run the adversarial suite, MSRV build, clean crate install, artifact
  verification, LSP transcripts, and benchmark protocol.
- Commission independent correctness/security review and resolve every
  high/critical finding.
- Publish the scorecard with raw evidence and explicitly list any remaining
  limitation.

Exit gate:

- Every row in the 10/10 definition table has a reproducible evidence link.
- No known spec contradiction, high/critical security issue, unexplained
  dynamic type boundary, user-reachable panic, or missed performance gate
  remains.
- Only then may project documentation call the toolchain 10/10.

Target release: v1.0.10.

## Recommended execution order

```text
scorecard
  → complete checker
  → adversarial safety
  → resolver/frame slots
  → profiled VM optimization
  → mechanical decomposition
  → supply-chain assurance
  → LSP/editor tooling
  → external programs and independent audit
  → v1.0.10 evidence report
```

The likely score progression is 8.8 → 9.1 after type precision → 9.3 after
safety evidence → 9.6 after performance → 9.7 after decomposition → 9.8 after
supply-chain work → 9.9 after tooling → 10 only after independent validation.

## Immediate next PR

Start with Phase 0, not frame slots. Add the scorecard, spec-to-test matrix,
type/safety inventory, and stable benchmark protocol. That baseline prevents
the later checker and VM rewrites from hiding regressions and gives every
subsequent PR an objective pass/fail gate.
