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
│   ├── maestro/                                                             # The maestro binary: the command line (CLI) over the knowledge library and the kernel
│   │   ├── src/                                                             # The crate's sources
│   │   │   ├── cli/                                                         # The commands, a module each, and what they share
│   │   │   │   ├── health/                                                  # maestro doctor and status: the checks of the kernel, the search service, the router and each role's card
│   │   │   │   │   ├── tests/                                               # Unit tests of the checks: the kernel's files, the services, the cards, what doctor must not touch
│   │   │   │   │   │   ├── findings.rs                                      # Foreign entries of the data directory listed and left untouched; grants that reach no known scope
│   │   │   │   │   │   ├── kernel.rs                                        # config.toml and bindings.toml refused with a fix; the database never created, damage and lost artifacts found
│   │   │   │   │   │   ├── mod.rs                                           # The health unit tests' door: declarations only
│   │   │   │   │   │   ├── services.rs                                      # Qdrant at the pinned version, the router's catalog, each role's card; next actions from what setup would do
│   │   │   │   │   │   └── support.rs                                       # What the health tests share: a scratch kernel directory, a one-answer HTTP stub, a closed address
│   │   │   │   │   ├── check.rs                                             # A check of the machine: its target, and what it saw, or its problem and next action
│   │   │   │   │   ├── doctor.rs                                            # maestro doctor: every check, each failure with its next action, what it must not touch; exit 1 on a failure
│   │   │   │   │   ├── findings.rs                                          # What doctor lists and never touches: entries the kernel does not own, as maestro v1's, and unreached grants
│   │   │   │   │   ├── kernel.rs                                            # The kernel's checks: configuration files, the database opened only if it exists and checked whole, the artifacts
│   │   │   │   │   ├── mod.rs                                               # The checks' door: declarations only
│   │   │   │   │   ├── services.rs                                          # Qdrant answering as the pinned version, the router listing its catalog, each role's model card
│   │   │   │   │   └── status.rs                                            # maestro status: the kernel, Qdrant and the router ready or not, and each readable collection; exits 0
│   │   │   │   ├── setup/                                                   # maestro setup: Qdrant 1.19.1 pinned by digest, a systemd user unit on 127.0.0.1; a preview, then --yes
│   │   │   │   │   ├── tests/                                               # Unit tests of setup: the pin and the platforms everywhere; the unit and the install with fake tools on Unix
│   │   │   │   │   │   ├── install.rs                                       # A preview and a second run change nothing, a digest mismatch writes nothing, each failure is named
│   │   │   │   │   │   ├── mod.rs                                           # The setup unit tests' door: declarations only
│   │   │   │   │   │   ├── platform.rs                                      # The pinned release is research R7's; Linux on x86-64 only, every other platform gets the manual steps
│   │   │   │   │   │   ├── support.rs                                       # What the install tests share: a scratch home, fake curl, tar and systemctl that log, a small release
│   │   │   │   │   │   └── unit.rs                                          # The service's layout and unit: loopback, telemetry off, data under the kernel's; paths escaped or refused
│   │   │   │   │   ├── command.rs                                           # maestro setup: the preview, the steps taken with --yes, the document printed; the readiness doctor asks
│   │   │   │   │   ├── mod.rs                                               # The setup's door: declarations only
│   │   │   │   │   ├── release.rs                                           # The pinned Qdrant 1.19.1 archive and binary digests, the ports, and the manual steps elsewhere
│   │   │   │   │   ├── service.rs                                           # The layout, the unit, the survey of what is missing, and the steps that install it, each checked first
│   │   │   │   │   └── tools.rs                                             # curl over HTTPS into memory, tar unpacking in memory, systemctl --user: the tools setup runs
│   │   │   │   ├── tests/                                                   # Unit tests: the lease of a foreground job, and how an import ends its job
│   │   │   │   │   ├── import_endings.rs                                    # An import ends its job succeeded with its report or failed saying why; each step journaled, or a lost lease stops it
│   │   │   │   │   ├── lease_heartbeats.rs                                  # A foreground job's lease, held only by Holder::run: renewed at each heartbeat and step, never after a takeover
│   │   │   │   │   ├── mod.rs                                               # The unit tests' door: declarations only
│   │   │   │   │   ├── supersessions.rs                                     # An import supersedes its resource's holder once no live lease holds it, and leaves a live one alone
│   │   │   │   │   └── support.rs                                           # What the unit tests share: a scratch kernel and a job leased in it, held or lost
│   │   │   │   ├── args.rs                                                  # The grammar, noun then verb, as clap derives it; the comments are the help
│   │   │   │   ├── collection.rs                                            # knowledge collection add, and the declaration a later command finds for a collection
│   │   │   │   ├── failure.rs                                               # Why a command stopped short: refused (exit 2) or failed (exit 1)
│   │   │   │   ├── import.rs                                                # knowledge import: a leased job in the foreground, its ID first; a rerun follows, takes over or supersedes
│   │   │   │   ├── kernel.rs                                                # The kernel every command opens: paths, database, config.toml applied, the local principal's scopes
│   │   │   │   ├── lease.rs                                                 # The lease of a job run in the foreground: Holder::run's heartbeat thread and each step renew it
│   │   │   │   ├── mod.rs                                                   # The commands' door: declarations only
│   │   │   │   ├── output.rs                                                # How a command prints: text, or one JSON document under --json; diagnostics on stderr
│   │   │   │   ├── run.rs                                                   # Parses the arguments, opens the kernel, runs the command, returns its exit code
│   │   │   │   ├── status.rs                                                # knowledge status: documents, revisions by status and disposition, generations
│   │   │   │   └── wait.rs                                                  # job wait: a job's stream followed to its end, the command exiting with its outcome; the follower
│   │   │   └── main.rs                                                      # The binary root: the commands, their output, exit codes and JSON schemas documented
│   │   ├── tests/                                                           # Integration tests
│   │   │   └── it/                                                          # The contract tests: the built binary run in a scratch home
│   │   │       ├── cli_contract.rs                                          # JSON on stdout, diagnostics on stderr, exit codes 0, 1 and 2, the job ID first
│   │   │       ├── collection_status.rs                                     # knowledge status of the synthetic collection: counts, dispositions and a generation
│   │   │       ├── doctor_checks.rs                                         # maestro doctor: each failure names its next action, the router its address; v1 files listed, untouched
│   │   │       ├── fakes.rs                                                 # Fake curl and systemctl for the binary's tests, found first on the PATH, logging each call
│   │   │       ├── import_jobs.rs                                           # knowledge import end to end, rerun, live holder refused, stale one superseded, leases taken over
│   │   │       ├── job_waits.rs                                             # job wait follows a job to its end and exits with its outcome; an unreadable job is unknown
│   │   │       ├── machine.rs                                               # How doctor and status tests run the binary: a router where nothing answers, the fakes on the PATH
│   │   │       ├── main.rs                                                  # The one integration-test crate of the binary
│   │   │       ├── setup_installs.rs                                        # maestro setup: the preview writes nothing, a wrong download is refused; elsewhere manual steps, exit 2
│   │   │       ├── status_summaries.rs                                      # maestro status: services ready or down, the readable collections, nothing created on a fresh machine
│   │   │       └── support.rs                                               # What the contract tests share: a scratch home, the synthetic collection, the binary under a deadline
│   │   └── Cargo.toml                                                       # Crate manifest: The command line of Maestro: collections, their imports, the jobs that run them, and the machine's setup and checks
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
│   │   │   │   │   ├── oversized.rs                                         # Oversized units: refused by name, with their block and its span, never by their text
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
│   │   │   ├── 0005_jobs.sql                                                # The jobs table: each job's key, attempt, resource, state, lease and outcome, and its triggers
│   │   │   ├── 0006_eval_reports.sql                                        # The evaluation reports: each run's collection, generation, suite and report artifact, and the triggers that keep it as recorded
│   │   │   └── 0007_chunk_sets.sql                                          # The guards of the chunk sets and their chunks: their states, moves and identity, and a complete set's chunks as counted
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
│   │   │   ├── chunk_set/                                                   # Chunk sets: a collection's chunks under one profile and one counter, building until complete or failed
│   │   │   │   ├── tests/                                                   # Tests of the chunk set records: their lifecycle and their chunks
│   │   │   │   │   ├── chunks.rs                                            # Chunks: a revision's chunks recorded at once into a building set, their prepared inputs pinned, read in scope
│   │   │   │   │   ├── guards.rs                                            # The guards of migration 0007_chunk_sets: each trigger refusing raw SQL, one test each
│   │   │   │   │   ├── lifecycle.rs                                         # A chunk set's lifecycle: begun building, found again by a rerun, then complete or failed for good
│   │   │   │   │   ├── mod.rs                                               # Tests of the chunk set records: their lifecycle and their chunks
│   │   │   │   │   └── support.rs                                           # What the chunk set tests share: a scratch database with revisions of two collections
│   │   │   │   ├── chunk.rs                                                 # Chunks: a revision's passages in a chunk set, recorded at once, each pinning its prepared input
│   │   │   │   ├── error.rs                                                 # Why the kernel refused to begin, move or fill a chunk set
│   │   │   │   ├── mod.rs                                                   # Chunk sets: a collection's chunks under one profile and one counter, building until complete or failed
│   │   │   │   ├── record.rs                                                # Chunk sets as the kernel records them: begun, completed with their manifest or failed, read in scope
│   │   │   │   └── state.rs                                                 # The states a chunk set moves through, and the moves it may make
│   │   │   ├── document/                                                    # The pipeline's document records (building block B5; docs/architecture/01
│   │   │   │   ├── tests/                                                   # Tests of the document records: the documents migration, collections
│   │   │   │   │   ├── counts.rs                                            # A collection's counts: documents, revisions by status and disposition, only in scope
│   │   │   │   │   ├── dispositions.rs                                      # Quality dispositions: one per revision, kept once given, read in scope, a hold journaled with it
│   │   │   │   │   ├── duplicates.rs                                        # Occurrences and near-duplicate groups: each recorded once, a batch in one write, read in scope
│   │   │   │   │   ├── errors.rs                                            # What the document records' refusals say, and the store's refusals they
│   │   │   │   │   ├── listing.rs                                           # The collections a set covers, listed in id order; a source's grant never lists its collection
│   │   │   │   │   ├── mod.rs                                               # Tests of the document records: the documents migration, collections
│   │   │   │   │   ├── parents.rs                                           # Collections, their sources and their documents: collections and sources
│   │   │   │   │   ├── revisions.rs                                         # Revisions: recorded once with their two artifacts pinned, immutable after
│   │   │   │   │   ├── schema.rs                                            # The documents migration: the ten pipeline tables of 01 §11, all strict
│   │   │   │   │   └── support.rs                                           # What the record tests share: a scratch database, and the collection
│   │   │   │   ├── collection.rs                                            # Collections, the sources they declare and the documents those sources
│   │   │   │   ├── counts.rs                                                # A collection's counts: documents, and revisions by status and by disposition, in one snapshot
│   │   │   │   ├── disposition.rs                                           # Quality dispositions: one per revision, kept once given, a hold journaled in the same write
│   │   │   │   ├── duplicate.rs                                             # The duplicates of revisions: every place a revision's content occurs, and the groups of near duplicates
│   │   │   │   ├── error.rs                                                 # Why the kernel refused to record a collection, a source, a document or a revision
│   │   │   │   ├── mod.rs                                                   # The pipeline's document records (building block B5; docs/architecture/01
│   │   │   │   └── revision.rs                                              # Revisions: one exact version of a document's bytes and metadata, recorded
│   │   │   ├── eval/                                                        # Evaluation reports (plan D13; FR-S1-009): each run of an evaluation suite
│   │   │   │   ├── tests/                                                   # Tests of the evaluation reports the kernel records: their artifact, their
│   │   │   │   │   ├── mod.rs                                               # Tests of the evaluation reports the kernel records: their artifact, their
│   │   │   │   │   ├── records.rs                                           # A report is stored as an artifact, indexed and pinned by its record, and
│   │   │   │   │   ├── support.rs                                           # What the report tests share: a scratch database holding the collections
│   │   │   │   │   └── table.rs                                             # The table of reports refuses, whoever writes, to change, replace or
│   │   │   │   ├── error.rs                                                 # Why the kernel refused to record or read an evaluation report
│   │   │   │   ├── mod.rs                                                   # Evaluation reports (plan D13; FR-S1-009): each run of an evaluation suite
│   │   │   │   └── report.rs                                                # Reports as the kernel records them: the artifact of each, the record that
│   │   │   ├── evidence/                                                    # Evidence (building block B7; docs/architecture/02 §6, plan D10): what a
│   │   │   │   ├── tests/                                                   # Tests of evidence: resolving a chunk from the authority, and bundles as
│   │   │   │   │   ├── bundle.rs                                            # Bundles: maestro-evidence/1 as JSON, its evidence apart from its trace
│   │   │   │   │   ├── mod.rs                                               # Tests of evidence: resolving a chunk from the authority, and bundles as
│   │   │   │   │   ├── resolve.rs                                           # Resolving a chunk: the exact bytes its span covers in its revision's
│   │   │   │   │   └── support.rs                                           # What the evidence tests share: a scratch database holding one revision of
│   │   │   │   ├── bundle.rs                                                # Bundles: maestro-evidence/1, the search response contract, checked whole
│   │   │   │   ├── error.rs                                                 # Why the kernel refused to resolve a chunk
│   │   │   │   ├── mod.rs                                                   # Evidence (building block B7; docs/architecture/02 §6, plan D10): what a
│   │   │   │   ├── passage.rs                                               # The passages a bundle cites: the source text of a span of one revision
│   │   │   │   └── resolve.rs                                               # Resolving a chunk: the exact source text its span covers, read from the
│   │   │   ├── gateway/                                                     # The model gateway (building block B10): every model, embedder, reranker
│   │   │   │   ├── tests/                                                   # Tests of the model gateway: model cards, the router client against a stub
│   │   │   │   │   ├── card.rs                                              # Tests of model cards: strict JSON artifacts whose digest is their
│   │   │   │   │   ├── catalog.rs                                           # The router's catalog: GET /v1/models in no room; refusals kept, a bad entry name an invalid answer
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
│   │   │   │   │   ├── listing.rs                                           # A collection's generations: all of them, in creation order, only in scope
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
│   │   │   │   │   ├── breaking.rs                                          # The compatibility check: what a fresh event schema changes of the committed one, beyond an added optional property
│   │   │   │   │   ├── child.rs                                             # Not a test of its own: what the child processes of the crash and
│   │   │   │   │   ├── compatibility.rs                                     # The compatibility check's rules on small schemas: what it refuses as a breaking change, what it lets through
│   │   │   │   │   ├── concurrency.rs                                       # Writers together: threads sharing the database and processes of their own
│   │   │   │   │   ├── crash.rs                                             # A crash: the event a process committed before it is read after the
│   │   │   │   │   ├── cursors.rs                                           # Cursors: where each consumer stands in each stream, moved forward only
│   │   │   │   │   ├── envelope.rs                                          # The envelope: an event read back from the journal as its CloudEvents 1.0 envelope
│   │   │   │   │   ├── events.rs                                            # Events: the ID and the sequence recording gives them, the event it
│   │   │   │   │   ├── mod.rs                                               # Tests of the journal: its events and their streams, its cursors, and what
│   │   │   │   │   ├── regeneration.rs                                      # The command that regenerates the committed event schemas and their index, tested on a scratch directory
│   │   │   │   │   ├── schemas.rs                                           # The committed event schemas: generated from their types, never narrowed, followed by the data, and the command that regenerates them
│   │   │   │   │   ├── subset.rs                                            # What the check and the validator read of a schema: the subset of JSON Schema schemars generates
│   │   │   │   │   ├── support.rs                                           # What the journal tests share: a scratch data directory, the events they
│   │   │   │   │   └── validation.rs                                        # A validator of data against the committed event schemas, over the subset schemars generates
│   │   │   │   ├── cursor.rs                                                # Cursors: how far each consumer has read each stream, moved forward only
│   │   │   │   ├── envelope.rs                                              # Events as they leave the kernel: the CloudEvents 1.0 envelope, and the machine its source names
│   │   │   │   ├── error.rs                                                 # Why the journal refused an operation
│   │   │   │   ├── event.rs                                                 # Events: recorded with a new ID and the next sequence of their stream
│   │   │   │   ├── knowledge.rs                                             # The public knowledge events: the data of each, and the catalogue naming the schema each follows
│   │   │   │   └── mod.rs                                                   # The journal: every change the kernel makes, recorded as an event in one
│   │   │   ├── scope/                                                       # Scopes and grants: who may see what (docs/architecture/04 §3, building
│   │   │   │   ├── tests/                                                   # Tests of scopes: their paths and names, what a grant covers, the grants
│   │   │   │   │   ├── config.rs                                            # config.toml: the local principal's grants, checked whole when read, and
│   │   │   │   │   ├── grants.rs                                            # Grants: a principal sees only what it was granted and what lies below it
│   │   │   │   │   ├── inventory.rs                                         # There is no read function without a ScopeSet: every public method of
│   │   │   │   │   ├── known.rs                                             # The scopes a set was granted, and the known scopes it covers: the workspace, collections and sources
│   │   │   │   │   ├── mod.rs                                               # Tests of scopes: their paths and names, what a grant covers, the grants
│   │   │   │   │   ├── paths.rs                                             # Scope paths: a workspace, then a collection, then a source, each named by
│   │   │   │   │   ├── readers.rs                                           # Readers of scoped data take the caller's ScopeSet and filter inside
│   │   │   │   │   ├── records.rs                                           # The readers of the pipeline's records take the caller's ScopeSet and
│   │   │   │   │   └── support.rs                                           # What the scope tests share: a scratch directory for the kernel's data and
│   │   │   │   ├── config.rs                                                # config.toml, the kernel's configuration file in its configuration
│   │   │   │   ├── grant.rs                                                 # Grants: the rights of principals on scopes, each given or taken back in a
│   │   │   │   ├── known.rs                                                 # The scopes the kernel knows, as a set sees them: its workspace and each recorded collection and source
│   │   │   │   ├── mod.rs                                                   # Scopes and grants: who may see what (docs/architecture/04 §3, building
│   │   │   │   ├── path.rs                                                  # Scope paths: a workspace, then optionally a collection, then optionally a
│   │   │   │   ├── right.rs                                                 # The rights a grant gives on a scope
│   │   │   │   └── set.rs                                                   # The scopes a principal may read, found for one request, and the condition
│   │   │   ├── store/                                                       # The kernel's database: one SQLite file beside the artifact store, holding
│   │   │   │   ├── tests/                                                   # Tests of the kernel database: its migrations, its connections, the
│   │   │   │   │   ├── artifacts.rs                                         # Artifacts: stored, recorded with their size, media type and pins, and read
│   │   │   │   │   ├── checks.rs                                            # SQLite's quick check, damage reported as found; each recorded artifact present and intact, or named
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
│       │   ├── eval/                                                        # The evaluation runner (plan D13; FR-S1-009, SC-S1-008): every retrieval
│       │   │   ├── tests/                                                   # Tests of the evaluation runner: how it ranks and judges each question
│       │   │   │   ├── compare.rs                                           # compare pairs two runs by question and gives, for each metric, the
│       │   │   │   ├── degraded.rs                                          # A degraded search, one where a route or the reranker could not run, still
│       │   │   │   ├── documents.rs                                         # A question may expect a document without sections whole: any passage of
│       │   │   │   ├── failures.rs                                          # Each failure of an answerable question gets its class at each cut-off it
│       │   │   │   ├── intervals.rs                                         # The intervals: 95 % percentile intervals of 2,000 bootstrap resamples
│       │   │   │   ├── metrics.rs                                           # Each metric of a run against values computed by hand on a small suite
│       │   │   │   ├── mod.rs                                               # Tests of the evaluation runner: how it ranks and judges each question
│       │   │   │   ├── ranking.rs                                           # A bundle lists its passages in reading order, so the runner ranks them
│       │   │   │   ├── report.rs                                            # A report is a strict JSON artifact, maestro-eval-report/1: it writes and
│       │   │   │   ├── run.rs                                               # run resolves each expected section of a suite in the canonical document
│       │   │   │   └── support.rs                                           # What the evaluation tests share: questions, bundles built from the hits a
│       │   │   ├── bootstrap.rs                                             # The bootstrap: resamples of a run's questions, drawn within the
│       │   │   ├── compare.rs                                               # Comparing two runs of one suite, question by question
│       │   │   ├── error.rs                                                 # Why a run or a comparison was refused
│       │   │   ├── judge.rs                                                 # Judging one question: ranking its bundle's passages, finding the sections
│       │   │   ├── metric.rs                                                # The metrics of a run: each a value over a sample of its questions, drawn
│       │   │   ├── mod.rs                                                   # The evaluation runner (plan D13; FR-S1-009, SC-S1-008): every retrieval
│       │   │   ├── report.rs                                                # Reports: maestro-eval-report/1, what a run measured, question by
│       │   │   └── run.rs                                                   # A run: every question of a suite, resolved in the generation it
│       │   ├── import/                                                      # Importing a collection's corpus through its maestro-corpus/1 manifests
│       │   │   ├── tests/                                                   # Tests of the import that reach inside it: its manifest lines, and its streaming, proven by an in-memory corpus
│       │   │   │   ├── lines.rs                                             # A manifest read one numbered line at a time, until it ends
│       │   │   │   ├── mod.rs                                               # Tests of the import that reach inside it: its manifest lines, and its streaming, proven by an in-memory corpus
│       │   │   │   └── streaming.rs                                         # The import holds one manifest line and one document at a time
│       │   │   ├── collection.rs                                            # Importing a collection: its declaration recorded, each source's manifest imported, its completion journaled
│       │   │   ├── corpus.rs                                                # Where an import reads a source's corpus: its manifest, and each document relative to it
│       │   │   ├── entry.rs                                                 # Importing one entry: its document checked, canonicalized, stored and recorded, or held
│       │   │   ├── error.rs                                                 # Why an import stopped, before any work or part way
│       │   │   ├── mod.rs                                                   # Importing a collection's corpus through its maestro-corpus/1 manifests
│       │   │   ├── report.rs                                                # What an import reports: its counts, and why it refused each entry it refused
│       │   │   └── source.rs                                                # Importing one source's manifest: shared source_refs found first, then each line in turn
│       │   ├── lexical/                                                     # The lexical analyzer of the BM25 route, profile bm25-en-fr/1: it turns a
│       │   │   ├── analyzer.rs                                              # The profile's name and the terms of a text
│       │   │   ├── fold.rs                                                  # Folding: a text without its accents, before any other rule reads it
│       │   │   ├── mod.rs                                                   # The lexical analyzer of the BM25 route, profile bm25-en-fr/1: it turns a
│       │   │   ├── split.rs                                                 # Splitting: the identifiers and words of a folded text, and the parts of
│       │   │   ├── stem.rs                                                  # Stemming: light suffix rules on folded, lowercased words, French and
│       │   │   ├── stopwords.rs                                             # The stopwords of bm25-en-fr/1: an English list and a French one, each of
│       │   │   ├── tests.rs                                                 # What the lexical module's lookups rely on
│       │   │   └── vector.rs                                                # Sparse vectors: a passage's terms weighed with BM25's term-frequency part
│       │   ├── prepare/                                                     # Preparing revisions for search: duplicates grouped, then chunks counted as the selected embedder counts them, through the model router
│       │   │   ├── tests/                                                   # Tests of the router tokenizer, and of the preparation of a collection over it
│       │   │   │   ├── chunk_sets.rs                                        # What a chunk set records: its profile, its counter, the same chunk IDs for the same input, each prepared input pinned
│       │   │   │   ├── counting.rs                                          # Counting and verifying through a qualified tokenizer: the port's IDs in
│       │   │   │   ├── duplicates.rs                                        # Duplicates: exact ones prepared once with every occurrence kept, near ones grouped with their Jaccard
│       │   │   │   ├── eligibility.rs                                       # Which revisions a preparation reads: the accepted ones, for a caller who reads the whole collection
│       │   │   │   ├── interruptions.rs                                     # Interrupted work never reads as complete: a stopped run resumes, a changed counter fails the set
│       │   │   │   ├── latest.rs                                            # A document is prepared by its latest revision alone; one whose latest is held or failed is left out and reported
│       │   │   │   ├── mod.rs                                               # Tests of the router tokenizer, and of the preparation of a collection over it
│       │   │   │   ├── near.rs                                              # Near duplicates: signatures and bands propose, an exact Jaccard of 0.85 or more confirms, pairs link groups
│       │   │   │   ├── oversized.rs                                         # A unit that cannot fit 700 tokens with its context refuses its document by name; the rest are prepared
│       │   │   │   ├── parity.rs                                            # The parity fixtures: how their file writes them, and what the built-in
│       │   │   │   ├── port.rs                                              # A model port that answers each parity fixture with the native counter's
│       │   │   │   ├── qualification.rs                                     # Qualification: a router tokenizer exists only once the port gives every
│       │   │   │   ├── refusals.rs                                          # The refusals: what each says, and the cause each keeps
│       │   │   │   ├── router_client.rs                                     # The router client through a router tokenizer, against a stub router: from
│       │   │   │   ├── scratch.rs                                           # What the preparation's tests share: a scratch corpus and kernel, a collection imported and decided
│       │   │   │   ├── stops.rs                                             # Why a preparation stops: what each stop says, and the cause each keeps
│       │   │   │   ├── stub.rs                                              # A stub of the model router for the router client's tests: a loopback HTTP
│       │   │   │   ├── support.rs                                           # What the router tokenizer's tests share: model cards, recorded in a
│       │   │   │   └── synthetic.rs                                         # The public synthetic collection prepared end to end: one exact group, two near-duplicate groups
│       │   │   ├── bridge.rs                                                # A model port's asynchronous tokenize, called synchronously: the port's
│       │   │   ├── chunking.rs                                              # Chunking prepared revisions: each prepared input stored, a revision's chunks recorded at once
│       │   │   ├── collection.rs                                            # Preparing a collection: its eligible revisions deduplicated, then chunked into the chunk set they name
│       │   │   ├── counter.rs                                               # The router tokenizer as the chunker counts through it, keeping the typed refusal the chunker holds as text
│       │   │   ├── error.rs                                                 # Why a router tokenizer refuses to qualify, or to count
│       │   │   ├── exact.rs                                                 # Exact duplicates: the same original bytes and canonical content, prepared once as the smallest revision
│       │   │   ├── failure.rs                                               # Why a preparation stopped, and what a rerun does then
│       │   │   ├── left_out.rs                                              # The documents a preparation leaves out, since the quality gate does not let their latest revision through, each with why
│       │   │   ├── manifest.rs                                              # A chunk set's identity, and its manifest maestro-chunk-set/1, which a complete set pins
│       │   │   ├── mod.rs                                                   # Preparing revisions for search: deduplicated, then chunked in the tokens of the selected embedder
│       │   │   ├── native-parity.json                                       # The native counter's ordered IDs for the 41 parity fixtures a router tokenizer must match to qualify
│       │   │   ├── near.rs                                                  # Near duplicates: word 5-gram shingles, MinHash bands, exact Jaccard confirmation, groups that delete nothing
│       │   │   ├── parity.rs                                                # The native profile's parity fixtures, native-parity.json: complete
│       │   │   ├── report.rs                                                # What a preparation reports, as JSON: its chunk set, its counts and each refusal
│       │   │   └── router_tokenizer.rs                                      # The router tokenizer: maestro-canonicalization's TokenCounter over the
│       │   ├── quality/                                                     # The quality gate: one disposition per revision before indexing (01 §4, FR-S1-002a)
│       │   │   ├── checks/                                                  # The automatic checks, each a rule ID with a documented threshold
│       │   │   │   ├── body.rs                                              # The body as the checks count it: near-empty and navigation-heavy
│       │   │   │   ├── flag.rs                                              # What a check says of a revision it flags: rule, outcome and reason
│       │   │   │   ├── mod.rs                                               # The automatic checks, each a rule ID with a documented threshold
│       │   │   │   ├── page.rs                                              # A page that is not the document: an application error, a sign-in prompt
│       │   │   │   ├── record.rs                                            # Canonicalization failures, missing provenance and assets, incomplete tables
│       │   │   │   ├── rules.rs                                             # Every automatic check, run in the order of the module table
│       │   │   │   ├── secret.rs                                            # text.suspected-secret: PEM private keys and AWS, GitHub, GitLab, Slack tokens
│       │   │   │   └── text.rs                                              # Replacement characters and extraction markers left in the text
│       │   │   ├── tests/                                                   # Each check with a document it flags and one it must not; the precedence
│       │   │   │   ├── body.rs                                              # The body checks: near-empty, navigation, application error, sign-in
│       │   │   │   ├── mod.rs                                               # Each check with a document it flags and one it must not; the precedence
│       │   │   │   ├── precedence.rs                                        # A ledger rule outranks the checks but never accepts a failed document
│       │   │   │   ├── record.rs                                            # The record checks: canonicalization, metadata, assets, tables, characters
│       │   │   │   ├── secrets.rs                                           # Each secret format with examples it flags and must not, placeholders among them
│       │   │   │   └── support.rs                                           # Canonical documents made from Markdown, revisions and ledgers for the tests
│       │   │   ├── decide.rs                                                # One decision: the first ledger rule that matches, else the checks
│       │   │   ├── error.rs                                                 # Why the gate stopped; what it decided before stays decided
│       │   │   ├── gate.rs                                                  # The gate over a collection, and the eligible revisions it lets through
│       │   │   ├── ledger.rs                                                # The quality ledger, maestro-quality-ledger/1: one strict JSON rule a line
│       │   │   ├── mod.rs                                                   # The gate, its precedence, and the list of automatic checks
│       │   │   ├── outcome.rs                                               # The five outcomes by their 01 §4 names, and which hold a revision back
│       │   │   └── report.rs                                                # What the gate reports: every revision counted once, the held ones listed
│       │   ├── collection.rs                                                # A collection's declaration: maestro-collection/1, the strict JSON that
│       │   ├── corpus.rs                                                    # A corpus manifest: maestro-corpus/1, one JSON line per document, through
│       │   ├── lib.rs                                                       # The knowledge pipeline of Maestro (docs/architecture/01): collections, their
│       │   ├── relative_path.rs                                             # Paths that a declaration or a manifest gives relative to a directory, which
│       │   ├── shape.rs                                                     # The JSON shapes the contracts name, and no other: an object where a
│       │   └── suite.rs                                                     # An evaluation suite: maestro-suite/1, one JSON line per question, which
│       ├── tests/                                                           # Integration tests
│       │   └── it/                                                          # It
│       │       ├── import_contract/                                         # The import of corpus manifests (T019): refusals, holds, idempotency, identity, report, synthetic collection
│       │       │   ├── declared_collections.rs                              # Declaring a collection records it and its sources, and nothing for a caller who cannot read them
│       │       │   ├── document_identity.rs                                 # A document's identity: its collection and source_ref hashed, its path relative to the manifest
│       │       │   ├── held_pairs.rs                                        # Lines sharing a source_ref with different digests: both revisions recorded, both held
│       │       │   ├── import_rate.rs                                       # The import rate on 2,000 generated documents, measured on demand
│       │       │   ├── import_report.rs                                     # What an import reports, as JSON and as import.completed, and what stops it before any work
│       │       │   ├── mod.rs                                               # The import of corpus manifests (T019): refusals, holds, idempotency, identity, report, synthetic collection
│       │       │   ├── observed_imports.rs                                  # An observed import: the report every hundred lines and after each last line; a break stops it
│       │       │   ├── refused_entries.rs                                   # Refusals, each with its line and reason, and the import goes on
│       │       │   ├── repeated_imports.rs                                  # An import is idempotent: a second one writes nothing, a change of metadata gives a new revision
│       │       │   ├── support.rs                                           # What the import's tests share: a scratch corpus and database, declarations, and the kernel read whole
│       │       │   └── synthetic_corpus.rs                                  # The public synthetic collection (T014) imported end to end
│       │       ├── local_collection/                                        # The collection this machine names, imported for real and gated on demand
│       │       │   ├── dispositions.rs                                      # The disposition report of a local run: identities, rules, reasons, counts
│       │       │   ├── mod.rs                                               # The collection this machine names, imported for real and gated on demand
│       │       │   └── real_import.rs                                       # The ignored run: a collection imported into the kernel data directory, then gated
│       │       ├── quality_gate/                                            # The quality gate (T020) through the import, the ledger and the kernel
│       │       │   ├── gate_report.rs                                       # The report as JSON, and the stops: unknown collection, broken artifact
│       │       │   ├── kept_dispositions.rs                                 # A disposition is kept: a rerun decides nothing, the import holds stay
│       │       │   ├── latest_revision.rs                                   # A document is eligible by its latest revision alone, never an older one
│       │       │   ├── mod.rs                                               # The quality gate (T020) through the import, the ledger and the kernel
│       │       │   ├── recorded_dispositions.rs                             # Dispositions with rule IDs and reasons, revision.held, eligibility
│       │       │   ├── support.rs                                           # A scratch corpus imported as garden, and what the kernel then records
│       │       │   └── synthetic_corpus.rs                                  # The public synthetic collection (T014) passes the gate end to end
│       │       ├── suite_check/                                             # Checking suites against their corpus: every name a suite of a directory
│       │       │   ├── check.rs                                             # The check itself: each suite of a directory read under its contract, and
│       │       │   ├── local_suites.rs                                      # The check on the suites and corpus this machine names, by hand
│       │       │   ├── mod.rs                                               # Checking suites against their corpus: every name a suite of a directory
│       │       │   ├── reported_problems.rs                                 # What the check reports as problems: a name that gives no one section nor
│       │       │   ├── scratch.rs                                           # What the check's tests share: a scratch directory holding a corpus, its
│       │       │   ├── synthetic_suite.rs                                   # The check on the public synthetic collection: every section its suite
│       │       │   └── unanswerable_leads.rs                                # Leads for unanswerable questions: each document of the manifest that holds
│       │       ├── collection_contract.rs                                   # maestro-collection/1: a strict declaration parses into typed values; an
│       │       ├── corpus_contract.rs                                       # maestro-corpus/1: one line per document parses into typed values; an
│       │       ├── eval_synthetic.rs                                        # The evaluation runner over the public synthetic suite (T014), end to end
│       │       ├── lexical_accents.rs                                       # Properties of bm25-en-fr/1 over generated texts: a text and the same
│       │       ├── lexical_fold.rs                                          # Folding in bm25-en-fr/1: every letter of Latin-1 Supplement and Latin
│       │       ├── lexical_golden.rs                                        # The golden of bm25-en-fr/1: the terms and vectors of sample passages and
│       │       ├── lexical_rules.rs                                         # The rules of bm25-en-fr/1 as the lexical module states them, each with
│       │       ├── lexical_sample.rs                                        # Research R7's public sample on bm25-en-fr/1: 22 passages, 11 in English
│       │       ├── lexical_vectors.rs                                       # The sparse vectors of bm25-en-fr/1: a passage's term weighs BM25's
│       │       ├── live_router.rs                                           # What the live tests share: the router their variables name, and its embedder's model card
│       │       ├── main.rs                                                  # The crate's integration tests, built as one test crate: each module proves
│       │       ├── prepare_live.rs                                          # knowledge prepare on this machine's kernel as a leased job, live: the chunk count, wall time and router calls
│       │       ├── quality_ledger.rs                                        # maestro-quality-ledger/1: strict rules a line; a missing ledger is empty
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
│   │   ├── 0020-rust-libraries-with-named-dependency-exceptions.md          # Rust libraries join the stack; the duplicates they force are named exceptions
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
├── schemas/                                                                 # The JSON Schemas this repository publishes, each generated from its Rust type
│   └── events/                                                              # The JSON Schema of each public event's data (07 §3.2), generated from its Rust type in journal/knowledge.rs
│       ├── knowledge.generation.published/                                  # The schemas of maestro.knowledge.generation.published, one file per major version
│       │   └── 1.json                                                       # A generation of a collection was published: searches read it from now on
│       ├── knowledge.generation.retired/                                    # The schemas of maestro.knowledge.generation.retired, one file per major version
│       │   └── 1.json                                                       # A published generation was retired: no search reads it, and it is kept for rollback
│       ├── knowledge.import.completed/                                      # The schemas of maestro.knowledge.import.completed, one file per major version
│       │   └── 1.json                                                       # An import of a collection's corpus manifest completed: what it did with its entries
│       ├── knowledge.revision.held/                                         # The schemas of maestro.knowledge.revision.held, one file per major version
│       │   └── 1.json                                                       # The quality gate held a revision back: it is not indexed
│       ├── README.md                                                        # The JSON Schema of each public event's data (07 §3.2), generated from its Rust type in journal/knowledge.rs
│       └── index.json                                                       # The committed event schemas, by name: the regeneration command refuses one whose file is gone
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
