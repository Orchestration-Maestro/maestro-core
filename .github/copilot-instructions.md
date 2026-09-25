# Copilot instructions for maestro-core

## Start here

The local runtime of Maestro: knowledge kernel, retrieval, orchestration and the
command-line tools. Today it holds one crate,
[`maestro-canonicalization`](../crates/maestro-canonicalization/README.md),
which turns Markdown into provenance-bearing canonical documents, groups
duplicates and cuts them into token-budgeted chunks. The rest arrives slice by
slice ([roadmap](../docs/architecture/06-roadmap.md)).

Paths below are relative to this repository. Before editing, read
[AGENTS.md](../AGENTS.md) for the rules that bind every change,
[CONTEXT.md](../CONTEXT.md) for the words it uses and
[CONTRIBUTING.md](https://github.com/Orchestration-Maestro/.github/blob/main/CONTRIBUTING.md)
for how a change is proposed. The organization's [golden
rules](https://github.com/Orchestration-Maestro/.github/blob/main/golden-rules/engineering.md)
come first: nothing in a specification, a plan or this repository weakens them.

For quality, engineering or security changes, read
[northstar.md](../docs/standards/northstar.md),
[engineering.md](../docs/standards/engineering.md) and
[security.md](../docs/standards/security.md): this repository's map of the
organization's golden rules.

Keep changes scoped to the request, and read historical plans and specifications
as records, not as instructions to start new work.

## Repository tree

Every tracked file, with what it is for. `rust-gate guide` writes this tree at
every commit and keeps each explanation already here, so improve an explanation
in place.

```text
.                                                                            # Repository root
├── .cargo/                                                                  # Cargo settings for this workspace
│   └── mutants.toml                                                         # Mutants no test can kill, each with its reason: none changes behaviour a test can observe
├── .github/                                                                 # GitHub metadata, templates and workflows
│   ├── workflows/                                                           # GitHub Actions workflows
│   │   ├── ci.yml                                                           # CI: calls ci.yml, upload-coverage.yml, upload-sarif.yml
│   │   ├── dependabot-auto-merge.yml                                        # Dependabot auto-merge
│   │   └── scorecard.yml                                                    # OpenSSF Scorecard
│   ├── CODEOWNERS                                                           # Who reviews each path
│   ├── copilot-instructions.md                                              # This guide, written by rust-gate guide at every commit
│   ├── dependabot.yml                                                       # The organization merges only conventional titles: "ci(deps): bump ..."
│   └── zizmor.yml                                                           # The workflow security audit just check and the commit hook run: zizmor, in its pedantic persona, offline
├── crates/                                                                  # The workspace's crates
│   ├── maestro-canonicalization/                                            # Local Rust library and CLI: completed Markdown + supplied metadata → parsed structure → validation → CanonicalDocument
│   │   ├── examples/                                                        # Worked examples
│   │   │   ├── assets/                                                      # Images and other assets
│   │   │   │   └── flow.svg                                                 # SVG image: flow
│   │   │   ├── expected/                                                    # What the tool must write for the example input
│   │   │   │   ├── canonical.json                                           # JSON data: canonical
│   │   │   │   └── original.md                                              # Sample document: Operations
│   │   │   ├── input.md                                                     # Sample document: Operations
│   │   │   └── metadata.json                                                # JSON data: metadata
│   │   ├── src/                                                             # The crate's sources
│   │   │   ├── chunk_mapping/                                               # Phase B-only mappings; original source offsets are never rendered offsets
│   │   │   │   ├── mod.rs                                                   # Phase B-only mappings; original source offsets are never rendered offsets
│   │   │   │   ├── slice.rs                                                 # Mapped slices and the source accounting ledger: which original bytes each unit covers
│   │   │   │   └── tests.rs                                                 # Tests of source mapping: order, Unicode, entities, envelopes and accounting
│   │   │   ├── chunk_split/                                                 # Structural preparation and packing; only the public native path certifies counts
│   │   │   │   ├── tests/                                                   # Tests of structural preparation and packing
│   │   │   │   │   ├── boundaries.rs                                        # The pure preparation helpers: cut points, fitting prefixes and delimiter-safe ranges
│   │   │   │   │   ├── context.rs                                           # Context, characterized on small documents: the exact prepared input of each chunk
│   │   │   │   │   ├── packing.rs                                           # Packing and preparation: shared chunks, context text, containers, part numbers and the table
│   │   │   │   │   └── splitting.rs                                         # Splitting, characterized on small documents: where oversized units and rows are cut
│   │   │   │   ├── context.rs                                               # The context a chunk repeats: headings, parent items, task markers and table headers
│   │   │   │   ├── layout.rs                                                # The layout's structural queries: owners, sections, table windows and packing atoms
│   │   │   │   ├── mod.rs                                                   # Structural preparation and packing; only the public native path certifies counts
│   │   │   │   ├── prepare.rs                                               # The prepared input: body parts, formatting, sentence boundaries and fitting prefixes
│   │   │   │   └── tests.rs                                                 # Tests of structural preparation and packing
│   │   │   ├── chunks/                                                      # Derived Phase B coordinates and evidence; none of these records grant access
│   │   │   │   ├── tests/                                                   # Tests of chunk assembly, prepared-input groups and replay validation
│   │   │   │   │   ├── identity.rs                                          # Identities: chunk and prepared-input identities follow scope and content alone
│   │   │   │   │   ├── mod.rs                                               # Tests of chunk assembly, prepared-input groups and replay validation
│   │   │   │   │   └── replay.rs                                            # Replay validation: coverage and prepared parts must rebuild from the mapped source
│   │   │   │   ├── identity.rs                                              # Prepared-input groups: identical prepared inputs share one identity
│   │   │   │   ├── mod.rs                                                   # Derived Phase B coordinates and evidence; none of these records grant access
│   │   │   │   └── validation.rs                                            # Replay checks: coverage and every prepared part must rebuild from the mapped source
│   │   │   ├── tokenizer/                                                   # Local vocabulary-only tokenization through the qualified executable
│   │   │   │   ├── binding.rs                                               # Where this machine keeps the artifacts the tokenizer profile fingerprints
│   │   │   │   ├── mod.rs                                                   # Local vocabulary-only tokenization through the qualified executable
│   │   │   │   ├── process.rs                                               # The counter subprocess: bounded pipes, a timeout and a child that is always reaped
│   │   │   │   └── tests.rs                                                 # Tests of the native tokenizer: profile identity, artifacts, process limits and output
│   │   │   ├── validate/                                                    # Structural checks against the preserved bytes; no guessed repairs
│   │   │   │   ├── blocks.rs                                                # Block checks: children, parents, assets, attributes, inline content and tables
│   │   │   │   ├── mod.rs                                                   # Structural checks against the preserved bytes; no guessed repairs
│   │   │   │   ├── sections.rs                                              # Section and extractor checks: the heading hierarchy and supplied extractor anchors
│   │   │   │   └── tests.rs                                                 # Structural checks against documents altered one field at a time: each fault is reported
│   │   │   ├── accounting.rs                                                # A deterministic, unique byte partition; nested block spans remain independently valid
│   │   │   ├── assemble.rs                                                  # Build natural blocks and lexical heading context from the offset-aware tree
│   │   │   ├── content.rs                                                   # Typed natural blocks and nested inline content
│   │   │   ├── dedup.rs                                                     # Pure, scoped exact grouping; equality never merges identity or grants access
│   │   │   ├── lib.rs                                                       # Local, deterministic canonical documents
│   │   │   ├── main.rs                                                      # Local CLI for Markdown canonicalization
│   │   │   ├── metadata.rs                                                  # Merge supplied metadata without guessing provenance or permissions
│   │   │   ├── model.rs                                                     # Database-independent provenance and versioning contract
│   │   │   ├── parse.rs                                                     # Offset-aware parser tree
│   │   │   └── store.rs                                                     # Immutable snapshots, accessed through directory handles without following symlinks
│   │   ├── tests/                                                           # Integration tests
│   │   │   ├── acceptance.rs                                                # Phase A acceptance: every source byte is accounted for and no content is silently hidden
│   │   │   ├── chunk_contract.rs                                            # Public API boundary: production callers cannot supply a substitute counter
│   │   │   ├── chunk_native.rs                                              # Explicit local acceptance: never treat an ignored native test as a pass
│   │   │   ├── cli_contract.rs                                              # The command-line tool's contract: arguments, exit codes and saved documents
│   │   │   ├── dedup_contract.rs                                            # Exact duplicate grouping: stable occurrences, authorized scope and whole-batch refusals
│   │   │   ├── document_contract.rs                                         # The canonical document's contract: structure, spans and provenance as the source gives them
│   │   │   ├── properties.rs                                                # Generated Markdown dialects keep their spans, meaning and round trips, deterministically
│   │   │   └── validation_boundary.rs                                       # Regression checks for review findings at the source and JSON trust boundaries
│   │   ├── ACCEPTANCE.md                                                    # Phase A acceptance — canonicalization
│   │   ├── CHUNKING.md                                                      # Mapped structural chunking
│   │   ├── Cargo.toml                                                       # Crate manifest
│   │   ├── DEDUPLICATION.md                                                 # Scoped exact duplicate grouping
│   │   ├── README.md                                                        # Local Rust library and CLI: completed Markdown + supplied metadata → parsed structure → validation → CanonicalDocument
│   │   ├── TOKENIZER.md                                                     # Local GGUF tokenizer
│   │   ├── VERIFICATION.md                                                  # Initial verification — 2026-09-21 (historical)
│   │   └── tokenizer-contract.json                                          # JSON data: tokenizer contract
│   └── maestro-conventions/                                                 # Maestro conventions
│       ├── src/                                                             # The crate's sources
│       │   └── lib.rs                                                       # Helpers for the repository's policy tests: the files the repository holds
│       ├── tests/                                                           # Integration tests
│       │   └── policies.rs                                                  # The repository's policies, checked on every pull request by cargo test
│       └── Cargo.toml                                                       # Crate manifest: Tests that hold the maestro-core repository to its own policies
├── docs/                                                                    # Documentation
│   ├── adr/                                                                 # Hard-to-reverse decisions, each with the trade-off that produced it
│   │   ├── 0001-fresh-start-with-canonicalization-only.md                   # Fresh start: only the canonicalization crate is carried over
│   │   ├── 0002-one-authority-many-projections.md                           # One authority, many projections
│   │   ├── 0003-qdrant-server-for-search-projections.md                     # Qdrant server for vector and lexical projections
│   │   ├── 0004-neo4j-for-the-graph-projection.md                           # Neo4j Community for the graph projection
│   │   ├── 0005-copilot-native-catalog-formats.md                           # Copilot-native catalog formats are canonical
│   │   ├── 0006-in-house-event-sourced-workflow-engine.md                   # An in-house, event-sourced workflow engine
│   │   ├── 0007-cedar-for-authorization.md                                  # Cedar for tool and operation authorization
│   │   ├── 0008-chunks-budgeted-in-embedder-tokens.md                       # Chunks are budgeted in the selected embedder's tokens
│   │   ├── 0009-vendor-specific-material-stays-private.md                   # Vendor-specific material stays private
│   │   ├── 0010-spec-kit-and-executable-gates.md                            # Spec Kit for delivery; evaluations and tests are the gates
│   │   ├── 0011-models-chosen-by-bake-off.md                                # No model is preselected
│   │   ├── 0012-catalog-and-runtime-in-separate-repositories.md             # Catalog content and runtime live in separate repositories
│   │   ├── 0013-extensions-through-events-and-operations-out-of-process.md  # Extensions plug in through events and operations, out of process
│   │   ├── 0014-strict-json-for-collection-and-source-policy.md             # Strict JSON for collection and source-policy declarations
│   │   ├── 0015-bundle-freshness-and-revocation.md                          # Bundles need freshness and revocation, not only signatures
│   │   ├── 0016-native-rust-desktop-workbench.md                            # The workbench is a native Rust desktop application
│   │   ├── 0017-spec-kit-installed-once-for-the-organization.md             # Spec Kit is installed once for the organization, not committed
│   │   └── README.md                                                        # Hard-to-reverse decisions, each with the trade-off that produced it
│   ├── architecture/                                                        # Status: design of record, 2026-09-23, completed 2026-09-24
│   │   ├── 01-knowledge-pipeline.md                                         # 01 Knowledge pipeline
│   │   ├── 02-retrieval-and-knowledge-graph.md                              # 02 Retrieval and knowledge graph
│   │   ├── 03-agent-orchestration.md                                        # 03 Agent orchestration
│   │   ├── 04-intelligence-backend.md                                       # 04 Intelligence backend
│   │   ├── 05-platform-and-operations.md                                    # 05 Platform and operations
│   │   ├── 06-roadmap.md                                                    # 06 Roadmap
│   │   ├── 07-extensibility.md                                              # 07 Extensibility: entry points, exit points and extensions
│   │   ├── 08-traceability.md                                               # 08 Traceability
│   │   └── README.md                                                        # Status: design of record, 2026-09-23, completed 2026-09-24
│   └── standards/                                                           # Standards
│       ├── engineering.md                                                   # Engineering rules in maestro-core
│       ├── northstar.md                                                     # Northstar for maestro-core
│       └── security.md                                                      # Security rules in maestro-core
├── scripts/                                                                 # Maintenance scripts
│   └── bootstrap.sh                                                         # One command to get from a fresh clone to a machine that can run the gate
├── specs/                                                                   # Specifications, one directory per slice
│   ├── 000-foundation/                                                      # 000 foundation
│   │   ├── plan.md                                                          # Implementation Plan: Foundation
│   │   ├── spec.md                                                          # Feature Specification: Foundation
│   │   └── tasks.md                                                         # Foundation Implementation Tasks
│   └── 001-knowledge-kernel/                                                # 001 knowledge kernel
│       └── spec.md                                                          # Feature Specification: Knowledge kernel and hybrid RAG
├── .editorconfig                                                            # Editor settings that survive the editor
├── .gitattributes                                                           # How Git should treat each kind of file
├── .gitignore                                                               # Paths git never tracks
├── .pre-commit-config.yaml                                                  # Spec Kit writes and refreshes these files in its own style; they stay as it writes them, so a refresh changes nothing by itself
├── .taplo.toml                                                              # taplo: the TOML formatter just check and the commit hook run over every TOML file in the repository
├── .yamlfmt.yml                                                             # How yamlfmt formats every YAML file
├── AGENTS.md                                                                # Rules for coding agents: what to read, what never to weaken, how to verify
├── CONTEXT.md                                                               # The words this repository uses, and the ones it avoids
├── Cargo.lock                                                               # Exact dependency versions, committed so every build resolves the same
├── Cargo.toml                                                               # Workspace manifest: its members and the lints every member inherits
├── LICENSE                                                                  # The licence this repository is distributed under
├── README.md                                                                # The local runtime of Maestro: knowledge kernel, retrieval, orchestration and the command-line tools
├── clippy.toml                                                              # The size limits the North Star holds every function to; the justfile denies the two lints that are off by default
├── deny.toml                                                                # Licence, dependency-ban, source and yanked-crate policy for every Cargo manifest in this repository
├── justfile                                                                 # List every recipe and what it does; this is what just alone prints
├── mise.lock                                                                # The checksum of every pinned tool download
├── mise.toml                                                                # The development toolbelt: every tool just check needs, at the version CI pins
├── rust-toolchain.toml                                                      # The pinned Rust toolchain
└── typos.toml                                                               # Spelling checks for maintained code and documentation
```

## Change and verification procedure

1. Read the rules in AGENTS.md that cover the files you change, and keep every
   gate intact: never weaken one to pass.
2. Add an executable regression check for a change in behaviour.
3. The commit hook `rust-gate guide` rewrites this guide when a file is added,
   moved or removed; commit it with the change. The organization's daily drift
   check reports a guide left stale.
4. Run `scripts/bootstrap.sh` once, then `just check`, and report the commands
   you actually ran.
5. Commits are signed, with a conventional title; the default branch takes only
   squash-merged pull requests.
