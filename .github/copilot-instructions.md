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
│   │   │   ├── .rumdl.toml                                                  # The example is a fixture, not documentation: input.md is Markdown as an author wrote it, duplicate headings and all
│   │   │   ├── input.md                                                     # Sample document: Operations
│   │   │   └── metadata.json                                                # JSON data: metadata
│   │   ├── src/                                                             # The crate's sources
│   │   │   ├── chunk_mapping/                                               # Phase B-only mappings; original source offsets are never rendered offsets
│   │   │   │   ├── document.rs                                              # Mapping a canonical document into source units: each block's context and inline text, with
│   │   │   │   ├── mod.rs                                                   # Phase B-only mappings; original source offsets are never rendered offsets
│   │   │   │   ├── refusal.rs                                               # The refusal every mapping check returns
│   │   │   │   ├── slice.rs                                                 # Mapped slices and the source accounting ledger: which original bytes each unit covers
│   │   │   │   └── tests.rs                                                 # Tests of source mapping: order, Unicode, entities, envelopes and accounting
│   │   │   ├── chunk_split/                                                 # Structural preparation and packing; only chunk_documents, through a verified TokenCounter, certifies counts
│   │   │   │   ├── tests/                                                   # Tests of structural preparation and packing
│   │   │   │   │   ├── boundaries.rs                                        # The pure preparation helpers: cut points, fitting prefixes and delimiter-safe ranges
│   │   │   │   │   ├── context.rs                                           # Context, characterized on small documents: the exact prepared input of each chunk
│   │   │   │   │   ├── mod.rs                                               # Tests of structural preparation and packing
│   │   │   │   │   ├── packing.rs                                           # Packing and preparation: shared chunks, context text, containers, part numbers and the table
│   │   │   │   │   └── splitting.rs                                         # Splitting, characterized on small documents: where oversized units and rows are cut
│   │   │   │   ├── context.rs                                               # The context a chunk repeats: headings, parent items, task markers and table headers
│   │   │   │   ├── drafts.rs                                                # Packing a document's atoms into drafts: combined up to the target, refined or split past the
│   │   │   │   ├── layout.rs                                                # The layout's structural queries: owners, sections, table windows and packing atoms
│   │   │   │   ├── limits.rs                                                # The token budgets drafts grow toward and never exceed
│   │   │   │   ├── mod.rs                                                   # Structural preparation and packing; only chunk_documents, through a verified TokenCounter, certifies counts
│   │   │   │   ├── prepare.rs                                               # The prepared input: body parts, formatting, sentence boundaries and fitting prefixes
│   │   │   │   ├── refusal.rs                                               # The refusal every structural check returns
│   │   │   │   ├── replay.rs                                                # Replaying a chunk's preparation: its table windows against its fragments, then the chunk
│   │   │   │   └── structure.rs                                             # A document's structure indexed for chunking, the bodies packed from it and the context they
│   │   │   ├── chunks/                                                      # Phase B chunk batches: assembly, prepared-input identities and replay validation
│   │   │   │   ├── tests/                                                   # Tests of chunk assembly, prepared-input groups and replay validation
│   │   │   │   │   ├── counter.rs                                           # Tests of the counting seam: any TokenCounter chunks, verified around its batch
│   │   │   │   │   ├── identity.rs                                          # Identities: chunk and prepared-input identities follow scope and content alone
│   │   │   │   │   ├── mod.rs                                               # Tests of chunk assembly, prepared-input groups and replay validation
│   │   │   │   │   └── replay.rs                                            # Replay validation: coverage and prepared parts must rebuild from the mapped source
│   │   │   │   ├── batch.rs                                                 # Batch records: per-occurrence evidence, retrieval chunks and prepared-input groups
│   │   │   │   ├── build.rs                                                 # Batch assembly: map, split, replay and identify every authorized occurrence
│   │   │   │   ├── identity.rs                                              # Prepared-input groups: identical prepared inputs share one identity
│   │   │   │   ├── mod.rs                                                   # Phase B chunk batches: assembly, prepared-input identities and replay validation
│   │   │   │   └── validation.rs                                            # Replay checks: coverage and every prepared part must rebuild from the mapped source
│   │   │   ├── filesystem/                                                  # Filesystem access that never follows a link, behind one interface: rustix's directory-relative
│   │   │   │   ├── mod.rs                                                   # Filesystem access that never follows a link, behind one interface: rustix's directory-relative
│   │   │   │   ├── root.rs                                                  # The root a caller names, resolved once, and the names the store appends below it
│   │   │   │   ├── unix.rs                                                  # Unix: every name resolves against an open directory through rustix's openat family, which
│   │   │   │   └── windows.rs                                               # Windows: names resolve by path, but every directory on the way is held open without
│   │   │   ├── tokenizer/                                                   # Token counting: the TokenCounter seam, and the qualified local executable that fills it
│   │   │   │   ├── tests/                                                   # Tests of the native tokenizer: profile identity, artifacts, process limits and output
│   │   │   │   │   ├── invocation.rs                                        # Tests of the counter's invocation: its environment, its arguments and each platform's loader
│   │   │   │   │   ├── libraries.rs                                         # Tests of the library inventory: each platform's naming, its aliases and where they resolve
│   │   │   │   │   └── mod.rs                                               # Tests of the native tokenizer: profile identity, artifacts, process limits and output
│   │   │   │   ├── artifacts.rs                                             # Artifact checks: pinned files by size and SHA-256, and the exact library inventory
│   │   │   │   ├── binding.rs                                               # Where this machine keeps the artifacts the tokenizer profile fingerprints
│   │   │   │   ├── contract.rs                                              # The committed qualification profile: its identity and typed access to its fields
│   │   │   │   ├── counter.rs                                               # The seam every token counter fills: what the chunker counts through, whoever counts
│   │   │   │   ├── loader.rs                                                # How each platform's dynamic loader is made to load the verified libraries, and only them
│   │   │   │   ├── mod.rs                                                   # Token counting: the TokenCounter seam, and the qualified local executable that fills it
│   │   │   │   ├── native.rs                                                # The pinned native tokenizer: verified artifacts and one counter process per input
│   │   │   │   └── process.rs                                               # The counter subprocess: bounded pipes, a timeout and a child that is always reaped
│   │   │   ├── validate/                                                    # Structural checks against the preserved bytes; no guessed repairs
│   │   │   │   ├── blocks.rs                                                # Block checks: children, parents, assets, attributes, inline content and tables
│   │   │   │   ├── mod.rs                                                   # Structural checks against the preserved bytes; no guessed repairs
│   │   │   │   ├── report.rs                                                # How a check records a finding located at a block's spans
│   │   │   │   ├── sections.rs                                              # Section and extractor checks: the heading hierarchy and supplied extractor anchors
│   │   │   │   ├── structure.rs                                             # Whole-document structural checks: reference, block hierarchy, source ledger and coverage
│   │   │   │   └── tests.rs                                                 # Structural checks against documents altered one field at a time: each fault is reported
│   │   │   ├── accounting.rs                                                # A deterministic, unique byte partition; nested block spans remain independently valid
│   │   │   ├── assemble.rs                                                  # Build natural blocks and lexical heading context from the offset-aware tree
│   │   │   ├── cli.rs                                                       # The command line: its usage, its flags, the metadata sidecar and the run that canonicalizes
│   │   │   ├── content.rs                                                   # Typed natural blocks and nested inline content
│   │   │   ├── dedup.rs                                                     # Pure, scoped exact grouping; equality never merges identity or grants access
│   │   │   ├── document.rs                                                  # The canonical document: the versioned record canonicalization returns, its heading sections
│   │   │   ├── error.rs                                                     # The crate's one error type: a refusal that says why, never the source text
│   │   │   ├── hashing.rs                                                   # SHA-256 as lower-case hexadecimal, the form every identity and content hash takes
│   │   │   ├── lib.rs                                                       # Local, deterministic canonical documents
│   │   │   ├── local_assets.rs                                              # The binary's filesystem look-up of local asset destinations; the library itself does no I/O
│   │   │   ├── main.rs                                                      # Local CLI for Markdown canonicalization
│   │   │   ├── metadata.rs                                                  # Merge supplied metadata without guessing provenance or permissions
│   │   │   ├── model.rs                                                     # Database-independent provenance and versioning contract
│   │   │   ├── parse.rs                                                     # Offset-aware parser tree
│   │   │   ├── pipeline.rs                                                  # The canonicalization pipeline: parse, merge metadata, assemble blocks, keep what the parser
│   │   │   ├── prepared_inputs.rs                                           # The prepared embedding input: its parts, primary fragments and table windows
│   │   │   ├── replay.rs                                                    # Validation by replay: canonicalize the reference bytes again with a document's recorded inputs
│   │   │   ├── source_units.rs                                              # Mapped source units: derived-text ranges and how each run relates to original source
│   │   │   └── store.rs                                                     # Immutable snapshots, accessed through directory handles without following symlinks
│   │   ├── tests/                                                           # Integration tests
│   │   │   └── it/                                                          # The crate's integration tests, built as one test crate
│   │   │       ├── cli_contract/                                            # The command-line tool's contract: arguments, exit codes and saved documents
│   │   │       │   ├── fixture.rs                                           # The scratch directory each command-line test runs the tool in
│   │   │       │   ├── invocation_contract.rs                               # Running the tool: arguments, exit codes, source bytes, sidecars, assets
│   │   │       │   ├── mod.rs                                               # The command-line tool's contract: arguments, exit codes and saved documents
│   │   │       │   └── snapshot_contract.rs                                 # Saved snapshots: publication, recovery, verified loading and the roots
│   │   │       ├── dedup_contract/                                          # Exact duplicate grouping: stable occurrences, authorized scope and whole-batch refusals
│   │   │       │   ├── authorized_scope.rs                                  # The authorized scope bounds each group, and an invalid, duplicate
│   │   │       │   ├── batch_inputs.rs                                      # The documents and the authorized scope each grouping test starts from
│   │   │       │   ├── content_equality.rs                                  # What groups occurrences: equal content in a stable order, canonical
│   │   │       │   ├── mod.rs                                               # Exact duplicate grouping: stable occurrences, authorized scope and whole-batch refusals
│   │   │       │   └── revision_history.rs                                  # Revisions of one document grouped together keep their own policy
│   │   │       ├── chunk_contract.rs                                        # Public API boundary: a counter from another crate chunks through the crate-root trait
│   │   │       ├── chunk_native.rs                                          # Explicit local acceptance: never treat an ignored native test as a pass
│   │   │       ├── dialect_properties.rs                                    # Generated Markdown dialects keep their spans, meaning and round trips, deterministically
│   │   │       ├── document_contract.rs                                     # The canonical document's contract: structure, spans and provenance as the source gives them
│   │   │       ├── main.rs                                                  # The crate's integration tests, built as one test crate: each module proves
│   │   │       ├── phase_a_acceptance.rs                                    # Phase A acceptance: every source byte is accounted for and no content is silently hidden
│   │   │       └── validation_boundary.rs                                   # Regression checks for review findings at the source and JSON trust boundaries
│   │   ├── ACCEPTANCE.md                                                    # Phase A acceptance — canonicalization
│   │   ├── CHUNKING.md                                                      # Mapped structural chunking
│   │   ├── Cargo.toml                                                       # Crate manifest
│   │   ├── DEDUPLICATION.md                                                 # Scoped exact duplicate grouping
│   │   ├── README.md                                                        # Local Rust library and CLI: completed Markdown + supplied metadata → parsed structure → validation → CanonicalDocument
│   │   ├── TOKENIZER.md                                                     # Local GGUF tokenizer
│   │   ├── VERIFICATION.md                                                  # Initial verification — 2026-09-21 (historical)
│   │   └── tokenizer-contract.json                                          # JSON data: tokenizer contract
│   ├── maestro-conventions/                                                 # Maestro conventions
│   │   ├── src/                                                             # The crate's sources
│   │   │   └── lib.rs                                                       # Helpers for the repository's policy tests: the files the repository holds
│   │   ├── tests/                                                           # Integration tests
│   │   │   └── policies.rs                                                  # The repository's policies, checked on every pull request by cargo test
│   │   └── Cargo.toml                                                       # Crate manifest: Tests that hold the maestro-core repository to its own policies
│   ├── maestro-kernel/                                                      # Maestro kernel
│   │   ├── migrations/                                                      # The kernel database's migrations, embedded and applied in number order
│   │   │   ├── 0001_artifacts.sql                                           # The artifacts table: each artifact's size, media type, pins and creation time
│   │   │   ├── 0002_journal.sql                                             # The journal: the append-only events, their triggers, and the consumers' cursors
│   │   │   ├── 0003_scopes.sql                                              # The grants: each principal's rights on a scope and on every scope below it
│   │   │   ├── 0004_documents.sql                                           # The pipeline's records: collections, sources, documents, revisions, their dispositions, chunk sets, chunks and generations
│   │   │   └── 0005_jobs.sql                                                # The jobs table: each job's key, attempt, resource, state, lease and outcome, and its triggers
│   │   ├── src/                                                             # The crate's sources
│   │   │   ├── artifact/                                                    # Content-addressed artifacts: immutable bytes stored, and read back, by their
│   │   │   │   ├── digest.rs                                                # A SHA-256 digest: the name every artifact is stored under
│   │   │   │   ├── mod.rs                                                   # Content-addressed artifacts: immutable bytes stored, and read back, by their
│   │   │   │   ├── store.rs                                                 # The store: artifacts written once under their digest, read back checked
│   │   │   │   └── tests.rs                                                 # Tests of the artifact store: digests, writes, repairs and refusals, with
│   │   │   ├── capability/                                                  # Capabilities: the registry of the tools Maestro offers (building block B9)
│   │   │   │   ├── mod.rs                                                   # Capabilities: the registry of the tools Maestro offers (building block B9)
│   │   │   │   ├── registry.rs                                              # The registry: each tool once, with what it takes, does and needs
│   │   │   │   └── tests.rs                                                 # Tests of the capability registry: declarations, refusals and order
│   │   │   ├── document/                                                    # The pipeline's document records (building block B5; docs/architecture/01
│   │   │   │   ├── tests/                                                   # Tests of the document records: the documents migration, collections
│   │   │   │   │   ├── errors.rs                                            # What the document records' refusals say, and the store's refusals they
│   │   │   │   │   ├── mod.rs                                               # Tests of the document records: the documents migration, collections
│   │   │   │   │   ├── parents.rs                                           # Collections, their sources and their documents: collections and sources
│   │   │   │   │   ├── revisions.rs                                         # Revisions: recorded once with their two artifacts pinned, immutable after
│   │   │   │   │   ├── schema.rs                                            # The documents migration: the ten pipeline tables of 01 §11, all strict
│   │   │   │   │   └── support.rs                                           # What the record tests share: a scratch database, and the collection
│   │   │   │   ├── collection.rs                                            # Collections, the sources they declare and the documents those sources
│   │   │   │   ├── error.rs                                                 # Why the kernel refused to record a collection, a source, a document or a revision
│   │   │   │   ├── mod.rs                                                   # The pipeline's document records (building block B5; docs/architecture/01
│   │   │   │   └── revision.rs                                              # Revisions: one exact version of a document's bytes and metadata, recorded
│   │   │   ├── gateway/                                                     # The model gateway (building block B10): every model, embedder, reranker
│   │   │   │   ├── tests/                                                   # Tests of the model gateway: model cards, the router client against a stub
│   │   │   │   │   ├── card.rs                                              # Tests of model cards: strict JSON artifacts whose digest is their
│   │   │   │   │   ├── fake.rs                                              # Tests of the deterministic fake: its outputs are fixed by its inputs, the
│   │   │   │   │   ├── fixture.rs                                           # What the gateway's tests share: a scratch store, a card for each role, and
│   │   │   │   │   ├── mod.rs                                               # Tests of the model gateway: model cards, the router client against a stub
│   │   │   │   │   ├── port.rs                                              # Tests of the port's refusals: each says what was refused and why
│   │   │   │   │   ├── router.rs                                            # Tests of the router client against a stub router: every call is bound to
│   │   │   │   │   └── stub.rs                                              # A stub of the model router: a loopback HTTP server, on a thread of its
│   │   │   │   ├── card.rs                                                  # Model cards: what was evaluated of a model filling a role (D8), kept as
│   │   │   │   ├── fake.rs                                                  # The deterministic fake behind the model port, which public CI uses since it
│   │   │   │   ├── mod.rs                                                   # The model gateway (building block B10): every model, embedder, reranker
│   │   │   │   ├── port.rs                                                  # The model port: the calls every way of reaching a model answers, each
│   │   │   │   └── router.rs                                                # The router client: the model port over maestro-model-router's dedicated
│   │   │   ├── generation/                                                  # The search generations of a collection (building block B6; plan D9): each
│   │   │   │   ├── tests/                                                   # Tests of the generation records: their lifecycle and their publication
│   │   │   │   │   ├── lifecycle.rs                                         # A generation's lifecycle: created building, then verified, published and
│   │   │   │   │   ├── mod.rs                                               # Tests of the generation records: their lifecycle and their publication
│   │   │   │   │   ├── publication.rs                                       # Publication: at most one generation of a collection is published, which
│   │   │   │   │   └── support.rs                                           # What the generation tests share: a scratch database holding two
│   │   │   │   ├── error.rs                                                 # Why the kernel refused to create or move a generation
│   │   │   │   ├── lifecycle.rs                                             # Generations as the kernel records them, and the calls that create and move
│   │   │   │   ├── mod.rs                                                   # The search generations of a collection (building block B6; plan D9): each
│   │   │   │   └── state.rs                                                 # The states a generation moves through, and the one move each allows
│   │   │   ├── job/                                                         # Jobs: long work, such as an import, a preparation or a publication, run
│   │   │   │   ├── tests/                                                   # Tests of jobs: their keys and attempts, their leases and states, their
│   │   │   │   │   ├── changes.rs                                           # Changes: each change of a job recorded on its stream of the journal, with
│   │   │   │   │   ├── child.rs                                             # Not a test of its own: the first process of the resume test, which works
│   │   │   │   │   ├── errors.rs                                            # Refusals: what each one says, and a stored job the kernel cannot read
│   │   │   │   │   ├── leases.rs                                            # Leases: one holder at a time, taken over once expired, renewed by
│   │   │   │   │   ├── mod.rs                                               # Tests of jobs: their keys and attempts, their leases and states, their
│   │   │   │   │   ├── progress.rs                                          # Progress: recorded on the job's stream of the journal in the write that
│   │   │   │   │   ├── resources.rs                                         # Resources: what a job holds exclusively while it is queued or running
│   │   │   │   │   ├── resume.rs                                            # A job interrupted mid-way: its first process dies, and a second process
│   │   │   │   │   ├── scopes.rs                                            # Scopes: a job is read only through a set that covers the scope it works
│   │   │   │   │   ├── states.rs                                            # States: a job moves only forward, and its three outcomes are final
│   │   │   │   │   ├── submit.rs                                            # Submitting a job: its ID, its idempotency key, and the job a retried
│   │   │   │   │   ├── support.rs                                           # What the job tests share: a scratch data directory, the publication they
│   │   │   │   │   └── table.rs                                             # The jobs table: what the database itself refuses, whoever writes, so no
│   │   │   │   ├── error.rs                                                 # Why the kernel refused to submit, lease, move or read a job
│   │   │   │   ├── events.rs                                                # The events of a job: each change and each step, recorded on the job's
│   │   │   │   ├── lease.rs                                                 # Leases: taken by one holder at a time, taken over once expired, and
│   │   │   │   ├── mod.rs                                                   # Jobs: long work, such as an import, a preparation or a publication, run
│   │   │   │   ├── outcome.rs                                               # Outcomes: a job ends succeeded, failed or cancelled, and stays so
│   │   │   │   ├── progress.rs                                              # Progress: each step of a job recorded on its stream of the journal, in
│   │   │   │   ├── record.rs                                                # Jobs as the kernel records them, their leases, and the calls that submit
│   │   │   │   └── state.rs                                                 # The states a job moves through, and the moves it may make
│   │   │   ├── journal/                                                     # The journal: every change the kernel makes, recorded as an event in one
│   │   │   │   ├── tests/                                                   # Tests of the journal: its events and their streams, its cursors, and what
│   │   │   │   │   ├── append_only.rs                                       # The journal is append-only: the database itself refuses to update, delete
│   │   │   │   │   ├── child.rs                                             # Not a test of its own: what the child processes of the crash and
│   │   │   │   │   ├── concurrency.rs                                       # Writers together: threads sharing the database and processes of their own
│   │   │   │   │   ├── crash.rs                                             # A crash: the event a process committed before it is read after the
│   │   │   │   │   ├── cursors.rs                                           # Cursors: where each consumer stands in each stream, moved forward only
│   │   │   │   │   ├── events.rs                                            # Events: the ID and the sequence recording gives them, the event it
│   │   │   │   │   ├── mod.rs                                               # Tests of the journal: its events and their streams, its cursors, and what
│   │   │   │   │   └── support.rs                                           # What the journal tests share: a scratch data directory, the events they
│   │   │   │   ├── cursor.rs                                                # Cursors: how far each consumer has read each stream, moved forward only
│   │   │   │   ├── error.rs                                                 # Why the journal refused an operation
│   │   │   │   ├── event.rs                                                 # Events: recorded with a new ID and the next sequence of their stream
│   │   │   │   └── mod.rs                                                   # The journal: every change the kernel makes, recorded as an event in one
│   │   │   ├── scope/                                                       # Scopes and grants: who may see what (docs/architecture/04 §3, building
│   │   │   │   ├── tests/                                                   # Tests of scopes: their paths and names, what a grant covers, the grants
│   │   │   │   │   ├── config.rs                                            # config.toml: the local principal's grants, checked whole when read, and
│   │   │   │   │   ├── grants.rs                                            # Grants: a principal sees only what it was granted and what lies below it
│   │   │   │   │   ├── inventory.rs                                         # There is no read function without a ScopeSet: every public method of
│   │   │   │   │   ├── mod.rs                                               # Tests of scopes: their paths and names, what a grant covers, the grants
│   │   │   │   │   ├── paths.rs                                             # Scope paths: a workspace, then a collection, then a source, each named by
│   │   │   │   │   ├── readers.rs                                           # Readers of scoped data take the caller's ScopeSet and filter inside
│   │   │   │   │   ├── records.rs                                           # The readers of the pipeline's records take the caller's ScopeSet and
│   │   │   │   │   └── support.rs                                           # What the scope tests share: a scratch directory for the kernel's data and
│   │   │   │   ├── config.rs                                                # config.toml, the kernel's configuration file in its configuration
│   │   │   │   ├── grant.rs                                                 # Grants: the rights of principals on scopes, each given or taken back in a
│   │   │   │   ├── mod.rs                                                   # Scopes and grants: who may see what (docs/architecture/04 §3, building
│   │   │   │   ├── path.rs                                                  # Scope paths: a workspace, then optionally a collection, then optionally a
│   │   │   │   ├── right.rs                                                 # The rights a grant gives on a scope
│   │   │   │   └── set.rs                                                   # The scopes a principal may read, found for one request, and the condition
│   │   │   ├── store/                                                       # The kernel's database: one SQLite file beside the artifact store, holding
│   │   │   │   ├── tests/                                                   # Tests of the kernel database: its migrations, its connections, the
│   │   │   │   │   ├── artifacts.rs                                         # Artifacts: stored, recorded with their size, media type and pins, and read
│   │   │   │   │   ├── connections.rs                                       # Connections: one writer shared by every thread, readers of their own, the
│   │   │   │   │   ├── garbage.rs                                           # Garbage collection: it lists before it removes, removes only artifacts
│   │   │   │   │   ├── migrations.rs                                        # Migrations: applied in number order, each once, recorded by name, and a
│   │   │   │   │   ├── mod.rs                                               # Tests of the kernel database: its migrations, its connections, the
│   │   │   │   │   └── support.rs                                           # What the database tests share: scratch directories, the digests of their
│   │   │   │   ├── artifacts.rs                                             # The artifacts table: what the artifact store holds, the pins that keep
│   │   │   │   ├── database.rs                                              # The database: its file, one writer connection behind a mutex, and readers
│   │   │   │   ├── error.rs                                                 # Why the kernel's database refused an operation
│   │   │   │   ├── migration.rs                                             # The migrations: the SQL files of migrations/, embedded in the binary
│   │   │   │   └── mod.rs                                                   # The kernel's database: one SQLite file beside the artifact store, holding
│   │   │   ├── telemetry/                                                   # Telemetry: pinned span names and component health (building block B11)
│   │   │   │   ├── health.rs                                                # Health: how each component is doing, asked of its own check
│   │   │   │   ├── mod.rs                                                   # Telemetry: pinned span names and component health (building block B11)
│   │   │   │   ├── span.rs                                                  # The spans the kernel opens, and the names they carry, pinned in one place
│   │   │   │   └── tests.rs                                                 # Tests of telemetry: component health and the names of a tool call's span
│   │   │   ├── binding.rs                                                   # Named bindings: the local paths that the logical names of committed files
│   │   │   ├── filesystem.rs                                                # The files and directories the kernel creates: its owner's only, and each
│   │   │   ├── lib.rs                                                       # The kernel of Maestro: the single authoritative store every later
│   │   │   └── paths.rs                                                     # Where the kernel keeps its data: $XDG_DATA_HOME/maestro when that names an
│   │   └── Cargo.toml                                                       # Crate manifest: The single authoritative store of Maestro, starting with its content-addressed artifacts
│   └── maestro-knowledge/                                                   # Maestro knowledge
│       ├── src/                                                             # The crate's sources
│       │   ├── prepare/                                                     # Preparing revisions for search: tokens counted as the selected embedder counts them, through the model router
│       │   │   ├── tests/                                                   # Tests of the router tokenizer: qualification by parity with the native
│       │   │   │   ├── counting.rs                                          # Counting and verifying through a qualified tokenizer: the port's IDs in
│       │   │   │   ├── mod.rs                                               # Tests of the router tokenizer: qualification by parity with the native
│       │   │   │   ├── parity.rs                                            # The parity fixtures: how their file writes them, and what the built-in
│       │   │   │   ├── port.rs                                              # A model port that answers each parity fixture with the native counter's
│       │   │   │   ├── qualification.rs                                     # Qualification: a router tokenizer exists only once the port gives every
│       │   │   │   ├── refusals.rs                                          # The refusals: what each says, and the cause each keeps
│       │   │   │   ├── router_client.rs                                     # The router client through a router tokenizer, against a stub router: from
│       │   │   │   ├── stub.rs                                              # A stub of the model router for the router client's tests: a loopback HTTP
│       │   │   │   └── support.rs                                           # What the router tokenizer's tests share: model cards, recorded in a
│       │   │   ├── bridge.rs                                                # A model port's asynchronous tokenize, called synchronously: the port's
│       │   │   ├── error.rs                                                 # Why a router tokenizer refuses to qualify, or to count
│       │   │   ├── mod.rs                                                   # Preparing revisions for search (docs/architecture/01 §7): their chunks are
│       │   │   ├── native-parity.json                                       # The native counter's ordered IDs for the 41 parity fixtures a router tokenizer must match to qualify
│       │   │   ├── parity.rs                                                # The native profile's parity fixtures, native-parity.json: complete
│       │   │   └── router_tokenizer.rs                                      # The router tokenizer: maestro-canonicalization's TokenCounter over the
│       │   ├── collection.rs                                                # A collection's declaration: maestro-collection/1, the strict JSON that
│       │   ├── corpus.rs                                                    # A corpus manifest: maestro-corpus/1, one JSON line per document, through
│       │   ├── lib.rs                                                       # The knowledge pipeline of Maestro (docs/architecture/01): collections, their
│       │   ├── relative_path.rs                                             # Paths that a declaration or a manifest gives relative to a directory, which
│       │   ├── shape.rs                                                     # The JSON shapes the contracts name, and no other: an object where a
│       │   └── suite.rs                                                     # An evaluation suite: maestro-suite/1, one JSON line per question, which
│       ├── tests/                                                           # Integration tests
│       │   └── it/                                                          # It
│       │       ├── collection_contract.rs                                   # maestro-collection/1: a strict declaration parses into typed values; an
│       │       ├── corpus_contract.rs                                       # maestro-corpus/1: one line per document parses into typed values; an
│       │       ├── main.rs                                                  # The crate's integration tests, built as one test crate: each module proves
│       │       ├── router_parity.rs                                         # The router tokenizer's parity with the native counter, live: an explicit
│       │       ├── suite_contract.rs                                        # maestro-suite/1: a suite, one JSON line per question, parses into typed
│       │       ├── suite_resolution.rs                                      # Resolving an expected section in its canonicalized document: a heading path
│       │       └── synthetic_collection.rs                                  # The public synthetic collection, tests/fixtures/synthetic, which stands in
│       └── Cargo.toml                                                       # Crate manifest: The knowledge pipeline of Maestro, starting with the collection and corpus contracts it imports through
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
│   │   ├── 0018-rustix-on-unix-and-win32-flags-on-windows.md                # The snapshot store uses rustix on Unix and Win32 flags on Windows
│   │   ├── 0019-reverse-engineering-is-analysis-behind-a-clean-room.md      # Reverse engineering produces knowledge only, behind a clean-room boundary
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
│   │   ├── 09-reverse-engineering.md                                        # 09 Reverse engineering and provenance
│   │   └── README.md                                                        # Status: design of record, 2026-09-23, completed 2026-09-24
│   └── standards/                                                           # Standards
│       ├── engineering.md                                                   # Engineering rules in maestro-core
│       ├── northstar.md                                                     # Northstar for maestro-core
│       └── security.md                                                      # Security rules in maestro-core
├── specs/                                                                   # Specifications, one directory per slice
│   ├── 000-foundation/                                                      # 000 foundation
│   │   ├── plan.md                                                          # Implementation Plan: Foundation
│   │   ├── spec.md                                                          # Feature Specification: Foundation
│   │   └── tasks.md                                                         # Foundation Implementation Tasks
│   └── 001-knowledge-kernel/                                                # 001 knowledge kernel
│       ├── checklists/                                                      # Checklists
│       │   └── requirements.md                                              # Specification Quality Checklist: Knowledge kernel and hybrid RAG
│       ├── plan.md                                                          # Implementation Plan: Knowledge kernel and hybrid RAG
│       ├── research.md                                                      # Research: Knowledge kernel and hybrid RAG
│       ├── spec.md                                                          # Feature Specification: Knowledge kernel and hybrid RAG
│       └── tasks.md                                                         # Knowledge Kernel and Hybrid RAG Implementation Tasks
├── supply-chain/                                                            # cargo-vet audits, configuration and imports
│   ├── audits.toml                                                          # cargo-vet audits file
│   ├── config.toml                                                          # cargo-vet config file
│   └── imports.lock                                                         # The audits cargo-vet imports, locked
├── tests/                                                                   # Test data shared by the workspace's crates
│   └── fixtures/                                                            # Test fixtures
│       └── synthetic/                                                       # The public synthetic collection and its suite, which stand in for the private corpus in public CI (ADR-0009)
│           ├── corpus/                                                      # The collection's one source: its maestro-corpus/1 manifest beside the Markdown documents it names
│           │   ├── en/                                                      # The documents written in English
│           │   │   ├── backups/                                             # Backups
│           │   │   │   ├── backup-policy.md                                 # Sample document: Backup policy
│           │   │   │   └── restoring-a-database.md                          # Sample document: Restoring a database from a backup
│           │   │   ├── databases/                                           # Databases
│           │   │   │   └── schema-migrations.md                             # Sample document: Running schema migrations
│           │   │   ├── http/                                                # HTTP APIs
│           │   │   │   ├── error-codes.md                                   # Sample document: HTTP error codes
│           │   │   │   └── pagination.md                                    # Sample document: Paginating API results
│           │   │   ├── logging/                                             # Logging
│           │   │   │   └── log-rotation.md                                  # Sample document: Log rotation
│           │   │   ├── messaging/                                           # Messaging
│           │   │   │   └── dead-letter-queues.md                            # Sample document: Dead-letter queues
│           │   │   ├── operations/                                          # Operations
│           │   │   │   ├── payments/                                        # The payments team's runbooks
│           │   │   │   │   └── on-call-handover.md                          # Sample document: On-call handover, a near-duplicate of operations/on-call-handover.md on purpose (T023)
│           │   │   │   ├── glossary.md                                      # Sample document: Glossary, an exact copy of en/glossary.md on purpose (T023)
│           │   │   │   └── on-call-handover.md                              # Sample document: On-call handover
│           │   │   ├── scheduling/                                          # Scheduling
│           │   │   │   ├── job-retries.md                                   # Sample document: Retrying scheduled jobs
│           │   │   │   └── schedule-expressions.md                          # Sample document: Schedule expressions
│           │   │   ├── tls/                                                 # TLS certificates
│           │   │   │   ├── certificate-renewal-4.1.md                       # Sample document: Renewing TLS certificates as release 4.1 had it, the older of two versions on purpose (T032)
│           │   │   │   └── certificate-renewal.md                           # Sample document: Renewing TLS certificates in release 4.2, the newer of two versions
│           │   │   └── glossary.md                                          # Sample document: Glossary
│           │   ├── fr/                                                      # The documents written in French
│           │   │   ├── backups/                                             # Backups
│           │   │   │   ├── politique-de-sauvegarde.md                       # Sample document: Politique de sauvegarde
│           │   │   │   └── verification-des-sauvegardes.md                  # Sample document: Vérifier les sauvegardes
│           │   │   ├── databases/                                           # Databases
│           │   │   │   └── migrations-sans-interruption.md                  # Sample document: Migrations sans interruption de service
│           │   │   ├── http/                                                # HTTP APIs
│           │   │   │   ├── codes-d-erreur.md                                # Sample document: Codes d'erreur HTTP
│           │   │   │   ├── jetons.md                                        # Sample document: Jetons d'accès à l'API
│           │   │   │   └── limites-de-debit.md                              # Sample document: Limites de débit
│           │   │   ├── logging/                                             # Logging
│           │   │   │   └── journaux-structures.md                           # Sample document: Journaux structurés
│           │   │   ├── messaging/                                           # Messaging
│           │   │   │   ├── accuses-de-reception.md                          # Sample document: Accusés de réception
│           │   │   │   └── ordre-des-messages.md                            # Sample document: Ordre des messages et partitions
│           │   │   ├── operations/                                          # Operations
│           │   │   │   └── gestion-des-incidents.md                         # Sample document: Gestion des incidents
│           │   │   ├── scheduling/                                          # Scheduling
│           │   │   │   ├── relance-manuelle.md                              # Sample document: Relancer une tâche à la main
│           │   │   │   └── supervision-des-taches.md                        # Sample document: Surveiller les tâches planifiées
│           │   │   └── tls/                                                 # TLS certificates
│           │   │       └── certificats-clients.md                           # Sample document: Certificats clients et TLS mutuel
│           │   └── maestro-corpus.jsonl                                     # The source's manifest: one maestro-corpus/1 line per document, with its digest and size
│           ├── evals/                                                       # The collection's evaluation suites, as evals.suite names them: each <name>.jsonl is the suite <name>
│           │   └── synthetic.jsonl                                          # The suite synthetic: one maestro-suite/1 question per line, French and English, each with the sections that answer it
│           ├── .rumdl.toml                                                  # The synthetic collection is test input, not documentation: one of its documents repeats a heading under the same parent, as authors do
│           └── collection.json                                              # The maestro-collection/1 declaration of the public collection synthetic
├── .editorconfig                                                            # Editor settings that survive the editor
├── .gitattributes                                                           # How Git should treat each kind of file
├── .gitignore                                                               # Paths git never tracks
├── .lycheeignore                                                            # The example fixture's original.md keeps its author's relative link to assets/flow.svg, which resolves beside input.md, not beside the copy
├── .pre-commit-config.yaml                                                  # Spec Kit writes and refreshes these files in its own style; they stay as it writes them, so a refresh changes nothing by itself
├── AGENTS.md                                                                # Rules for coding agents: what to read, what never to weaken, how to verify
├── CONTEXT.md                                                               # The words this repository uses, and the ones it avoids
├── Cargo.lock                                                               # Exact dependency versions, committed so every build resolves the same
├── Cargo.toml                                                               # Workspace manifest: its members and the lints every member inherits
├── LICENSE                                                                  # The licence this repository is distributed under
├── README.md                                                                # The local runtime of Maestro: knowledge kernel, retrieval, orchestration and the command-line tools
├── justfile                                                                 # List every recipe and what it does; this is what just alone prints
├── maestro-quality.toml                                                     # The organization's quality rules as this repository shapes them: the inputs its CI caller passes, the seams that keep one caller
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
4. Run `just check`, and report the commands you actually ran.
5. Commits are signed, with a conventional title; the default branch takes only
   squash-merged pull requests.
