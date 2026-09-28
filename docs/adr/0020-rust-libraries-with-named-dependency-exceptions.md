# Rust libraries join the stack; the duplicates they force are named exceptions

Status: accepted, 2026-09-26; graph direction amended on 2026-09-28 by
[ADR-0021](0021-embedded-ladybug-graph-projection.md).

Maestro builds on the Rust ecosystem's libraries rather than on hand-written
clients: `qdrant-client` for the search projections (ADR-0003), owner-approved
`lbug` for the embedded S2 graph, `docling` (docling.rs) for document
conversion, `spider` and `dom_smoothie` for acquisition. `lbug` adoption remains
blocked on G25's measured qualification: exact pin, minimum features, native
links, licences, vet evidence and at most one named forced duplicate. S2 adds
no `neo4rs` dependency. The later user-selected Neo4j adapter belongs to
separate deployment-modes work; its driver must be qualified there.

The organization's one-version rule (DEP-001) stays the default. The ecosystem
is midway through several major transitions (`syn` 2 to 3, `getrandom` 0.2 to 0.3,
`windows-sys`), so an adopted library can force a second version of a crate
maestro-core already uses. Each such duplicate is a DEP-001 exception in
`maestro-quality.toml` naming the crate and version, the library that forces
it and the version that ends it, and it is removed when the library moves.

Before a library is adopted, its tree is measured against the workspace's
lock: the crates it adds, the second versions it forces, the native libraries
it links. It is taken with the fewest features it needs, and the numbers go
into the task's brief. A library whose native link clashes with the core's is
taken only with features that avoid the clash.

Historical measurements on 2026-09-26 against maestro-core's 128 crates,
not an adoption receipt for S2 or a substitute for G25's lbug measurements:

| Library | Crates in its tree | New to maestro-core | Second versions it forces |
| --- | ---: | ---: | --- |
| `qdrant-client` 1.19.0, no default features | 128 | 78 | 4: `syn`, `base64`, `getrandom`, `windows-sys` |
| `neo4rs` 0.9.0-rc.10 | 122 | 84 | 4: `syn`, `getrandom`, `socket2`, `windows-sys` |
| `dom_smoothie` 0.18.2 | 61 | 45 | 1: `syn` |
| `docling` 1.69.2, no default features to defaults | 146 to 439 | 120 to 363 | 9 to 23 |
| `spider` 2.53.9, no default features to defaults | 268 to 424 | 186 to 331 | 15 to 38, a second `reqwest` among them |

`spider`'s default features also link a second SQLite (`libsqlite3-sys` 0.30
beside the kernel's 0.38), which no build can link; without them it links
none.

## Considered options

- Hand-written HTTP clients, T026's first design: no duplicates, but maestro
  would maintain protocol code the vendors already maintain, for every
  service. Rejected.
- Relaxing DEP-001 for the whole organization, or for build-only crates such
  as `syn`: fewer exceptions, but the rule's readers would no longer see which
  library holds which duplicate. The build-only case, procedural-macro
  dependencies that ship in no binary, is worth proposing to the gate's
  maintainers; until the gate distinguishes it, exceptions.
- Every large library in its own process, ADR-0013's extension model: it
  isolates crashes and untrusted input at the cost of a protocol per library.
  Kept as a choice each adoption weighs, for example a crawler or a PDF parser
  reading untrusted input, not as a rule.

## Consequences

`maestro-quality.toml` gains an exception for each forced duplicate, with its
removal condition, and `cargo vet` gains an exemption or an audit for each new
crate. Builds grow with each library, since a second `syn` compiles twice;
binaries grow less, since build-only duplicates ship in none. Upgrading a
library is a task: measure again and drop the exceptions it no longer forces.
