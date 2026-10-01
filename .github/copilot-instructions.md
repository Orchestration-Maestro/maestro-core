# Copilot instructions for maestro-core

## Start here

The local runtime of Maestro: a knowledge kernel, retrieval and a command-line
application. The workspace contains `maestro-canonicalization`,
`maestro-conventions`, `maestro-kernel`, `maestro-knowledge`,
[`maestro-acquisition`](../crates/maestro-acquisition/README.md) and `maestro`.
[`maestro-canonicalization`](../crates/maestro-canonicalization/README.md) turns
Markdown into provenance-bearing canonical documents, groups duplicates and cuts
them into token-budgeted chunks; later capabilities arrive slice by slice
([roadmap](../docs/architecture/06-roadmap.md)).

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
.                                                                                        # Repository root
├── .cargo/                                                                              # Cargo settings for this workspace
│   └── mutants.toml                                                                     # Mutants no test can kill, each with its reason: none changes behaviour a test can observe
├── .github/                                                                             # GitHub metadata, templates and workflows
│   ├── workflows/                                                                       # GitHub Actions workflows
│   │   ├── dependabot-auto-merge.yml                                                    # Dependabot auto-merge
│   │   ├── event-schemas.yml                                                            # Released event schema compatibility
│   │   ├── integration.yml                                                              # Qdrant integration: the projection's tests against a Qdrant 1.19 image pinned by digest
│   │   └── scorecard.yml                                                                # OpenSSF Scorecard
│   ├── CODEOWNERS                                                                       # Who reviews each path
│   ├── copilot-instructions.md                                                          # This guide, written by rust-gate guide at every commit
│   ├── dependabot.yml                                                                   # The organization merges only conventional titles: "ci(deps): bump ..."
│   └── zizmor.yml                                                                       # The workflow security audit just check and the commit hook run: zizmor, in its pedantic persona, offline
├── crates/                                                                              # The workspace's crates
│   ├── maestro/                                                                         # The maestro binary: the command line (CLI) over the knowledge library and the kernel
│   │   ├── src/                                                                         # The crate's sources
│   │   │   ├── acquisition/                                                             # Native acquisition's local authority boundary
│   │   │   │   ├── authority.rs                                                         # Owner authority commands; no pipeline writer or same-user store fallback
│   │   │   │   ├── authority_host.rs                                                    # Linux host qualification, kept outside the serving path
│   │   │   │   ├── authority_probe.rs                                                   # One-shot qualification evidence authenticated by Linux peer credentials
│   │   │   │   ├── authority_service.rs                                                 # Bounded Linux Unix-socket IPC authenticated with kernel peer credentials
│   │   │   │   ├── authority_store.rs                                                   # Authority-only SQLite writer; grant changes and audit commit atomically
│   │   │   │   └── mod.rs                                                               # Native acquisition's local authority boundary
│   │   │   ├── cli/                                                                     # The commands, a module each, and what they share
│   │   │   │   ├── backup/                                                              # Back up and restore the kernel
│   │   │   │   │   ├── command.rs                                                       # The backup and restore command handlers and backup writer
│   │   │   │   │   ├── filesystem.rs                                                    # Rust source: filesystem
│   │   │   │   │   ├── manifest.rs                                                      # Rust source: manifest
│   │   │   │   │   ├── mod.rs                                                           # Back up and restore the kernel
│   │   │   │   │   ├── names.rs                                                         # Rust source: names
│   │   │   │   │   ├── restore.rs                                                       # Rust source: restore
│   │   │   │   │   ├── test_support.rs                                                  # Rust source: test support
│   │   │   │   │   └── tests.rs                                                         # Rust source: tests
│   │   │   │   ├── config/                                                              # maestro config: the settings, read, explained, changed and their
│   │   │   │   │   ├── change.rs                                                        # config set and config unset: one setting written in one preferences
│   │   │   │   │   ├── history.rs                                                       # config history: the journaled changes of the local principal's
│   │   │   │   │   ├── mod.rs                                                           # maestro config: the settings, read, explained, changed and their
│   │   │   │   │   ├── show.rs                                                          # config get, config list and config explain: a session's effective
│   │   │   │   │   └── tests.rs                                                         # config set and config unset: the file they choose, the edit, and the
│   │   │   │   ├── eval/                                                                # maestro eval ladder: the M1 ladder, every question of a suite searched
│   │   │   │   │   ├── tests/                                                           # Tests of the ladder command: its manifest, its run over a fake engine, its
│   │   │   │   │   │   ├── ask_settings.rs                                              # A rung's ask: true asks as ask does by default, false does not
│   │   │   │   │   │   ├── comparison.rs                                                # The comparison across rungs: each floor per rung and its change, the
│   │   │   │   │   │   ├── engine.rs                                                    # The engine's pieces that need no search service: a rung's reranker taken
│   │   │   │   │   │   ├── intent.rs                                                    # Intent expansion on the ladder: a HyDE rung's card reaches search and
│   │   │   │   │   │   ├── kernel_engine.rs                                             # The engine on a real kernel, with the router and the search service
│   │   │   │   │   │   ├── manifest.rs                                                  # The manifest: what it holds, where its paths lead, and each refusal
│   │   │   │   │   │   ├── manifest_defaults.rs                                         # Rust source: manifest defaults
│   │   │   │   │   │   ├── mod.rs                                                       # Tests of the ladder command: its manifest, its run over a fake engine, its
│   │   │   │   │   │   ├── noise_guard.rs                                               # The identifier noise guard on the ladder: a guarded search's row counts
│   │   │   │   │   │   ├── reports.rs                                                   # What a ladder writes: private rows with IDs, ranks, citations, refusal
│   │   │   │   │   │   ├── rung_answerer.rs                                             # A rung's answerer and prompt: ask.card names a registered answerer card
│   │   │   │   │   │   ├── runner.rs                                                    # The ladder's run: warm-ups unscored, the rung's configuration in every
│   │   │   │   │   │   ├── stages.rs                                                    # A search fails when a stage its rung enables did not run: out of time for
│   │   │   │   │   │   └── support.rs                                                   # A synthetic suite, rungs, and a fake engine that records what it is asked
│   │   │   │   │   ├── ask_settings.rs                                                  # Ask-only ladder settings and validation
│   │   │   │   │   ├── candidates.rs                                                    # The registered cards a ladder rung may name by digest: found in the
│   │   │   │   │   ├── command.rs                                                       # Running the ladder a manifest describes, on this machine
│   │   │   │   │   ├── comparison.rs                                                    # The comparison across a ladder's rungs, maestro-eval-ladder-comparison/1
│   │   │   │   │   ├── documents.rs                                                     # The documents a ladder row scores: a search's ranked documents, the first
│   │   │   │   │   ├── engine.rs                                                        # The ladder's engine on this machine: the kernel opened for the local
│   │   │   │   │   ├── engine_outcome.rs                                                # Converts a checked answer to the ladder's scoring outcome
│   │   │   │   │   ├── manifest.rs                                                      # The ladder's manifest, maestro-ladder-manifest/1: the suite, the
│   │   │   │   │   ├── mod.rs                                                           # maestro eval ladder: the M1 ladder, every question of a suite searched
│   │   │   │   │   ├── rank_settings.rs                                                 # Manifest adapters for optional candidate-context and section-prior policies
│   │   │   │   │   ├── reports.rs                                                       # What a ladder writes: for each rung, its private rows, one JSON line per
│   │   │   │   │   ├── rung_prompt.rs                                                   # A rung's answer prompt: a prompt version, or a private prompt file of
│   │   │   │   │   ├── runner.rs                                                        # The ladder's run: every rung's cards checked first, then each rung in
│   │   │   │   │   └── stages.rs                                                        # Whether a search ran every stage its rung enables
│   │   │   │   ├── health/                                                              # maestro doctor and status: the checks of the kernel, the search service, the router and each role's card
│   │   │   │   │   ├── tests/                                                           # Unit tests of the checks: the kernel's files, the services, the cards, what doctor must not touch
│   │   │   │   │   │   ├── doctor.rs                                                    # Doctor's verdict: a check that cannot run yet, as each role's model card
│   │   │   │   │   │   ├── findings.rs                                                  # Foreign entries of the data directory listed and left untouched; grants that reach no known scope
│   │   │   │   │   │   ├── kernel.rs                                                    # config.toml and bindings.toml refused with a fix; the database never created, damage and lost artifacts found
│   │   │   │   │   │   ├── mod.rs                                                       # The health unit tests' door: declarations only
│   │   │   │   │   │   ├── services.rs                                                  # Qdrant at the pinned version, the router's catalog, each role's card; next actions from what setup would do
│   │   │   │   │   │   ├── settings.rs                                                  # The settings check: the preferences files a session reads, or the one
│   │   │   │   │   │   └── support.rs                                                   # What the health tests share: a scratch kernel directory, a one-answer HTTP stub, a closed address
│   │   │   │   │   ├── check.rs                                                         # A check of the machine: its target, and what it saw, or its problem and next action
│   │   │   │   │   ├── doctor.rs                                                        # maestro doctor: every check, each failure with its next action, what it must not touch; exit 1 on a failure
│   │   │   │   │   ├── findings.rs                                                      # What doctor lists and never touches: entries the kernel does not own, as maestro v1's, and unreached grants
│   │   │   │   │   ├── kernel.rs                                                        # The kernel's checks: configuration files, the database opened only if it exists and checked whole, the artifacts
│   │   │   │   │   ├── mod.rs                                                           # The checks' door: declarations only
│   │   │   │   │   ├── services.rs                                                      # Qdrant answering as the pinned version, the router listing its catalog, each role's model card
│   │   │   │   │   ├── settings.rs                                                      # The settings check: which preferences files a session reads, the user's
│   │   │   │   │   └── status.rs                                                        # maestro status: the kernel, Qdrant and the router ready or not, and each readable collection; exits 0
│   │   │   │   ├── publish/                                                             # Explicit replacement of a lost projection and frozen resume identity
│   │   │   │   │   ├── mod.rs                                                           # Explicit replacement of a lost projection and frozen resume identity
│   │   │   │   │   ├── recovery.rs                                                      # Frozen identity and restart selection for explicit projection recovery
│   │   │   │   │   └── run.rs                                                           # knowledge publish: a verified Qdrant projection as a leased job
│   │   │   │   ├── publish_report/                                                      # Historical publication and current projection readiness reporting
│   │   │   │   │   ├── mod.rs                                                           # Historical publication and current projection readiness reporting
│   │   │   │   │   ├── render.rs                                                        # Reconciled output for the historical publication and current projection
│   │   │   │   │   └── tests.rs                                                         # Pure report formatting and database-to-Qdrant reconciliation checks
│   │   │   │   ├── search/                                                              # CLI presentation for scoped search bundles
│   │   │   │   │   ├── execution.rs                                                     # CLI setup, scoped execution and exit-code mapping for search
│   │   │   │   │   ├── mod.rs                                                           # CLI presentation for scoped search bundles
│   │   │   │   │   ├── presentation.rs                                                  # The CLI's bounded evidence document and text view
│   │   │   │   │   └── tests.rs                                                         # Rust source: tests
│   │   │   │   ├── setup/                                                               # maestro setup: Qdrant 1.19.1 pinned by digest, a systemd user unit on 127.0.0.1; a preview, then --yes
│   │   │   │   │   ├── tests/                                                           # Unit tests of setup: the pin and the platforms everywhere; the unit and the install with fake tools on Unix
│   │   │   │   │   │   ├── install.rs                                                   # A preview and a second run change nothing, a digest mismatch writes nothing, each failure is named
│   │   │   │   │   │   ├── manager.rs                                                   # The systemd user manager setup needs: without one, setup is refused before any step
│   │   │   │   │   │   ├── mod.rs                                                       # The setup unit tests' door: declarations only
│   │   │   │   │   │   ├── platform.rs                                                  # The pinned release is research R7's; Linux on x86-64 only, every other platform gets the manual steps
│   │   │   │   │   │   ├── support.rs                                                   # What the install tests share: a scratch home, fake curl, tar and systemctl that log, a small release
│   │   │   │   │   │   └── unit.rs                                                      # The service's layout and unit: loopback, telemetry off, data under the kernel's; paths escaped or refused
│   │   │   │   │   ├── command.rs                                                       # maestro setup: the preview, the steps taken with --yes, the document printed; the readiness doctor asks
│   │   │   │   │   ├── mod.rs                                                           # The setup's door: declarations only
│   │   │   │   │   ├── release.rs                                                       # The pinned Qdrant 1.19.1 archive and binary digests, the ports, and the manual steps elsewhere
│   │   │   │   │   ├── service.rs                                                       # The layout, the unit, the survey of what is missing, and the steps that install it, each checked first
│   │   │   │   │   └── tools.rs                                                         # curl over HTTPS into memory, tar unpacking in memory, systemctl --user: the tools setup runs
│   │   │   │   ├── tests/                                                               # Unit tests: the lease of a foreground job, and how an import ends its job
│   │   │   │   │   ├── publish_again/                                                   # Publish again
│   │   │   │   │   │   ├── selection/                                                   # Selection
│   │   │   │   │   │   │   └── jobs.rs                                                  # End-to-end selection and retry of journaled publication attempts
│   │   │   │   │   │   ├── cases.rs                                                     # Selection of active and failed explicit replacement jobs
│   │   │   │   │   │   └── mod.rs                                                       # Rust source: mod
│   │   │   │   │   ├── import_endings.rs                                                # An import ends its job succeeded with its report or failed saying why; each step journaled, or a lost lease stops it
│   │   │   │   │   ├── lease_heartbeats.rs                                              # A foreground job's lease, held only by Holder::run: renewed at each heartbeat and step, never after a takeover
│   │   │   │   │   ├── mod.rs                                                           # The unit tests' door: declarations only
│   │   │   │   │   ├── model_failures.rs                                                # Registration constraint failures are refused; other database errors fail.
│   │   │   │   │   ├── publication_resume.rs                                            # A new publication attempt resumes from the last step of its predecessor
│   │   │   │   │   ├── supersessions.rs                                                 # An import supersedes its resource's holder once no live lease holds it, and leaves a live one alone
│   │   │   │   │   ├── support.rs                                                       # What the unit tests share: a scratch kernel and a job leased in it, held or lost
│   │   │   │   │   └── verify_outcome.rs                                                # Verification findings make the verify job fail without dropping its report
│   │   │   │   ├── args.rs                                                              # The grammar, noun then verb, as clap derives it; the comments are the help
│   │   │   │   ├── ask.rs                                                               # CLI adapter for evidence-grounded knowledge answers
│   │   │   │   ├── collection.rs                                                        # knowledge collection add, and the declaration a later command finds for a collection
│   │   │   │   ├── foreground.rs                                                        # A job run in the foreground: submitted or found by its key, taken or followed, a stale holder superseded
│   │   │   │   ├── import.rs                                                            # knowledge import: a leased job in the foreground, its ID first; a rerun follows, takes over or supersedes
│   │   │   │   ├── lease.rs                                                             # The lease of a job run in the foreground: Holder::run's heartbeat thread and each step renew it
│   │   │   │   ├── mod.rs                                                               # The commands' door: declarations only
│   │   │   │   ├── model.rs                                                             # Registering and listing scoped model cards
│   │   │   │   ├── output.rs                                                            # How a command prints: text, or one JSON document under --json; diagnostics on stderr
│   │   │   │   ├── prepare.rs                                                           # knowledge prepare: T023's chunking as a leased job
│   │   │   │   ├── quality.rs                                                           # knowledge quality: the gate as a leased job; its inputs the ledger beside the declaration and the revisions
│   │   │   │   ├── retrieve.rs                                                          # CLI adapters for the shared, permission-scoped read operations
│   │   │   │   ├── run.rs                                                               # Parses the arguments, opens the kernel, runs the command, returns its exit code
│   │   │   │   ├── status.rs                                                            # knowledge status: documents, revisions by status and disposition, generations
│   │   │   │   ├── verify.rs                                                            # knowledge verify: checks a published generation as a leased job
│   │   │   │   └── wait.rs                                                              # job wait: a job's stream followed to its end, the command exiting with its outcome; the follower
│   │   │   ├── knowledge/                                                               # Scoped knowledge operations shared by the CLI and stdio MCP server
│   │   │   │   ├── operations/                                                          # Source-scoped kernel operations for the CLI and MCP read surfaces
│   │   │   │   │   ├── ask/                                                             # Ask
│   │   │   │   │   │   ├── mod.rs                                                       # The shared knowledge ask operation
│   │   │   │   │   │   ├── run.rs                                                       # One local ask operation shared by CLI and MCP
│   │   │   │   │   │   └── tests.rs                                                     # Rust source: tests
│   │   │   │   │   ├── implementation.rs                                                # Scoped application operations shared by CLI and MCP
│   │   │   │   │   ├── intent_tests.rs                                                  # Production searches take their query expander from the collection's
│   │   │   │   │   ├── mod.rs                                                           # Source-scoped kernel operations for the CLI and MCP read surfaces
│   │   │   │   │   ├── search.rs                                                        # Scoped search followed by the approved evidence assembly handoff
│   │   │   │   │   ├── search_request_tests.rs                                          # Rust source: search request tests
│   │   │   │   │   ├── stack_tests.rs                                                   # Search fits half the stack of a Windows main thread
│   │   │   │   │   └── tests.rs                                                         # Rust source: tests
│   │   │   │   ├── output/                                                              # Shared response bounds and semantic search truncation for CLI and MCP
│   │   │   │   │   ├── mod.rs                                                           # Shared response bounds and semantic search truncation for CLI and MCP
│   │   │   │   │   ├── policy.rs                                                        # Shared response bounds and semantic search truncation for CLI and MCP
│   │   │   │   │   └── tests.rs                                                         # Rust source: tests
│   │   │   │   ├── source_classes/                                                      # The source-class table this machine binds as source_classes: the
│   │   │   │   │   ├── load.rs                                                          # Loading the bound source-class table
│   │   │   │   │   ├── mod.rs                                                           # The source-class table this machine binds as source_classes: the
│   │   │   │   │   └── tests.rs                                                         # Rust source: tests
│   │   │   │   ├── mod.rs                                                               # Scoped knowledge operations shared by the CLI and stdio MCP server
│   │   │   │   └── requests.rs                                                          # Strict arguments shared by the CLI and MCP tools
│   │   │   ├── mcp/                                                                     # Bounded local stdio MCP transport and tools
│   │   │   │   ├── server/                                                              # Read-only MCP tools backed by fresh, local-principal kernel operations
│   │   │   │   │   ├── search/                                                          # Async MCP dispatch and complete-response bounds for search
│   │   │   │   │   │   └── tests.rs                                                     # Rust source: tests
│   │   │   │   │   ├── tests/                                                           # Integration tests
│   │   │   │   │   │   ├── call_deadlines.rs                                            # The MCP call caps against the search and chat caps they wrap
│   │   │   │   │   │   └── search_workers.rs                                            # Rust source: search workers
│   │   │   │   │   ├── warmup/                                                          # Background startup warming for visible published embedder cards
│   │   │   │   │   │   └── tests.rs                                                     # Rust source: tests
│   │   │   │   │   ├── handler.rs                                                       # MCP protocol handler and advertised knowledge tool schemas
│   │   │   │   │   ├── knowledge_server.rs                                              # The configured local knowledge server and its startup warm-up task
│   │   │   │   │   ├── mod.rs                                                           # Read-only MCP tools backed by fresh, local-principal kernel operations
│   │   │   │   │   ├── operations.rs                                                    # Strict MCP request parsing and bounded kernel workers
│   │   │   │   │   ├── response.rs                                                      # Bounded MCP response shapes, errors and wire-size checks
│   │   │   │   │   ├── search.rs                                                        # Async MCP dispatch and complete-response bounds for search
│   │   │   │   │   ├── span_tests.rs                                                    # Each tool call opens its gen_ai.execute_tool span, which names a listed
│   │   │   │   │   ├── tests.rs                                                         # Rust source: tests
│   │   │   │   │   ├── types.rs                                                         # Shared limits and kernel-open seam for the MCP server
│   │   │   │   │   ├── warmup.rs                                                        # Background startup warming for visible published embedder cards
│   │   │   │   │   └── wire_tests.rs                                                    # Wire-size and output-shaping tests for the MCP handler
│   │   │   │   ├── transport/                                                           # Stdio JSON-RPC framing with complete-line and complete-response byte bounds
│   │   │   │   │   └── tests.rs                                                         # Rust source: tests
│   │   │   │   ├── ask_tool.rs                                                          # MCP input and schema for knowledge_ask
│   │   │   │   ├── mod.rs                                                               # Bounded local stdio MCP transport and tools
│   │   │   │   ├── outcome.rs                                                           # How a tool call ended, for its span: by the public code of the error it
│   │   │   │   ├── run.rs                                                               # Runs the stdio MCP server without sending diagnostics to stdout
│   │   │   │   └── transport.rs                                                         # Stdio JSON-RPC framing with complete-line and complete-response byte bounds
│   │   │   ├── settings/                                                                # The session's settings, resolved once for the CLI and the MCP server, and
│   │   │   │   ├── knowledge.rs                                                         # The knowledge operations' settings, read from a session's resolved
│   │   │   │   ├── mod.rs                                                               # The session's settings, resolved once for the CLI and the MCP server, and
│   │   │   │   ├── session.rs                                                           # A session's settings: the registry, the files it reads and the explicit
│   │   │   │   └── tests.rs                                                             # The session's settings: every default is today's behaviour, each
│   │   │   ├── failure.rs                                                               # Failures at the CLI and local-kernel boundary
│   │   │   ├── kernel.rs                                                                # The kernel opened for the local principal and its configured scopes
│   │   │   └── main.rs                                                                  # The binary root: the commands, their output, exit codes and JSON schemas documented
│   │   ├── tests/                                                                       # Integration tests
│   │   │   └── it/                                                                      # The contract tests: the built binary run in a scratch home
│   │   │       ├── knowledge_get/                                                       # CLI and MCP contracts for exact knowledge retrieval
│   │   │       │   ├── ask_republish.rs                                                 # ask keeps the collection's selected reranker after a republish, as
│   │   │       │   ├── cli_cases.rs                                                     # Exact knowledge get CLI behavior
│   │   │       │   ├── cold_reranker.rs                                                 # A cold reranker costs CLI and MCP search only the rerank
│   │   │       │   ├── evidence_ceiling.rs                                              # CLI and MCP search accept evidence up to the 24,000-byte ceiling and refuse one byte past it
│   │   │       │   ├── knowledge_search.rs                                              # CLI and MCP search through the shared scoped evidence pipeline
│   │   │       │   ├── mcp_and_authorization.rs                                         # MCP parity, permission refresh, and exact-retrieval refusal cases
│   │   │       │   ├── mod.rs                                                           # CLI and MCP contracts for exact knowledge retrieval
│   │   │       │   ├── search_cli_failures.rs                                           # CLI search failures retain their documented process exit codes
│   │   │       │   ├── search_mcp_contract.rs                                           # MCP search advertises only the strict read-only search arguments
│   │   │       │   └── section_cases.rs                                                 # Exact canonical-section retrieval across the CLI and MCP surfaces
│   │   │       ├── rebuild_drill/                                                       # An owned backup-loss-rebuild ranking drill and its safety/equality checks
│   │   │       │   ├── backup_loss_drill/                                               # The opt-in real-service backup-loss and ranking-rebuild drill
│   │   │       │   │   ├── backup_loss.rs                                               # The opt-in real-service backup, projection-loss, restore and ranking proof
│   │   │       │   │   ├── mod.rs                                                       # The opt-in real-service backup-loss and ranking-rebuild drill
│   │   │       │   │   └── setup.rs                                                     # Synthetic inputs and CLI commands that start the owned recovery drill
│   │   │       │   ├── authority.rs                                                     # Backup manifest, restored authority, retained work and projection assertions
│   │   │       │   ├── fixture.rs                                                       # Frozen public questions, profiles and actual T029c search capture
│   │   │       │   ├── mod.rs                                                           # An owned backup-loss-rebuild ranking drill and its safety/equality checks
│   │   │       │   ├── qdrant.rs                                                        # Official Qdrant inspection and cleanup, restricted to the empty owned service
│   │   │       │   ├── ranking_oracle.rs                                                # Exact equality oracle for the synthetic T029c candidate rankings
│   │   │       │   ├── ranking_oracle_tests.rs                                          # Equality-oracle tests for ordered rankings and frozen search inputs
│   │   │       │   ├── resume.rs                                                        # A backed-up expired replacement attempt resumes against a new Qdrant target
│   │   │       │   └── wipe_safety.rs                                                   # Ownership checks for the destructive kernel and Qdrant portions
│   │   │       ├── backup_restore.rs                                                    # Backup and restore: online copies keep the database, artifacts and leased
│   │   │       ├── backup_restore_targets.rs                                            # Rust source: backup restore targets
│   │   │       ├── cli_contract.rs                                                      # JSON on stdout, diagnostics on stderr, exit codes 0, 1 and 2, the job ID first
│   │   │       ├── collection_status.rs                                                 # knowledge status of the synthetic collection: counts, dispositions and a generation
│   │   │       ├── doctor_checks.rs                                                     # maestro doctor: each failure names its next action, the router its address; v1 files listed, untouched
│   │   │       ├── fakes.rs                                                             # Fake curl and systemctl for the binary's tests, found first on the PATH, logging each call
│   │   │       ├── import_jobs.rs                                                       # knowledge import end to end, rerun, live holder refused, stale one superseded, leases taken over
│   │   │       ├── job_waits.rs                                                         # job wait follows a job to its end and exits with its outcome; an unreadable job is unknown
│   │   │       ├── knowledge_ask.rs                                                     # The public CLI refuses an invalid ask model before contacting a backend
│   │   │       ├── knowledge_collections.rs                                             # Visible metadata returned by knowledge collections
│   │   │       ├── knowledge_prepare_profiles.rs                                        # knowledge prepare --chunk-profile builds the chunk set of the chunking
│   │   │       ├── knowledge_prepare_v2.rs                                              # knowledge prepare qualifies a v2 embedder card against the tokenizer
│   │   │       ├── knowledge_publish.rs                                                 # knowledge prepare and knowledge publish refuse missing or unsuitable
│   │   │       ├── knowledge_verify_recheck.rs                                          # knowledge verify must read its artifacts again on every invocation
│   │   │       ├── machine.rs                                                           # How doctor and status tests run the binary: a router where nothing answers, the fakes on the PATH
│   │   │       ├── main.rs                                                              # The one integration-test crate of the binary
│   │   │       ├── mcp_clients.rs                                                       # Client-like MCP stdio round trips over the synthetic collection
│   │   │       ├── mcp_stdio.rs                                                         # The stdio MCP server's process boundary and advertised tools
│   │   │       ├── model_cli.rs                                                         # Registration and listing of scoped model cards through the public CLI
│   │   │       ├── model_cli_registration.rs                                            # Registration input, grant, and router-failure contract tests
│   │   │       ├── model_cli_selection.rs                                               # Selection reads only the evaluations of the exact card it selects
│   │   │       ├── n05_authority_contract.rs                                            # Read-only authority contract: exact scope/account/effect and authority-clock expiry
│   │   │       ├── n05_authority_frames.rs                                              # Actual Unix IPC framing refuses oversized, partial and cumulative-timeout input
│   │   │       ├── n05_authority_linux.rs                                               # Qualified Linux authority fixtures; only the launcher uses sudo
│   │   │       ├── n05_authority_review_fixes.rs                                        # Review regressions against real owner and unprivileged identities
│   │   │       ├── n05_establish_owner_only_grants_and_the_read_only_authority_port.rs  # N05 authority command refusals; real identity probes are explicitly opt-in
│   │   │       ├── publish_again.rs                                                     # Help for explicit projection recovery
│   │   │       ├── quality_gates.rs                                                     # knowledge quality: its report, a rerun, a gate after an import, the ledger first, failures and refusals
│   │   │       ├── settings_config.rs                                                   # maestro config: the user file preferences.toml, the project file
│   │   │       ├── setup_installs.rs                                                    # maestro setup: the preview writes nothing, a wrong download is refused; elsewhere manual steps, exit 2
│   │   │       ├── status_summaries.rs                                                  # maestro status: services ready or down, the readable collections, nothing created on a fresh machine
│   │   │       └── support.rs                                                           # What the contract tests share: a scratch home, the synthetic collection, the binary under a deadline
│   │   └── Cargo.toml                                                                   # Crate manifest: The command line of Maestro: collections, their imports, the jobs that run them, and the machine's setup and checks
│   ├── maestro-acquisition/                                                             # N03 implements strict source-policy declarations and immutable local baseline resolution
│   │   ├── src/                                                                         # The crate's sources
│   │   │   ├── adaptation/                                                              # Scoped local overlays and replaceable activation policy and commit ports
│   │   │   │   ├── change.rs                                                            # Pure closed automatic-change control over freshly resolved, typed values
│   │   │   │   ├── golden-activation-recovery.json                                      # JSON data: golden activation recovery
│   │   │   │   ├── golden-activation.json                                               # JSON data: golden activation
│   │   │   │   ├── golden-effective.json                                                # JSON data: golden effective
│   │   │   │   ├── golden-manifest.json                                                 # JSON data: golden manifest
│   │   │   │   ├── golden-proposal-recovery.json                                        # JSON data: golden proposal recovery
│   │   │   │   ├── golden-proposal.json                                                 # JSON data: golden proposal
│   │   │   │   ├── golden.rs                                                            # Frozen typed preimages: declaration order must not depend on JSON map features
│   │   │   │   ├── lineage.rs                                                           # Validate only payloads that contribute to the effective configuration
│   │   │   │   ├── manifest.rs                                                          # Typed local overlays and replaceable activation/recovery policy seams
│   │   │   │   ├── mod.rs                                                               # Scoped local overlays and replaceable activation policy and commit ports
│   │   │   │   ├── recovery.rs                                                          # Small strict transaction markers, verified against the retained old pointer
│   │   │   │   ├── recovery_tests.rs                                                    # Each transition guard is checked directly, without lineage checks masking it
│   │   │   │   ├── storage.rs                                                           # Local overlay I/O and protected receipt identity helpers
│   │   │   │   └── writer.rs                                                            # Immutable baseline plus scoped local overlays; pointer replacement commits
│   │   │   ├── extraction/                                                              # Immutable extraction profiles and bounded pure selection
│   │   │   │   ├── detect.rs                                                            # Bounded deterministic evidence matching; no parser, network or model starts
│   │   │   │   ├── mod.rs                                                               # Immutable extraction profiles and bounded pure selection
│   │   │   │   ├── model.rs                                                             # Strict immutable profile definitions; field order is part of digest version one
│   │   │   │   ├── outcome.rs                                                           # Immutable checked handles and content-free registry outcomes
│   │   │   │   └── registry.rs                                                          # Pure replaceable registry adapter and core revalidation boundaries
│   │   │   ├── lifecycle/                                                               # Process-local acquisition lifecycle accounting
│   │   │   │   ├── mod.rs                                                               # Process-local acquisition lifecycle accounting
│   │   │   │   └── resources.rs                                                         # Shared owned reservations, with fresh measurements at every checkpoint
│   │   │   ├── policy/                                                                  # Strict policy schemas and the single immutable baseline validator
│   │   │   │   ├── acquisition.rs                                                       # Strict version-one source-policy wire contracts
│   │   │   │   ├── authority.rs                                                         # Read-only acquisition authority; manifests and registry evidence grant no access
│   │   │   │   ├── authority_socket.rs                                                  # Read-only local IPC adapter: both peers are kernel-authenticated, never JSON identities
│   │   │   │   ├── checks.rs                                                            # Semantic constraints not expressible by primitive serde shapes
│   │   │   │   ├── decision.rs                                                          # One pure admission path: current caller, URL/network/robots, content, cache
│   │   │   │   ├── decisions.rs                                                         # Strict version-one source-policy wire contracts
│   │   │   │   ├── identity.rs                                                          # URL identity is explicit, versioned and separate from protected references
│   │   │   │   ├── limits.rs                                                            # Strict version-one source-policy wire contracts
│   │   │   │   ├── manifest.rs                                                          # Strict version-one source-policy wire contracts
│   │   │   │   ├── mod.rs                                                               # Strict policy schemas and the single immutable baseline validator
│   │   │   │   ├── resolve.rs                                                           # One strict validator for local files and immutable catalog resources
│   │   │   │   ├── resource.rs                                                          # Shared scoped resource identity; never an acquisition grant
│   │   │   │   ├── schema.rs                                                            # Strict version-one source-policy wire contracts
│   │   │   │   ├── shape.rs                                                             # Objects, scalar names and bounded strings, never positional arrays or scripts
│   │   │   │   ├── source.rs                                                            # Strict version-one source-policy wire contracts
│   │   │   │   ├── utc.rs                                                               # One strict Gregorian UTC implementation for grants and transport times
│   │   │   │   └── wiki.rs                                                              # Strict version-one source-policy wire contracts
│   │   │   ├── transport/                                                               # Transport
│   │   │   │   ├── address.rs                                                           # Deny-only address resources plus a non-removable special-purpose floor
│   │   │   │   ├── budget.rs                                                            # Field-kind composition and typed resource-induced pending work
│   │   │   │   ├── connect.rs                                                           # One checked destination per connection, with no proxy, pool or second DNS
│   │   │   │   ├── dns.rs                                                               # Read-only native DNS waiting is bounded independently of the HTTP future
│   │   │   │   ├── failure.rs                                                           # Content-free transport and decode refusal contract
│   │   │   │   ├── floor.rs                                                             # Non-removable IANA snapshot (2025-10-09), multicast and metadata denials
│   │   │   │   ├── http.rs                                                              # Bounded HTTP/1 over one freshly admitted pinned connection per hop
│   │   │   │   ├── http_protocol.rs                                                     # Bounded response parser, transient metadata and manual redirect spelling
│   │   │   │   ├── mod.rs                                                               # Rust source: mod
│   │   │   │   ├── pacing.rs                                                            # One injected atomic origin ledger shared by HTTP, browser and resumed runs
│   │   │   │   ├── robots.rs                                                            # RFC 9309 rules over N07's canonical fetch identity, without network effects
│   │   │   │   ├── robots_store.rs                                                      # Bounded robots entries; different policy digests on one origin coexist
│   │   │   │   ├── stream.rs                                                            # Shared cumulative wire/decode accounting, reusable by downstream decoders
│   │   │   │   └── wire_quota.rs                                                        # Gate header read-ahead and cap every raw post-header read before I/O
│   │   │   ├── files.rs                                                                 # Caller-bound direct files, never source-provided paths or writable bundles
│   │   │   ├── lib.rs                                                                   # Strict source-policy configuration for native acquisition
│   │   │   ├── ports.rs                                                                 # Small read-only ports; authority and transport startup are deliberately absent
│   │   │   └── refusal.rs                                                               # Content-free policy failure codes: no source bytes or paths in diagnostics
│   │   ├── tests/                                                                       # Integration tests
│   │   │   ├── fixtures/                                                                # Test fixtures
│   │   │   │   ├── tls/                                                                 # Tls
│   │   │   │   │   ├── certificate.pem                                                  # File: certificate
│   │   │   │   │   └── synthetic-key.json                                               # JSON data: synthetic key
│   │   │   │   ├── address-table.json                                                   # JSON data: address table
│   │   │   │   ├── collection.json                                                      # JSON data: collection
│   │   │   │   ├── decisions.json                                                       # JSON data: decisions
│   │   │   │   ├── http.json                                                            # JSON data: http
│   │   │   │   ├── policy.json                                                          # JSON data: policy
│   │   │   │   └── profile-definition-v1.txt                                            # Text: profile definition v1
│   │   │   └── it/                                                                      # It
│   │   │       ├── main.rs                                                              # N03's synthetic integration contracts, in one test binary
│   │   │       ├── n03_implement_strict_source_policy_and_local_baseline_resolution.rs  # N03: malformed policy closure must refuse before caller-side effects
│   │   │       ├── n03_review_fixes.rs                                                  # Four ruled N03 parser and schema regressions
│   │   │       ├── n07_parse_url_identity_and_denial_precedence.rs                      # N07 URL and denial contracts use only synthetic destinations
│   │   │       ├── n07_url_policy_edges.rs                                              # N07 edge guards share the primary task's synthetic fixture builders
│   │   │       ├── n07_url_review_regressions.rs                                        # N07 review regressions share the checked synthetic policy closure
│   │   │       ├── n08_classify_and_pin_every_destination_address.rs                    # N08 uses real admission with explicit synthetic resolver/connection adapters
│   │   │       ├── n09_accounting.rs                                                    # Small public accounting checks for downstream reuse and exact boundaries
│   │   │       ├── n09_authority_connect.rs                                             # Supervisor-approved N05 fix: IPC connect never blocks on a full backlog
│   │   │       ├── n09_boundaries.rs                                                    # Read-side quotas, trailer reservation and async completion neighbours
│   │   │       ├── n09_controls.rs                                                      # Caller/network/robots refusals precede any HTTP-owned effects
│   │   │       ├── n09_decode.rs                                                        # Cumulative decoder guards, independently selected synthetic bombs
│   │   │       ├── n09_gzip_members.rs                                                  # RFC 1952 member accounting is shared with every decode stage
│   │   │       ├── n09_hop_edges.rs                                                     # Attempt, redirect, authority and relative-reference boundary guards
│   │   │       ├── n09_implement_bounded_admitted_http_transport.rs                     # N09 transport guards exercised over real hyper HTTP/1.1
│   │   │       ├── n09_interim.rs                                                       # Informational blocks share finite header bounds without body read-ahead
│   │   │       ├── n09_pacing.rs                                                        # HTTP owns pacing for first hops, redirects, retries and robots refreshes
│   │   │       ├── n09_pacing_support.rs                                                # Recording fake that delegates N10 semantics while observing HTTP ownership
│   │   │       ├── n09_parser.rs                                                        # Parser allocation floors and hostile header/status lines stay bounded
│   │   │       ├── n09_profiles.rs                                                      # Checked browser profiles must never silently fall back to content HTTP
│   │   │       ├── n09_retained.rs                                                      # Tightened cumulative envelopes refuse before further effects, even without DATA
│   │   │       ├── n09_review_probes.rs                                                 # Independent review probes: failures confirm suspected N09 regressions
│   │   │       ├── n09_review_support.rs                                                # Test-owned timing and segmented I/O instrumentation for N09 review probes
│   │   │       ├── n09_robots.rs                                                        # Robots transport outcomes reach N10's actual fail-closed mapping
│   │   │       ├── n09_run_deadline.rs                                                  # The independent logical-run cutoff constrains still-live document work
│   │   │       ├── n09_support.rs                                                       # Synthetic admitted HTTP/1.1 connections, never a network listener
│   │   │       ├── n09_sync_deadlines.rs                                                # Synchronous ports cannot consume a document deadline then dial a socket
│   │   │       ├── n10_admission.rs                                                     # Robots denial reaches N07's real admission path before transport effects
│   │   │       ├── n10_conform_robots_and_aggregate_origin_pacing.rs                    # Independently authored RFC 9309 and shared-origin contracts
│   │   │       ├── n10_control_edges.rs                                                 # Additional actual-engine boundaries and substitute rules adapter
│   │   │       ├── n10_review_regressions.rs                                            # RFC 9309 regression assertions from the independent N10 review probes
│   │   │       ├── n11_account_aggregate_resources_and_interactive_priority.rs          # Synthetic N11 accounting contracts; no processes, live grants or GPU jobs
│   │   │       ├── n11_resource_edges.rs                                                # N11 concurrent admission, overflow and resume edge contracts
│   │   │       ├── n11_review_regressions.rs                                            # N11 review regressions for composed ceilings and retained allocation ownership
│   │   │       ├── n11_support.rs                                                       # Shared synthetic explicit N11 host/settings fixtures
│   │   │       ├── n15_bounds.rs                                                        # Independent shape/qualification guard neighbours, not parser-error mutants
│   │   │       ├── n15_policy_binding.rs                                                # Genuine admitted handles exercise each core policy binding independently
│   │   │       ├── n15_qualification_closure.rs                                         # Shared/cyclic closures remain bounded by the existing logical resource cache
│   │   │       ├── n15_resources.rs                                                     # Unmasked immutable-resource and policy-binding neighbours
│   │   │       ├── n15_review.rs                                                        # Permanent qualification and wire-shape regressions from the N15 review
│   │   │       ├── n15_route_content_through_one_extensible_profile_registry.rs         # One resolve/select contract over local, synthetic catalog and disabled ports
│   │   │       ├── n15_selection_edges.rs                                               # Selection predicates with qualified, pinned positive and negative neighbours
│   │   │       ├── n15_support.rs                                                       # Independently authored synthetic registry resources, never catalog trust
│   │   │       ├── n15_validation.rs                                                    # Independent boundary fixtures for registry guards and unmasked red proofs
│   │   │       ├── n30_additional_contracts.rs                                          # Typed proposal inputs, fresh CAS and scoped inheritance regressions
│   │   │       ├── n30_implement_proposal_and_activation_manifest_write_port.rs         # Synthetic write-port conformance; no installed catalog admission is claimed
│   │   │       ├── n30_lineage_regressions.rs                                           # Digest-consistent payload substitutions must not break effective ancestry
│   │   │       ├── n30_mutation_regressions.rs                                          # Regression checks for scope inheritance and pre-exposure/notification guards
│   │   │       ├── n30_recovery_guards.rs                                               # Recovery, rollback and filesystem refusal contracts
│   │   │       ├── n30_review_regressions.rs                                            # N30 review regressions: current authority, effective lineage and recovery
│   │   │       ├── n30_support.rs                                                       # Synthetic write-port conformance; no installed catalog admission is claimed
│   │   │       ├── n30_transition_regressions.rs                                        # Recovery must prove an exact pointer transition before delivery
│   │   │       ├── n30_write_guards.rs                                                  # Individual safety guards and recovery identity proofs for N30
│   │   │       ├── n32_change_guards.rs                                                 # Isolated boundary probes for N32's pure controls
│   │   │       ├── n32_enforce_the_closed_automatic_change_allow_list.rs                # Pure synthetic N32 controls; runtime artifact contracts belong to N34
│   │   │       ├── n32_support.rs                                                       # Pure synthetic N32 controls; runtime artifact contracts belong to N34
│   │   │       └── support.rs                                                           # Complete independently authored synthetic policy fixtures
│   │   ├── Cargo.toml                                                                   # Crate manifest: Strict source policies and immutable baseline resolution for native acquisition
│   │   └── README.md                                                                    # N03 implements strict source-policy declarations and immutable local baseline resolution
│   ├── maestro-canonicalization/                                                        # Local Rust library and CLI: completed Markdown + supplied metadata → parsed structure → validation → CanonicalDocument
│   │   ├── examples/                                                                    # Worked examples
│   │   │   ├── assets/                                                                  # Images and other assets
│   │   │   │   └── flow.svg                                                             # SVG image: flow
│   │   │   ├── expected/                                                                # What the tool must write for the example input
│   │   │   │   ├── canonical.json                                                       # JSON data: canonical
│   │   │   │   └── original.md                                                          # Sample document: Operations
│   │   │   ├── .rumdl.toml                                                              # The example is a fixture, not documentation: input.md is Markdown as an author wrote it, duplicate headings and all
│   │   │   ├── input.md                                                                 # Sample document: Operations
│   │   │   └── metadata.json                                                            # JSON data: metadata
│   │   ├── src/                                                                         # The crate's sources
│   │   │   ├── chunk_mapping/                                                           # Phase B-only mappings; original source offsets are never rendered offsets
│   │   │   │   ├── document.rs                                                          # Mapping a canonical document into source units: each block's context and inline text, with
│   │   │   │   ├── mod.rs                                                               # Phase B-only mappings; original source offsets are never rendered offsets
│   │   │   │   ├── refusal.rs                                                           # The refusal every mapping check returns
│   │   │   │   ├── slice.rs                                                             # Mapped slices and the source accounting ledger: which original bytes each unit covers
│   │   │   │   └── tests.rs                                                             # Tests of source mapping: order, Unicode, entities, envelopes and accounting
│   │   │   ├── chunk_split/                                                             # Structural preparation and packing; only chunk_documents, through a verified TokenCounter, certifies counts
│   │   │   │   ├── tests/                                                               # Tests of structural preparation and packing
│   │   │   │   │   ├── boundaries.rs                                                    # The pure preparation helpers: cut points, fitting prefixes and delimiter-safe ranges
│   │   │   │   │   ├── chrome.rs                                                        # Page chrome under the complete-ideas profile: only a label or markup of known HTML elements
│   │   │   │   │   ├── context.rs                                                       # Context, characterized on small documents: the exact prepared input of each chunk
│   │   │   │   │   ├── ideas.rs                                                         # The complete-ideas profile: page chrome left out of the indexed text, and a section's blocks
│   │   │   │   │   ├── mod.rs                                                           # Tests of structural preparation and packing
│   │   │   │   │   ├── oversized.rs                                                     # Oversized units: refused by name, with their block and its span, never by their text
│   │   │   │   │   ├── packing.rs                                                       # Packing and preparation: shared chunks, context text, containers, part numbers and the table
│   │   │   │   │   └── splitting.rs                                                     # Splitting, characterized on small documents: where oversized units and rows are cut
│   │   │   │   ├── chrome.rs                                                            # Page chrome: interface labels an HTML export keeps around a page's content, which the
│   │   │   │   ├── context.rs                                                           # The context a chunk repeats: headings, parent items, task markers and table headers
│   │   │   │   ├── drafts.rs                                                            # Packing a document's atoms into drafts: combined up to the target, refined or split past the
│   │   │   │   ├── ideas.rs                                                             # Ideas: the atoms that belong in one chunk
│   │   │   │   ├── layout.rs                                                            # The layout's structural queries: owners, sections, table windows and packing atoms
│   │   │   │   ├── limits.rs                                                            # The token budgets drafts grow toward and never exceed
│   │   │   │   ├── mod.rs                                                               # Structural preparation and packing; only chunk_documents, through a verified TokenCounter, certifies counts
│   │   │   │   ├── prepare.rs                                                           # The prepared input: body parts, formatting, sentence boundaries and fitting prefixes
│   │   │   │   ├── ranges.rs                                                            # Unit ranges a chunk may hold: never starting or ending inside an inline delimiter
│   │   │   │   ├── refusal.rs                                                           # The refusal every structural check returns
│   │   │   │   ├── replay.rs                                                            # Replaying a chunk's preparation: its table windows against its fragments, then the chunk
│   │   │   │   └── structure.rs                                                         # A document's structure indexed for chunking, the bodies packed from it and the context they
│   │   │   ├── chunks/                                                                  # Phase B chunk batches: assembly, prepared-input identities and replay validation
│   │   │   │   ├── tests/                                                               # Tests of chunk assembly, prepared-input groups and replay validation
│   │   │   │   │   ├── counter.rs                                                       # Tests of the counting seam: any TokenCounter chunks, verified around its batch
│   │   │   │   │   ├── identity.rs                                                      # Identities: chunk and prepared-input identities follow scope and content alone
│   │   │   │   │   ├── mod.rs                                                           # Tests of chunk assembly, prepared-input groups and replay validation
│   │   │   │   │   ├── profiles.rs                                                      # Profiles: the pinned default batch and what each profile names
│   │   │   │   │   └── replay.rs                                                        # Replay validation: coverage and prepared parts must rebuild from the mapped source
│   │   │   │   ├── batch.rs                                                             # Batch records: per-occurrence evidence, retrieval chunks and prepared-input groups
│   │   │   │   ├── build.rs                                                             # Batch assembly: map, split, replay and identify every authorized occurrence
│   │   │   │   ├── chrome.rs                                                            # The page chrome a profile leaves out of a document's indexed text, counted per rule
│   │   │   │   ├── identity.rs                                                          # Prepared-input groups: identical prepared inputs share one identity
│   │   │   │   ├── mod.rs                                                               # Phase B chunk batches: assembly, prepared-input identities and replay validation
│   │   │   │   └── validation.rs                                                        # Replay checks: coverage and every prepared part must rebuild from the mapped source
│   │   │   ├── filesystem/                                                              # Filesystem access that never follows a link, behind one interface: rustix's directory-relative
│   │   │   │   ├── mod.rs                                                               # Filesystem access that never follows a link, behind one interface: rustix's directory-relative
│   │   │   │   ├── root.rs                                                              # The root a caller names, resolved once, and the names the store appends below it
│   │   │   │   ├── unix.rs                                                              # Unix: every name resolves against an open directory through rustix's openat family, which
│   │   │   │   └── windows.rs                                                           # Windows: names resolve by path, but every directory on the way is held open without
│   │   │   ├── tokenizer/                                                               # Token counting: the TokenCounter seam, and the qualified local executable that fills it
│   │   │   │   ├── tests/                                                               # Tests of the native tokenizer: profile identity, artifacts, process limits and output
│   │   │   │   │   ├── invocation.rs                                                    # Tests of the counter's invocation: its environment, its arguments and each platform's loader
│   │   │   │   │   ├── libraries.rs                                                     # Tests of the library inventory: each platform's naming, its aliases and where they resolve
│   │   │   │   │   └── mod.rs                                                           # Tests of the native tokenizer: profile identity, artifacts, process limits and output
│   │   │   │   ├── artifacts.rs                                                         # Artifact checks: pinned files by size and SHA-256, and the exact library inventory
│   │   │   │   ├── binding.rs                                                           # Where this machine keeps the artifacts the tokenizer profile fingerprints
│   │   │   │   ├── contract.rs                                                          # The committed qualification profile: its identity and typed access to its fields
│   │   │   │   ├── counter.rs                                                           # The seam every token counter fills: what the chunker counts through, whoever counts
│   │   │   │   ├── loader.rs                                                            # How each platform's dynamic loader is made to load the verified libraries, and only them
│   │   │   │   ├── mod.rs                                                               # Token counting: the TokenCounter seam, and the qualified local executable that fills it
│   │   │   │   ├── native.rs                                                            # The pinned native tokenizer: verified artifacts and one counter process per input
│   │   │   │   └── process.rs                                                           # The counter subprocess: bounded pipes, a timeout and a child that is always reaped
│   │   │   ├── unit_graph/                                                              # Opt-in /4 mapped delivery graph and digest-pinned ranking view
│   │   │   │   ├── tests/                                                               # Unit graph profile and delivery contract regressions
│   │   │   │   │   ├── build_producer.rs                                                # Rust source: build producer
│   │   │   │   │   ├── c1.rs                                                            # Rust source: c1
│   │   │   │   │   ├── c2.rs                                                            # Rust source: c2
│   │   │   │   │   ├── c3.rs                                                            # Rust source: c3
│   │   │   │   │   ├── c4.rs                                                            # Rust source: c4
│   │   │   │   │   ├── c5.rs                                                            # Rust source: c5
│   │   │   │   │   ├── groups_producer.rs                                               # Rust source: groups producer
│   │   │   │   │   ├── helpers.rs                                                       # Rust source: helpers
│   │   │   │   │   ├── i1.rs                                                            # Rust source: i1
│   │   │   │   │   ├── i2.rs                                                            # Rust source: i2
│   │   │   │   │   ├── i3.rs                                                            # Rust source: i3
│   │   │   │   │   ├── i4.rs                                                            # Rust source: i4
│   │   │   │   │   ├── i5.rs                                                            # Rust source: i5
│   │   │   │   │   ├── i6.rs                                                            # Rust source: i6
│   │   │   │   │   ├── i7.rs                                                            # Rust source: i7
│   │   │   │   │   ├── mod.rs                                                           # Unit graph profile and delivery contract regressions
│   │   │   │   │   ├── producer_rows.rs                                                 # Rust source: producer rows
│   │   │   │   │   └── producer_rows_more.rs                                            # Rust source: producer rows more
│   │   │   │   ├── build.rs                                                             # Build a delivery graph from checked canonical mappings and ranked views
│   │   │   │   ├── build_units.rs                                                       # Assemble complete delivery units from primary canonical mappings
│   │   │   │   ├── group_helpers.rs                                                     # Canonical source ancestry and stable identifiers for graph groups
│   │   │   │   ├── group_links.rs                                                       # Reciprocal group links and source-ordered structural occurrences
│   │   │   │   ├── groups.rs                                                            # Deterministic page, section, table, procedure and code ancestry
│   │   │   │   ├── ledger.rs                                                            # Exactly-once primary ownership and explicit source exclusions
│   │   │   │   ├── mod.rs                                                               # Opt-in /4 mapped delivery graph and digest-pinned ranking view
│   │   │   │   ├── prepared.rs                                                          # Retrieval preparation from ancestor headings and typed context relations
│   │   │   │   ├── profile.rs                                                           # Digest-pinned /4 graph and ranking rules
│   │   │   │   ├── serialization.rs                                                     # Stable graph payload serialization and digest
│   │   │   │   ├── table_packing.rs                                                     # Whole-table packing when its complete prepared input fits the pinned counter limit
│   │   │   │   ├── types.rs                                                             # Deterministic, digest-bound /4 delivery records
│   │   │   │   ├── validation.rs                                                        # Reject malformed graph identities, source ranges and exact memberships
│   │   │   │   └── wire_types.rs                                                        # Private serde DTOs for the versioned graph and mapping payloads
│   │   │   ├── validate/                                                                # Structural checks against the preserved bytes; no guessed repairs
│   │   │   │   ├── blocks.rs                                                            # Block checks: children, parents, assets, attributes, inline content and tables
│   │   │   │   ├── mod.rs                                                               # Structural checks against the preserved bytes; no guessed repairs
│   │   │   │   ├── report.rs                                                            # How a check records a finding located at a block's spans
│   │   │   │   ├── sections.rs                                                          # Section and extractor checks: the heading hierarchy and supplied extractor anchors
│   │   │   │   ├── structure.rs                                                         # Whole-document structural checks: reference, block hierarchy, source ledger and coverage
│   │   │   │   └── tests.rs                                                             # Structural checks against documents altered one field at a time: each fault is reported
│   │   │   ├── accounting.rs                                                            # A deterministic, unique byte partition; nested block spans remain independently valid
│   │   │   ├── assemble.rs                                                              # Build natural blocks and lexical heading context from the offset-aware tree
│   │   │   ├── chunk_profile.rs                                                         # Chunking profiles: which blocks share a chunk and what its prepared input leaves out
│   │   │   ├── cli.rs                                                                   # The command line: its usage, its flags, the metadata sidecar and the run that canonicalizes
│   │   │   ├── content.rs                                                               # Typed natural blocks and nested inline content
│   │   │   ├── dedup.rs                                                                 # Pure, scoped exact grouping; equality never merges identity or grants access
│   │   │   ├── document.rs                                                              # The canonical document: the versioned record canonicalization returns, its heading sections
│   │   │   ├── error.rs                                                                 # The crate's one error type: a refusal that says why, never the source text
│   │   │   ├── hashing.rs                                                               # SHA-256 as lower-case hexadecimal, the form every identity and content hash takes
│   │   │   ├── lib.rs                                                                   # Local, deterministic canonical documents
│   │   │   ├── local_assets.rs                                                          # The binary's filesystem look-up of local asset destinations; the library itself does no I/O
│   │   │   ├── main.rs                                                                  # Local CLI for Markdown canonicalization
│   │   │   ├── metadata.rs                                                              # Merge supplied metadata without guessing provenance or permissions
│   │   │   ├── model.rs                                                                 # Database-independent provenance and versioning contract
│   │   │   ├── parse.rs                                                                 # Offset-aware parser tree
│   │   │   ├── pipeline.rs                                                              # The canonicalization pipeline: parse, merge metadata, assemble blocks, keep what the parser
│   │   │   ├── prepared_inputs.rs                                                       # The prepared embedding input: its parts, primary fragments and table windows
│   │   │   ├── replay.rs                                                                # Validation by replay: canonicalize the reference bytes again with a document's recorded inputs
│   │   │   ├── source_units.rs                                                          # Mapped source units: derived-text ranges and how each run relates to original source
│   │   │   └── store.rs                                                                 # Immutable snapshots, accessed through directory handles without following symlinks
│   │   ├── tests/                                                                       # Integration tests
│   │   │   ├── fixtures/                                                                # Test fixtures
│   │   │   │   ├── unit-graph-v1.built.json                                             # JSON data: unit graph v1.built
│   │   │   │   ├── unit-graph-v1.built.md                                               # Sample document: Product guide
│   │   │   │   ├── unit-graph-v1.code_in_list.json                                      # JSON data: unit graph v1.code in list
│   │   │   │   ├── unit-graph-v1.code_in_list.md                                        # Sample document: T
│   │   │   │   ├── unit-graph-v1.nested_list.json                                       # JSON data: unit graph v1.nested list
│   │   │   │   ├── unit-graph-v1.nested_list.md                                         # Sample document: T
│   │   │   │   ├── unit-graph-v1.table_in_list.json                                     # JSON data: unit graph v1.table in list
│   │   │   │   ├── unit-graph-v1.table_in_list.md                                       # Sample document: T
│   │   │   │   ├── unit-mapping-v1.built.json                                           # JSON data: unit mapping v1.built
│   │   │   │   ├── unit-mapping-v1.code_in_list.json                                    # JSON data: unit mapping v1.code in list
│   │   │   │   ├── unit-mapping-v1.nested_list.json                                     # JSON data: unit mapping v1.nested list
│   │   │   │   └── unit-mapping-v1.table_in_list.json                                   # JSON data: unit mapping v1.table in list
│   │   │   └── it/                                                                      # The crate's integration tests, built as one test crate
│   │   │       ├── cli_contract/                                                        # The command-line tool's contract: arguments, exit codes and saved documents
│   │   │       │   ├── fixture.rs                                                       # The scratch directory each command-line test runs the tool in
│   │   │       │   ├── invocation_contract.rs                                           # Running the tool: arguments, exit codes, source bytes, sidecars, assets
│   │   │       │   ├── mod.rs                                                           # The command-line tool's contract: arguments, exit codes and saved documents
│   │   │       │   └── snapshot_contract.rs                                             # Saved snapshots: publication, recovery, verified loading and the roots
│   │   │       ├── dedup_contract/                                                      # Exact duplicate grouping: stable occurrences, authorized scope and whole-batch refusals
│   │   │       │   ├── authorized_scope.rs                                              # The authorized scope bounds each group, and an invalid, duplicate
│   │   │       │   ├── batch_inputs.rs                                                  # The documents and the authorized scope each grouping test starts from
│   │   │       │   ├── content_equality.rs                                              # What groups occurrences: equal content in a stable order, canonical
│   │   │       │   ├── mod.rs                                                           # Exact duplicate grouping: stable occurrences, authorized scope and whole-batch refusals
│   │   │       │   └── revision_history.rs                                              # Revisions of one document grouped together keep their own policy
│   │   │       ├── chunk_contract.rs                                                    # Public API boundary: a counter from another crate chunks through the crate-root trait
│   │   │       ├── chunk_native.rs                                                      # Explicit local acceptance: never treat an ignored native test as a pass
│   │   │       ├── dialect_properties.rs                                                # Generated Markdown dialects keep their spans, meaning and round trips, deterministically
│   │   │       ├── document_contract.rs                                                 # The canonical document's contract: structure, spans and provenance as the source gives them
│   │   │       ├── main.rs                                                              # The crate's integration tests, built as one test crate: each module proves
│   │   │       ├── phase_a_acceptance.rs                                                # Phase A acceptance: every source byte is accounted for and no content is silently hidden
│   │   │       └── validation_boundary.rs                                               # Regression checks for review findings at the source and JSON trust boundaries
│   │   ├── ACCEPTANCE.md                                                                # Phase A acceptance — canonicalization
│   │   ├── CHUNKING.md                                                                  # Mapped structural chunking
│   │   ├── Cargo.toml                                                                   # Crate manifest
│   │   ├── DEDUPLICATION.md                                                             # Scoped exact duplicate grouping
│   │   ├── README.md                                                                    # Local Rust library and CLI: completed Markdown + supplied metadata → parsed structure → validation → CanonicalDocument
│   │   ├── TOKENIZER.md                                                                 # Local GGUF tokenizer
│   │   ├── VERIFICATION.md                                                              # Initial verification — 2026-09-21 (historical)
│   │   └── tokenizer-contract.json                                                      # JSON data: tokenizer contract
│   ├── maestro-conventions/                                                             # Maestro conventions
│   │   ├── src/                                                                         # The crate's sources
│   │   │   └── lib.rs                                                                   # Helpers for the repository's policy tests: the files the repository holds
│   │   ├── tests/                                                                       # Integration tests
│   │   │   ├── s1_traceability/                                                         # MR-07: the owner-approved S1 row keys and their delivery map
│   │   │   │   ├── keys.rs                                                              # The 08 row keys the owner approved for S1 on 2026-09-28 (MR-07)
│   │   │   │   ├── mod.rs                                                               # MR-07: the owner-approved S1 row keys and their delivery map
│   │   │   │   └── row_key_checks.rs                                                    # The spec and 08 §20.4 carry exactly the approved keys, each an 08 row
│   │   │   └── policies.rs                                                              # The repository's policies, checked on every pull request by cargo test
│   │   └── Cargo.toml                                                                   # Crate manifest: Tests that hold the maestro-core repository to its own policies
│   ├── maestro-kernel/                                                                  # Maestro kernel
│   │   ├── migrations/                                                                  # The kernel database's migrations, embedded and applied in number order
│   │   │   ├── 0001_artifacts.sql                                                       # The artifacts table: each artifact's size, media type, pins and creation time
│   │   │   ├── 0002_journal.sql                                                         # The journal: the append-only events, their triggers, and the consumers' cursors
│   │   │   ├── 0003_scopes.sql                                                          # The grants: each principal's rights on a scope and on every scope below it
│   │   │   ├── 0004_documents.sql                                                       # The pipeline's records: collections, sources, documents, revisions, their dispositions, chunk sets, chunks and generations
│   │   │   ├── 0005_jobs.sql                                                            # The jobs table: each job's key, attempt, resource, state, lease and outcome, and its triggers
│   │   │   ├── 0006_eval_reports.sql                                                    # The evaluation reports: each run's collection, generation, suite and report artifact, and the triggers that keep it as recorded
│   │   │   ├── 0007_chunk_sets.sql                                                      # The guards of the chunk sets and their chunks: their states, moves and identity, and a complete set's chunks as counted
│   │   │   ├── 0008_document_guards.sql                                                 # The guards of the documents: each keeps its id, collection, source and source reference
│   │   │   ├── 0009_model_cards.sql                                                     # V2 cards, evaluations, selections and immutable triggers
│   │   │   ├── 0010_search.sql                                                          # File: 0010 search
│   │   │   ├── 0011_exact_identifiers.sql                                               # Exact, publish-time identifier membership
│   │   │   ├── 0017_unit_graphs.sql                                                     # File: 0017 unit graphs
│   │   │   ├── 0018_retrieval_representations.sql                                       # File: 0018 retrieval representations
│   │   │   ├── 0019_acquisition_frontier.sql                                            # File: 0019 acquisition frontier
│   │   │   └── 0020_acquisition_receipts.sql                                            # File: 0020 acquisition receipts
│   │   ├── src/                                                                         # The crate's sources
│   │   │   ├── acquisition/                                                             # Kernel-owned acquisition work, exposed through a replaceable frontier port
│   │   │   │   ├── error.rs                                                             # Refusals from the frontier port; no refused operation acknowledges work
│   │   │   │   ├── frontier.rs                                                          # Replaceable frontier contract and the kernel SQLite adapter
│   │   │   │   ├── lease.rs                                                             # Fencing handles: durable authority time plus a local monotonic deadline
│   │   │   │   ├── mod.rs                                                               # Kernel-owned acquisition work, exposed through a replaceable frontier port
│   │   │   │   ├── privacy.rs                                                           # Content-free output types and opaque, transitively scoped artifact handles
│   │   │   │   ├── receipt.rs                                                           # Unique run attempts, immutable receipt snapshots and bounded stage inventories
│   │   │   │   └── record.rs                                                            # Durable request identities and item rows; contexts are never normalized together
│   │   │   ├── artifact/                                                                # Content-addressed artifacts: immutable bytes stored, and read back, by their
│   │   │   │   ├── digest.rs                                                            # A SHA-256 digest: the name every artifact is stored under
│   │   │   │   ├── mod.rs                                                               # Content-addressed artifacts: immutable bytes stored, and read back, by their
│   │   │   │   ├── store.rs                                                             # The store: artifacts written once under their digest, read back checked
│   │   │   │   └── tests.rs                                                             # Tests of the artifact store: digests, writes, repairs and refusals, with
│   │   │   ├── capability/                                                              # Capabilities: the registry of the tools Maestro offers (building block B9)
│   │   │   │   ├── mod.rs                                                               # Capabilities: the registry of the tools Maestro offers (building block B9)
│   │   │   │   ├── registry.rs                                                          # The registry: each tool once, with what it takes, does and needs
│   │   │   │   └── tests.rs                                                             # Tests of the capability registry: declarations, refusals and order
│   │   │   ├── chunk_set/                                                               # Chunk sets: a collection's chunks under one profile and one counter, building until complete or failed
│   │   │   │   ├── tests/                                                               # Tests of the chunk set records: their lifecycle and their chunks
│   │   │   │   │   ├── chunks.rs                                                        # Chunks: a revision's chunks recorded at once into a building set, their prepared inputs pinned, read in scope
│   │   │   │   │   ├── guards.rs                                                        # The guards of migration 0007_chunk_sets: each trigger refusing raw SQL, one test each
│   │   │   │   │   ├── latest.rs                                                        # The default set for publication is the latest complete set, ignoring
│   │   │   │   │   ├── lifecycle.rs                                                     # A chunk set's lifecycle: begun building, found again by a rerun, then complete or failed for good
│   │   │   │   │   ├── mod.rs                                                           # Tests of the chunk set records: their lifecycle and their chunks
│   │   │   │   │   ├── section_revisions.rs                                             # Scoped section lookups stop after two distinct document/revision pairs
│   │   │   │   │   └── support.rs                                                       # What the chunk set tests share: a scratch database with revisions of two collections
│   │   │   │   ├── chunk.rs                                                             # Chunks: a revision's passages in a chunk set, recorded at once, each pinning its prepared input
│   │   │   │   ├── error.rs                                                             # Why the kernel refused to begin, move or fill a chunk set
│   │   │   │   ├── mod.rs                                                               # Chunk sets: a collection's chunks under one profile and one counter, building until complete or failed
│   │   │   │   ├── record.rs                                                            # Chunk sets as the kernel records them: begun, completed with their manifest or failed, read in scope
│   │   │   │   └── state.rs                                                             # The states a chunk set moves through, and the moves it may make
│   │   │   ├── document/                                                                # The pipeline's document records (building block B5; docs/architecture/01
│   │   │   │   ├── tests/                                                               # Tests of the document records: the documents migration, collections
│   │   │   │   │   ├── counts.rs                                                        # A collection's counts: documents, revisions by status and disposition, only in scope
│   │   │   │   │   ├── dispositions.rs                                                  # Quality dispositions: one per revision, kept once given, read in scope, a hold journaled with it
│   │   │   │   │   ├── duplicates.rs                                                    # Occurrences and near-duplicate groups: each recorded once, a batch in one write, read in scope
│   │   │   │   │   ├── errors.rs                                                        # What the document records' refusals say, and the store's refusals they
│   │   │   │   │   ├── guards.rs                                                        # The guards of migration 0008_document_guards: each trigger refusing raw SQL
│   │   │   │   │   ├── listing.rs                                                       # The collections a set covers, listed in id order; a source's grant never lists its collection
│   │   │   │   │   ├── mod.rs                                                           # Tests of the document records: the documents migration, collections
│   │   │   │   │   ├── parents.rs                                                       # Collections, their sources and their documents: collections and sources
│   │   │   │   │   ├── revisions.rs                                                     # Revisions: recorded once with their two artifacts pinned, immutable after
│   │   │   │   │   ├── schema.rs                                                        # The documents migration: the ten pipeline tables of 01 §11, all strict
│   │   │   │   │   └── support.rs                                                       # What the record tests share: a scratch database, and the collection
│   │   │   │   ├── collection.rs                                                        # Collections, the sources they declare and the documents those sources
│   │   │   │   ├── counts.rs                                                            # A collection's counts: documents, and revisions by status and by disposition, in one snapshot
│   │   │   │   ├── disposition.rs                                                       # Quality dispositions: one per revision, kept once given, a hold journaled in the same write
│   │   │   │   ├── duplicate.rs                                                         # The duplicates of revisions: every place a revision's content occurs, and the groups of near duplicates
│   │   │   │   ├── error.rs                                                             # Why the kernel refused to record a collection, a source, a document or a revision
│   │   │   │   ├── mod.rs                                                               # The pipeline's document records (building block B5; docs/architecture/01
│   │   │   │   └── revision.rs                                                          # Revisions: one exact version of a document's bytes and metadata, recorded
│   │   │   ├── eval/                                                                    # Evaluation reports (plan D13; FR-S1-009): each run of an evaluation suite
│   │   │   │   ├── tests/                                                               # Tests of the evaluation reports the kernel records: their artifact, their
│   │   │   │   │   ├── mod.rs                                                           # Tests of the evaluation reports the kernel records: their artifact, their
│   │   │   │   │   ├── records.rs                                                       # A report is stored as an artifact, indexed and pinned by its record, and
│   │   │   │   │   ├── support.rs                                                       # What the report tests share: a scratch database holding the collections
│   │   │   │   │   └── table.rs                                                         # The table of reports refuses, whoever writes, to change, replace or
│   │   │   │   ├── error.rs                                                             # Why the kernel refused to record or read an evaluation report
│   │   │   │   ├── mod.rs                                                               # Evaluation reports (plan D13; FR-S1-009): each run of an evaluation suite
│   │   │   │   └── report.rs                                                            # Reports as the kernel records them: the artifact of each, the record that
│   │   │   ├── evidence/                                                                # Evidence (building block B7; docs/architecture/02 §6, plan D10): what a
│   │   │   │   ├── tests/                                                               # Tests of evidence: resolving a chunk from the authority, and bundles as
│   │   │   │   │   ├── bundle.rs                                                        # Bundles: maestro-evidence/1 as JSON, its evidence apart from its trace
│   │   │   │   │   ├── lookup.rs                                                        # Rust source: lookup
│   │   │   │   │   ├── mod.rs                                                           # Tests of evidence: resolving a chunk from the authority, and bundles as
│   │   │   │   │   ├── parent_context.rs                                                # Rust source: parent context
│   │   │   │   │   ├── resolve.rs                                                       # Resolving a chunk: the exact bytes its span covers in its revision's
│   │   │   │   │   ├── search_wire.rs                                                   # Backwards-readable search inventory and request-budget fields
│   │   │   │   │   └── support.rs                                                       # What the evidence tests share: a scratch database holding one revision of
│   │   │   │   ├── bundle.rs                                                            # Bundles: maestro-evidence/1, the search response contract, checked whole
│   │   │   │   ├── error.rs                                                             # Why the kernel refused to resolve a chunk
│   │   │   │   ├── inventory.rs                                                         # Exact, bounded S1 search inventories, separate from supporting passages
│   │   │   │   ├── lookup.rs                                                            # Scoped chunk membership in retained, readable generations
│   │   │   │   ├── mod.rs                                                               # Evidence (building block B7; docs/architecture/02 §6, plan D10): what a
│   │   │   │   ├── passage.rs                                                           # The passages a bundle cites: the source text of a span of one revision
│   │   │   │   ├── request_budget.rs                                                    # The transport-safe echo of the accepted search budget
│   │   │   │   └── resolve.rs                                                           # Resolving a chunk: the exact source text its span covers, read from the
│   │   │   ├── gateway/                                                                 # The model gateway (building block B10): every model, embedder, reranker
│   │   │   │   ├── card_v2/                                                             # Strict v2 model-card identity encoding and validation
│   │   │   │   │   ├── mod.rs                                                           # Strict v2 model-card identity encoding and validation
│   │   │   │   │   ├── paths.rs                                                         # Local-path recognition shared by card identity validation
│   │   │   │   │   ├── runtime.rs                                                       # Canonical runtime-flag checks for values also held in typed identity fields
│   │   │   │   │   ├── types.rs                                                         # Strict typed identity recorded by maestro-model-card/2
│   │   │   │   │   └── validation.rs                                                    # Semantic validation and legacy gateway projection for v2 identities
│   │   │   │   ├── tests/                                                               # Tests of the model gateway: model cards, the router client against a stub
│   │   │   │   │   ├── card.rs                                                          # Tests of model cards: strict JSON artifacts whose digest is their
│   │   │   │   │   ├── catalog.rs                                                       # The router's catalog: GET /v1/models in no room; refusals kept, a bad entry name an invalid answer
│   │   │   │   │   ├── chat.rs                                                          # Chat sends one bounded, non-streaming prompt and refuses unusable replies
│   │   │   │   │   ├── fake.rs                                                          # Tests of the deterministic fake: its outputs are fixed by its inputs, the
│   │   │   │   │   ├── fixture.rs                                                       # What the gateway's tests share: a scratch store, a card for each role, and
│   │   │   │   │   ├── formatting.rs                                                    # Model-card document and query formatting
│   │   │   │   │   ├── mod.rs                                                           # Tests of the model gateway: model cards, the router client against a stub
│   │   │   │   │   ├── port.rs                                                          # Tests of the port's refusals: each says what was refused and why
│   │   │   │   │   ├── reply_cap.rs                                                     # The reply cap of a chat request: the smallest of the caller's cap, the
│   │   │   │   │   ├── router.rs                                                        # Tests of the router client against a stub router: every call is bound to
│   │   │   │   │   ├── safety.rs                                                        # The router client against a misbehaving router: it never follows a
│   │   │   │   │   ├── stub.rs                                                          # A stub of the model router: a loopback HTTP server, on a thread of its
│   │   │   │   │   ├── v2.rs                                                            # V2 model-card identity contract
│   │   │   │   │   ├── v2_golden.rs                                                     # Canonical model-card JSON fixtures with pinned wire bytes
│   │   │   │   │   ├── v2_validation.rs                                                 # V2 card schema, weight, and runtime flag validation tests
│   │   │   │   │   ├── v2_validation_more.rs                                            # V2 card resource, provenance, role, sampling, and canonical-load validation tests
│   │   │   │   │   └── v2_validation_paths.rs                                           # Machine-path checks for v2 provenance fields
│   │   │   │   ├── body.rs                                                              # Bounded answers: each call reads at most what its endpoint can
│   │   │   │   ├── card.rs                                                              # Model cards: what was evaluated of a model filling a role (D8), kept as
│   │   │   │   ├── card_types.rs                                                        # Shared model-card vocabulary, independent of v1 and v2 encodings
│   │   │   │   ├── fake.rs                                                              # The deterministic fake behind the model port, which public CI uses since it
│   │   │   │   ├── mod.rs                                                               # The model gateway (building block B10): every model, embedder, reranker
│   │   │   │   ├── port.rs                                                              # The model port: the calls every way of reaching a model answers, each
│   │   │   │   └── router.rs                                                            # The router client: the model port over maestro-model-router's dedicated
│   │   │   ├── generation/                                                              # The search generations of a collection (building block B6; plan D9): each
│   │   │   │   ├── tests/                                                               # Tests of the generation records: their lifecycle and their publication
│   │   │   │   │   ├── lifecycle.rs                                                     # A generation's lifecycle: created building, then verified, published and
│   │   │   │   │   ├── listing.rs                                                       # A collection's generations: all of them, in creation order, only in scope
│   │   │   │   │   ├── mod.rs                                                           # Tests of the generation records: their lifecycle and their publication
│   │   │   │   │   ├── publication.rs                                                   # Publication: at most one generation of a collection is published, which
│   │   │   │   │   ├── publication_events.rs                                            # Publication events are committed with each generation lifecycle change
│   │   │   │   │   └── support.rs                                                       # What the generation tests share: a scratch database holding two
│   │   │   │   ├── error.rs                                                             # Why the kernel refused to create or move a generation
│   │   │   │   ├── lifecycle.rs                                                         # Generations as the kernel records them, and the calls that create and move
│   │   │   │   ├── mod.rs                                                               # The search generations of a collection (building block B6; plan D9): each
│   │   │   │   └── state.rs                                                             # The states a generation moves through, and the one move each allows
│   │   │   ├── job/                                                                     # Jobs: long work, such as an import, a preparation or a publication, run
│   │   │   │   ├── tests/                                                               # Tests of jobs: their keys and attempts, their leases and states, their
│   │   │   │   │   ├── changes.rs                                                       # Changes: each change of a job recorded on its stream of the journal, with
│   │   │   │   │   ├── child.rs                                                         # Not a test of its own: the first process of the resume test, which works
│   │   │   │   │   ├── errors.rs                                                        # Refusals: what each one says, and a stored job the kernel cannot read
│   │   │   │   │   ├── leases.rs                                                        # Leases: one holder at a time, taken over once expired, renewed by
│   │   │   │   │   ├── mod.rs                                                           # Tests of jobs: their keys and attempts, their leases and states, their
│   │   │   │   │   ├── progress.rs                                                      # Progress: recorded on the job's stream of the journal in the write that
│   │   │   │   │   ├── resources.rs                                                     # Resources: what a job holds exclusively while it is queued or running
│   │   │   │   │   ├── resume.rs                                                        # A job interrupted mid-way: its first process dies, and a second process
│   │   │   │   │   ├── scopes.rs                                                        # Scopes: a job is read only through a set that covers the scope it works
│   │   │   │   │   ├── states.rs                                                        # States: a job moves only forward, and its three outcomes are final
│   │   │   │   │   ├── submit.rs                                                        # Submitting a job: its ID, its idempotency key, and the job a retried
│   │   │   │   │   ├── support.rs                                                       # What the job tests share: a scratch data directory, the publication they
│   │   │   │   │   └── table.rs                                                         # The jobs table: what the database itself refuses, whoever writes, so no
│   │   │   │   ├── error.rs                                                             # Why the kernel refused to submit, lease, move or read a job
│   │   │   │   ├── events.rs                                                            # The events of a job: each change and each step, recorded on the job's
│   │   │   │   ├── lease.rs                                                             # Leases: taken by one holder at a time, taken over once expired, and
│   │   │   │   ├── mod.rs                                                               # Jobs: long work, such as an import, a preparation or a publication, run
│   │   │   │   ├── outcome.rs                                                           # Outcomes: a job ends succeeded, failed or cancelled, and stays so
│   │   │   │   ├── progress.rs                                                          # Progress: each step of a job recorded on its stream of the journal, in
│   │   │   │   ├── record.rs                                                            # Jobs as the kernel records them, their leases, and the calls that submit
│   │   │   │   └── state.rs                                                             # The states a job moves through, and the moves it may make
│   │   │   ├── journal/                                                                 # The journal: every change the kernel makes, recorded as an event in one
│   │   │   │   ├── tests/                                                               # Tests of the journal: its events and their streams, its cursors, and what
│   │   │   │   │   ├── append_only.rs                                                   # The journal is append-only: the database itself refuses to update, delete
│   │   │   │   │   ├── breaking.rs                                                      # The compatibility check: what a fresh event schema changes of the committed one, beyond an added optional property
│   │   │   │   │   ├── child.rs                                                         # Not a test of its own: what the child processes of the crash and
│   │   │   │   │   ├── compatibility.rs                                                 # The compatibility check's rules on small schemas: what it refuses as a breaking change, what it lets through
│   │   │   │   │   ├── concurrency.rs                                                   # Writers together: threads sharing the database and processes of their own
│   │   │   │   │   ├── crash.rs                                                         # A crash: the event a process committed before it is read after the
│   │   │   │   │   ├── cursors.rs                                                       # Cursors: where each consumer stands in each stream, moved forward only
│   │   │   │   │   ├── envelope.rs                                                      # The envelope: an event read back from the journal as its CloudEvents 1.0 envelope
│   │   │   │   │   ├── events.rs                                                        # Events: the ID and the sequence recording gives them, the event it
│   │   │   │   │   ├── mod.rs                                                           # Tests of the journal: its events and their streams, its cursors, and what
│   │   │   │   │   ├── predecessor.rs                                                   # The public event schemas may add optional properties within a major, but
│   │   │   │   │   ├── regeneration.rs                                                  # The command that regenerates the committed event schemas and their index, tested on a scratch directory
│   │   │   │   │   ├── schemas.rs                                                       # The committed event schemas: generated from their types, never narrowed, followed by the data, and the command that regenerates them
│   │   │   │   │   ├── subset.rs                                                        # What the check and the validator read of a schema: the subset of JSON Schema schemars generates
│   │   │   │   │   ├── support.rs                                                       # What the journal tests share: a scratch data directory, the events they
│   │   │   │   │   └── validation.rs                                                    # A validator of data against the committed event schemas, over the subset schemars generates
│   │   │   │   ├── cursor.rs                                                            # Cursors: how far each consumer has read each stream, moved forward only
│   │   │   │   ├── envelope.rs                                                          # Events as they leave the kernel: the CloudEvents 1.0 envelope, and the machine its source names
│   │   │   │   ├── error.rs                                                             # Why the journal refused an operation
│   │   │   │   ├── event.rs                                                             # Events: recorded with a new ID and the next sequence of their stream
│   │   │   │   ├── knowledge.rs                                                         # The public knowledge events: the data of each, and the catalogue naming the schema each follows
│   │   │   │   └── mod.rs                                                               # The journal: every change the kernel makes, recorded as an event in one
│   │   │   ├── model/                                                                   # Scoped model-card registry: cards, evaluations and selections
│   │   │   │   ├── tests/                                                               # Contracts for registry identity, writes, reads, scopes and SQL guards
│   │   │   │   │   ├── concurrency.rs                                                   # Concurrent model registry writes stay idempotent
│   │   │   │   │   ├── errors.rs                                                        # Model-registry error text and source chaining
│   │   │   │   │   ├── guards.rs                                                        # Model registry write preconditions, eligibility, and transaction rollback tests
│   │   │   │   │   ├── history.rs                                                       # Exact evaluation-selection joins, insertion ordering, and selection history
│   │   │   │   │   ├── journal.rs                                                       # Transactional model-registry journal event contents and rollback
│   │   │   │   │   ├── live_registration.rs                                             # Safe fake-backed registration checks and the explicitly ignored live card registration
│   │   │   │   │   ├── live_selection.rs                                                # The ignored live reranker selection and the checks of its helper
│   │   │   │   │   ├── mod.rs                                                           # Model registry tests for identities, persistence, eligibility, scopes, and guards
│   │   │   │   │   ├── reads.rs                                                         # Integrity checks for scoped model registry reads
│   │   │   │   │   ├── records.rs                                                       # Model card, evaluation, and selection record/read contract tests
│   │   │   │   │   ├── rung_selection.rs                                                # Reranker selection from a ladder rung's report and its manifest
│   │   │   │   │   ├── scopes.rs                                                        # Scope-bound visibility tests for model cards, evaluations, and selections
│   │   │   │   │   ├── support.rs                                                       # Scratch databases and v2 card fixtures for model persistence tests
│   │   │   │   │   ├── triggers.rs                                                      # Direct tests for immutable model-registry row guards
│   │   │   │   │   └── writes.rs                                                        # Transactional model-registry write preconditions and rollback tests
│   │   │   │   ├── error.rs                                                             # Why the kernel refused a model-card, evaluation, or selection operation
│   │   │   │   ├── mod.rs                                                               # Scoped model-card, evaluation and selection persistence
│   │   │   │   ├── read.rs                                                              # Scoped model registry readers with artifact and row-metadata verification
│   │   │   │   ├── records.rs                                                           # Public metadata for immutable model registrations, evaluations and selections
│   │   │   │   ├── registration.rs                                                      # Importing and registering a strict v2 model card with its pinned evidence
│   │   │   │   └── write.rs                                                             # Scoped transactional model registry writers
│   │   │   ├── representation/                                                          # Immutable representation manifests and one-time generation binding
│   │   │   │   ├── error.rs                                                             # Representation-set boundary errors
│   │   │   │   ├── mod.rs                                                               # Immutable representation manifests and one-time generation binding
│   │   │   │   ├── read.rs                                                              # Scoped representation metadata and individual revision-shard reads
│   │   │   │   ├── record.rs                                                            # Representation set lifecycle, revision shards and generation binding
│   │   │   │   ├── types.rs                                                             # Immutable revision shards and representation-set metadata
│   │   │   │   └── validation.rs                                                        # Canonical shard validation and relational set completion
│   │   │   ├── retrieval/                                                               # Controlled, scope-bound retrieval and its shared literal search rules
│   │   │   │   ├── tests/                                                               # Tests for exact identifier search inputs
│   │   │   │   │   ├── clock.rs                                                         # A settable clock shared by controlled-read tests
│   │   │   │   │   ├── deadlines.rs                                                     # Cancellation and real-clock bounds on SQLite reads
│   │   │   │   │   ├── errors.rs                                                        # Nonblank retrieval refusals and preserved store error sources
│   │   │   │   │   ├── identifier_scope.rs                                              # Rust source: identifier scope
│   │   │   │   │   ├── identifiers.rs                                                   # Literal identifier boundaries and shared whitespace normalization
│   │   │   │   │   ├── inventory.rs                                                     # Complete, pinned-generation inventory reads
│   │   │   │   │   ├── mod.rs                                                           # Tests for exact identifier search inputs
│   │   │   │   │   ├── projection_storage.rs                                            # Tests for search-member, readiness and generation projection storage
│   │   │   │   │   ├── storage.rs                                                       # Atomic derived-input, member and readiness writes
│   │   │   │   │   ├── support.rs                                                       # Shared scratch records for controlled search-reader tests
│   │   │   │   │   └── versions.rs                                                      # Exact version existence in a pinned, scoped generation
│   │   │   │   ├── error.rs                                                             # Errors from exact, scoped search storage and reads
│   │   │   │   ├── identifiers.rs                                                       # Shared literal identifier matching and query-whitespace normalization
│   │   │   │   ├── inventory.rs                                                         # Exact, bounded inventories over a pinned generation's own member revisions
│   │   │   │   ├── mod.rs                                                               # Controlled, scope-bound retrieval and its shared literal search rules
│   │   │   │   ├── read.rs                                                              # Deadline-controlled SQLite readers and scoped chunk-batch loading
│   │   │   │   ├── types.rs                                                             # Public inputs and results of kernel-owned search operations
│   │   │   │   └── write.rs                                                             # Atomic writes of exact search derivatives and readiness markers
│   │   │   ├── scope/                                                                   # Scopes and grants: who may see what (docs/architecture/04 §3, building
│   │   │   │   ├── tests/                                                               # Tests of scopes: their paths and names, what a grant covers, the grants
│   │   │   │   │   ├── config.rs                                                        # config.toml: the local principal's grants, checked whole when read, and
│   │   │   │   │   ├── grants.rs                                                        # Grants: a principal sees only what it was granted and what lies below it
│   │   │   │   │   ├── inventory.rs                                                     # There is no read function without a ScopeSet: every public method of
│   │   │   │   │   ├── known.rs                                                         # The scopes a set was granted, and the known scopes it covers: the workspace, collections and sources
│   │   │   │   │   ├── mod.rs                                                           # Tests of scopes: their paths and names, what a grant covers, the grants
│   │   │   │   │   ├── paths.rs                                                         # Scope paths: a workspace, then a collection, then a source, each named by
│   │   │   │   │   ├── readers.rs                                                       # Readers of scoped data take the caller's ScopeSet and filter inside
│   │   │   │   │   ├── records.rs                                                       # The readers of the pipeline's records take the caller's ScopeSet and
│   │   │   │   │   └── support.rs                                                       # What the scope tests share: a scratch directory for the kernel's data and
│   │   │   │   ├── config.rs                                                            # config.toml, the kernel's configuration file in its configuration
│   │   │   │   ├── grant.rs                                                             # Grants: the rights of principals on scopes, each given or taken back in a
│   │   │   │   ├── known.rs                                                             # The scopes the kernel knows, as a set sees them: its workspace and each recorded collection and source
│   │   │   │   ├── mod.rs                                                               # Scopes and grants: who may see what (docs/architecture/04 §3, building
│   │   │   │   ├── path.rs                                                              # Scope paths: a workspace, then optionally a collection, then optionally a
│   │   │   │   ├── right.rs                                                             # The rights a grant gives on a scope
│   │   │   │   └── set.rs                                                               # The scopes a principal may read, found for one request, and the condition
│   │   │   ├── settings/                                                                # The journal of settings changes: every maestro config set and config
│   │   │   │   ├── change.rs                                                            # Recording a setting's change and reading a principal's changes back
│   │   │   │   ├── mod.rs                                                               # The journal of settings changes: every maestro config set and config
│   │   │   │   └── tests.rs                                                             # The journal of settings changes: each change recorded with its time and
│   │   │   ├── store/                                                                   # The kernel's database: one SQLite file beside the artifact store, holding
│   │   │   │   ├── tests/                                                               # Tests of the kernel database: its migrations, its connections, the
│   │   │   │   │   ├── artifacts.rs                                                     # Artifacts: stored, recorded with their size, media type and pins, and read
│   │   │   │   │   ├── checks.rs                                                        # SQLite's quick check, damage reported as found; each recorded artifact present and intact, or named
│   │   │   │   │   ├── connections.rs                                                   # Connections: one writer shared by every thread, readers of their own, the
│   │   │   │   │   ├── garbage.rs                                                       # Garbage collection: it lists before it removes, removes only artifacts
│   │   │   │   │   ├── graph_migrations.rs                                              # Graph migrations upgrade populated legacy stores and roll back failed DDL
│   │   │   │   │   ├── migrations.rs                                                    # Migrations: applied in number order, each once, recorded by name, and a
│   │   │   │   │   ├── mod.rs                                                           # Tests of the kernel database: its migrations, its connections, the
│   │   │   │   │   └── support.rs                                                       # What the database tests share: scratch directories, the digests of their
│   │   │   │   ├── artifacts.rs                                                         # The artifacts table: what the artifact store holds, the pins that keep
│   │   │   │   ├── database.rs                                                          # The database: its file, one writer connection behind a mutex, and readers
│   │   │   │   ├── error.rs                                                             # Why the kernel's database refused an operation
│   │   │   │   ├── migration.rs                                                         # The migrations: the SQL files of migrations/, embedded in the binary
│   │   │   │   └── mod.rs                                                               # The kernel's database: one SQLite file beside the artifact store, holding
│   │   │   ├── telemetry/                                                               # Telemetry: pinned span names and component health (building block B11)
│   │   │   │   ├── tests/                                                               # Tests of telemetry: component health, and the stages and their spans
│   │   │   │   │   ├── health.rs                                                        # Tests of component health
│   │   │   │   │   ├── mod.rs                                                           # Tests of telemetry: component health, and the stages and their spans
│   │   │   │   │   └── stages.rs                                                        # Tests of the stages: the spans they open under their pinned names, the
│   │   │   │   ├── health.rs                                                            # Health: how each component is doing, asked of its own check
│   │   │   │   ├── mod.rs                                                               # Telemetry: pinned span names and component health (building block B11)
│   │   │   │   ├── span.rs                                                              # The spans the kernel opens, and the names they carry, pinned in one place
│   │   │   │   └── stage.rs                                                             # One traced stage of a knowledge operation: its span, the outcome it ends
│   │   │   ├── unit_graph/                                                              # Digest-bound delivery graphs and source-scoped persistence
│   │   │   │   ├── tests/                                                               # Graph persistence and source authorization regressions
│   │   │   │   │   ├── mod.rs                                                           # Graph persistence and source authorization regressions
│   │   │   │   │   ├── records.rs                                                       # Record/read atomicity and hidden-ancestry refusal
│   │   │   │   │   ├── representations.rs                                               # Revision shards complete frozen representation sets and bind once
│   │   │   │   │   ├── scale.rs                                                         # Synthetic graph size and indexed candidate lookup, without row explosion
│   │   │   │   │   ├── support.rs                                                       # Synthetic source, graph and kernel records shared by persistence tests
│   │   │   │   │   └── validation.rs                                                    # Direct invariants of the current delivery-graph contract
│   │   │   │   ├── coverage.rs                                                          # Exact ownership and source accounting, with structural references by ID
│   │   │   │   ├── error.rs                                                             # Refusals at the graph wire and persistence boundary
│   │   │   │   ├── group_parts.rs                                                       # Canonical graph array and node part ordering
│   │   │   │   ├── hierarchy.rs                                                         # Structural ancestry and context derivation without transitive storage
│   │   │   │   ├── mapping.rs                                                           # Source accounting and explicitly typed context dependencies
│   │   │   │   ├── membership.rs                                                        # Retrieval membership identity, ordering and content coverage
│   │   │   │   ├── mod.rs                                                               # Digest-bound delivery graphs and source-scoped persistence
│   │   │   │   ├── read.rs                                                              # One authorized candidate revision maps to one immutable graph artifact
│   │   │   │   ├── record.rs                                                            # Atomic graph membership and artifact pins, after existing chunk recording
│   │   │   │   ├── serialization.rs                                                     # Canonical JSON serialization: typed field order, exact bytes, no extra keys
│   │   │   │   ├── types.rs                                                             # Kernel-owned delivery graph wire values, independent of canonicalization
│   │   │   │   └── validation.rs                                                        # Validate the immutable graph before it crosses the persistence boundary
│   │   │   ├── binding.rs                                                               # Named bindings: the local paths that the logical names of committed files
│   │   │   ├── filesystem.rs                                                            # The files and directories the kernel creates: its owner's only, and each
│   │   │   ├── lib.rs                                                                   # The kernel of Maestro: the single authoritative store every later
│   │   │   └── paths.rs                                                                 # Where the kernel keeps its data: $XDG_DATA_HOME/maestro when that names an
│   │   ├── tests/                                                                       # Integration tests
│   │   │   ├── fixtures/                                                                # Test fixtures
│   │   │   │   ├── unit-graph-v1.json                                                   # JSON data: unit graph v1
│   │   │   │   ├── unit-graph-v1.txt                                                    # Text: unit graph v1
│   │   │   │   └── unit-mapping-v1.json                                                 # JSON data: unit mapping v1
│   │   │   └── it/                                                                      # It
│   │   │       ├── main.rs                                                              # Kernel integration tests for the unit-graph wire contract and producer conformance
│   │   │       ├── n04_frontier_ack.rs                                                  # N04 acknowledgement integrity, atomic journal writes and immutable source binding
│   │   │       ├── n04_frontier_support.rs                                              # Synthetic fixtures for the N04 replaceable frontier contract
│   │   │       ├── n04_persist_frontier_leases_and_fenced_submissions.rs                # N04 frontier contract: durable work, exclusive leases and fenced acknowledgement
│   │   │       ├── n06_receipt_review_regressions.rs                                    # N06 review regressions for terminal reconciliation, bounds and privacy
│   │   │       ├── n06_store_scoped_receipts_and_content_free_progress_events.rs        # N06: opaque receipt views, durable attempts, and content-free event sinks
│   │   │       ├── n30_filesystem.rs                                                    # Protected local-overlay and durable replacement contracts
│   │   │       ├── unit_graph_code_leadin.rs                                            # The kernel accepts code groups without an optional lead-in relation
│   │   │       ├── unit_graph_nested_producer.rs                                        # Kernel conformance for nested procedure, code and table producer graphs
│   │   │       ├── unit_graph_producer.rs                                               # Kernel conformance against the canonicalization producer snapshot
│   │   │       └── unit_graph_wire.rs                                                   # The shared B06/B07 wire fixture is a canonical, validated CAS payload
│   │   └── Cargo.toml                                                                   # Crate manifest: The single authoritative store of Maestro, starting with its content-addressed artifacts
│   ├── maestro-knowledge/                                                               # Maestro knowledge
│   │   ├── src/                                                                         # The crate's sources
│   │   │   ├── answer/                                                                  # Evidence-grounded answering with bounded generation and one validation retry
│   │   │   │   ├── tests/                                                               # Integration tests
│   │   │   │   │   ├── delivered.rs                                                     # Every answer and refusal keeps, for evaluation only, the anchors of the
│   │   │   │   │   ├── explain.rs                                                       # Rust source: explain
│   │   │   │   │   ├── guardrails.rs                                                    # Rust source: guardrails
│   │   │   │   │   ├── parent_context.rs                                                # Rust source: parent context
│   │   │   │   │   ├── presentation.rs                                                  # A presented prompt: the session's language and tone
│   │   │   │   │   ├── prompt_text.rs                                                   # A prompt a ladder rung supplies as text: its {data} slot holds the
│   │   │   │   │   ├── prompts.rs                                                       # The prompt versions: v1 stays today's text, v2 asks for every supporting
│   │   │   │   │   ├── reply_cap.rs                                                     # Rust source: reply cap
│   │   │   │   │   ├── requests.rs                                                      # Rust source: requests
│   │   │   │   │   ├── router_refusal.rs                                                # A refusal the router client cannot read in full, through the answer path
│   │   │   │   │   └── threshold.rs                                                     # The reranker relevance threshold: below it, ask refuses without chat
│   │   │   │   ├── validate/                                                            # Deterministic support checks for buffered answerer replies
│   │   │   │   │   ├── edge_tests.rs                                                    # Rust source: edge tests
│   │   │   │   │   └── tests.rs                                                         # Rust source: tests
│   │   │   │   ├── generate.rs                                                          # The bounded search-to-answer state machine and refusal handling
│   │   │   │   ├── presentation.rs                                                      # How an answer speaks: the session's language and tone (the owner's
│   │   │   │   ├── prompt.rs                                                            # The evidence-only prompt and registered chat-template controls
│   │   │   │   ├── tests.rs                                                             # Rust source: tests
│   │   │   │   ├── types.rs                                                             # Typed ask requests, results, refusals, and trusted dependencies
│   │   │   │   └── validate.rs                                                          # Deterministic support checks for buffered answerer replies
│   │   │   ├── eval/                                                                    # The evaluation runner (plan D13; FR-S1-009, SC-S1-008): every retrieval
│   │   │   │   ├── reports/                                                             # Reports
│   │   │   │   │   ├── base.rs                                                          # Reports: maestro-eval-report/1, what a run measured, question by
│   │   │   │   │   ├── metrics.rs                                                       # Summary metrics shared by the v1 and v2 report contracts
│   │   │   │   │   ├── mod.rs                                                           # Strict versioned report contracts and their wire types
│   │   │   │   │   └── v2.rs                                                            # V2 report metadata and cohort/subgroup summaries
│   │   │   │   ├── tests/                                                               # Tests of the evaluation runner: how it ranks and judges each question
│   │   │   │   │   ├── compare.rs                                                       # compare pairs two runs by question and gives, for each metric, the
│   │   │   │   │   ├── degraded.rs                                                      # A degraded search, one where a route or the reranker could not run, still
│   │   │   │   │   ├── delivered.rs                                                     # Tests of the delivery score: which anchors deliver a section, how much of
│   │   │   │   │   ├── documents.rs                                                     # A question may expect a document without sections whole: any passage of
│   │   │   │   │   ├── failures.rs                                                      # Each failure of an answerable question gets its class at each cut-off it
│   │   │   │   │   ├── groups.rs                                                        # Expected section copies share one nDCG item while remaining visible in
│   │   │   │   │   ├── intervals.rs                                                     # The intervals: 95 % percentile intervals of 2,000 bootstrap resamples
│   │   │   │   │   ├── ladder.rs                                                        # Tests of the ladder's floors: each count at its exact boundary, the
│   │   │   │   │   ├── ladder_languages.rs                                              # The ladder's counts per language: French and English questions counted
│   │   │   │   │   ├── ladder_resolve.rs                                                # resolve_expected gives the ladder's scorer each question's expected
│   │   │   │   │   ├── ladder_rows.rs                                                   # Tests of what the ladder's rows may not hide: answerable questions left
│   │   │   │   │   ├── ladder_spans.rs                                                  # Citation locality against the expected section's pinned source extent
│   │   │   │   │   ├── metrics.rs                                                       # Each metric of a run against values computed by hand on a small suite
│   │   │   │   │   ├── mod.rs                                                           # Tests of the evaluation runner: how it ranks and judges each question
│   │   │   │   │   ├── ranking.rs                                                       # A bundle lists its passages in reading order, so the runner ranks them
│   │   │   │   │   ├── report.rs                                                        # A report is a strict JSON artifact, maestro-eval-report/1: it writes and
│   │   │   │   │   ├── reranker_health.rs                                               # The reranker health gate admits only finite, separated public-pair scores
│   │   │   │   │   ├── run.rs                                                           # run resolves each expected section of a suite in the canonical document
│   │   │   │   │   ├── support.rs                                                       # What the evaluation tests share: questions, bundles built from the hits a
│   │   │   │   │   ├── v2.rs                                                            # Report v2 retains failed attempts and their timing cohort
│   │   │   │   │   ├── v2_comparison.rs                                                 # Report v2 paired comparison rules
│   │   │   │   │   ├── v2_merge_validation.rs                                           # Rust source: v2 merge validation
│   │   │   │   │   ├── v2_scoring.rs                                                    # Hand-checked v2 ranking, subgroup and repeated-question metrics
│   │   │   │   │   ├── v2_statistics.rs                                                 # Report v2 schedule, subgroup, and timing statistics
│   │   │   │   │   └── v2_validation.rs                                                 # Rust source: v2 validation
│   │   │   │   ├── bootstrap.rs                                                         # The bootstrap: resamples of a run's questions, drawn within the
│   │   │   │   ├── compare.rs                                                           # Comparing two runs of one suite, question by question
│   │   │   │   ├── delivered.rs                                                         # The ladder's delivery score: whether the sections a question accepts
│   │   │   │   ├── error.rs                                                             # Why a run or a comparison was refused
│   │   │   │   ├── judge.rs                                                             # Judging one question: ranking its bundle's passages, finding the sections
│   │   │   │   ├── ladder.rs                                                            # The ladder's scorer (T037): the M1 floors of one configuration, each PASS
│   │   │   │   ├── ladder_markdown.rs                                                   # The ladder's score as a Markdown table
│   │   │   │   ├── ladder_score.rs                                                      # The ladder's score as a configuration reports it: its floors with counts
│   │   │   │   ├── merge.rs                                                             # Merging v2 attempt reports without dropping scheduled work
│   │   │   │   ├── metric.rs                                                            # The metrics of a run: each a value over a sample of its questions, drawn
│   │   │   │   ├── mod.rs                                                               # The evaluation runner (plan D13; FR-S1-009, SC-S1-008): every retrieval
│   │   │   │   ├── record.rs                                                            # Recording a report in the kernel, under the collection, generation and suite it names
│   │   │   │   ├── report_validation.rs                                                 # V1 and v2 report-reader rules, shared with the v2 writer
│   │   │   │   ├── reranker_health.rs                                                   # Health qualification for rerankers, with public paired examples and a card-bound receipt
│   │   │   │   ├── run.rs                                                               # A run: every question of a suite, resolved in the generation it
│   │   │   │   └── run_v2.rs                                                            # Strict v2 attempt execution and aggregation
│   │   │   ├── import/                                                                  # Importing a collection's corpus through its maestro-corpus/1 manifests
│   │   │   │   ├── tests/                                                               # Tests of the import that reach inside it: its manifest lines, and its streaming, proven by an in-memory corpus
│   │   │   │   │   ├── lines.rs                                                         # A manifest read one numbered line at a time, until it ends
│   │   │   │   │   ├── mod.rs                                                           # Tests of the import that reach inside it: its manifest lines, and its streaming, proven by an in-memory corpus
│   │   │   │   │   └── streaming.rs                                                     # The import holds one manifest line and one document at a time
│   │   │   │   ├── collection.rs                                                        # Importing a collection: its declaration recorded, each source's manifest imported, its completion journaled
│   │   │   │   ├── corpus.rs                                                            # Where an import reads a source's corpus: its manifest, and each document relative to it
│   │   │   │   ├── entry.rs                                                             # Importing one entry: its document checked, canonicalized, stored and recorded, or held
│   │   │   │   ├── error.rs                                                             # Why an import stopped, before any work or part way
│   │   │   │   ├── mod.rs                                                               # Importing a collection's corpus through its maestro-corpus/1 manifests
│   │   │   │   ├── report.rs                                                            # What an import reports: its counts, and why it refused each entry it refused
│   │   │   │   └── source.rs                                                            # Importing one source's manifest: shared source_refs found first, then each line in turn
│   │   │   ├── index/                                                                   # The search projection in Qdrant: generations of chunk sets, behind the collection's alias
│   │   │   │   ├── tests/                                                               # Tests of the projection's parts no publication reaches
│   │   │   │   │   ├── dense.rs                                                         # The embedder's deadline: a port that never answers is dropped
│   │   │   │   │   ├── errors.rs                                                        # What each refusal of a publication says, and the cause each keeps
│   │   │   │   │   ├── lengths.rs                                                       # The lengths the sparse vectors are weighed against, with no passage or no term
│   │   │   │   │   ├── mod.rs                                                           # Tests of the projection's parts no publication reaches
│   │   │   │   │   ├── point.rs                                                         # Point IDs: the UUIDv5 of the chunk's ID, pinned by Python's uuid
│   │   │   │   │   ├── qdrant.rs                                                        # A Qdrant out of reach: the client's error is the cause of the refusal
│   │   │   │   │   ├── qdrant_adapter.rs                                                # Unit tests of the Qdrant adapter's hit conversion
│   │   │   │   │   └── verify.rs                                                        # Tests for the published vector-layout diagnostic
│   │   │   │   ├── batches.rs                                                           # Writing a generation's points a batch at a time, each shown to the caller to journal
│   │   │   │   ├── dense.rs                                                             # Dense vectors: a batch embedded in free room within a deadline, then checked
│   │   │   │   ├── error.rs                                                             # Why a publication stopped, and the check a generation's collection failed
│   │   │   │   ├── mod.rs                                                               # The search projection in Qdrant: generations of chunk sets, behind the collection's alias
│   │   │   │   ├── names.rs                                                             # Qdrant names derived from a generation's kernel record
│   │   │   │   ├── point.rs                                                             # Points: each chunk under the UUIDv5 of its ID, with its two vectors and its payload
│   │   │   │   ├── progress.rs                                                          # A publication's progress after each batch, which a job journals, and its report
│   │   │   │   ├── projection.rs                                                        # What a publication works with
│   │   │   │   ├── projection_port.rs                                                   # Backend-neutral operations and values for a generation's retrieval projection
│   │   │   │   ├── provenance.rs                                                        # What a chunk's point carries of its revision: version, source kind, scope tags, sections
│   │   │   │   ├── publication_names.rs                                                 # Stable backend collection names and publication reports
│   │   │   │   ├── publish.rs                                                           # Publishing a chunk set as a generation: built or resumed, checked, then behind the alias
│   │   │   │   ├── qdrant.rs                                                            # Qdrant through its official Rust client, qdrant-client 1.19, over gRPC
│   │   │   │   ├── qdrant_adapter.rs                                                    # Conversion between backend-neutral projection operations and Qdrant's transport types
│   │   │   │   ├── rebuild.rs                                                           # A fresh, guarded replacement for a published generation whose projection
│   │   │   │   ├── rebuild_state.rs                                                     # Snapshot types for one guarded generation replacement
│   │   │   │   ├── rebuild_validation.rs                                                # Validation of the frozen identity for a guarded projection rebuild
│   │   │   │   ├── search_inputs.rs                                                     # The kernel-owned search derivatives of one published generation
│   │   │   │   ├── sparse.rs                                                            # Sparse vectors: BM25 weights against the chunk set's average passage length
│   │   │   │   └── verify.rs                                                            # The structural check of a generation's collection before its alias moves
│   │   │   ├── lexical/                                                                 # The lexical analyzer of the BM25 route, profile bm25-en-fr/1: it turns a
│   │   │   │   ├── analyzer.rs                                                          # The profile's name and the terms of a text
│   │   │   │   ├── fold.rs                                                              # Folding: a text without its accents, before any other rule reads it
│   │   │   │   ├── mod.rs                                                               # The lexical analyzer of the BM25 route, profile bm25-en-fr/1: it turns a
│   │   │   │   ├── split.rs                                                             # Splitting: the identifiers and words of a folded text, and the parts of
│   │   │   │   ├── stem.rs                                                              # Stemming: light suffix rules on folded, lowercased words, French and
│   │   │   │   ├── stopwords.rs                                                         # The stopwords of bm25-en-fr/1: an English list and a French one, each of
│   │   │   │   ├── tests.rs                                                             # What the lexical module's lookups rely on
│   │   │   │   └── vector.rs                                                            # Sparse vectors: a passage's terms weighed with BM25's term-frequency part
│   │   │   ├── prepare/                                                                 # Preparing revisions for search: duplicates grouped, then chunks counted as the selected embedder counts them, through the model router
│   │   │   │   ├── tests/                                                               # Tests of the router tokenizer, and of the preparation of a collection over it
│   │   │   │   │   ├── chunk_sets.rs                                                    # What a chunk set records: its profile, its counter, the same chunk IDs for the same input, each prepared input pinned
│   │   │   │   │   ├── counting.rs                                                      # Counting and verifying through a qualified tokenizer: the port's IDs in
│   │   │   │   │   ├── duplicates.rs                                                    # Duplicates: exact ones prepared once with every occurrence kept, near ones grouped with their Jaccard
│   │   │   │   │   ├── eligibility.rs                                                   # Which revisions a preparation reads: the accepted ones, for a caller who reads the whole collection
│   │   │   │   │   ├── interruptions.rs                                                 # Interrupted work never reads as complete: a stopped run resumes, a changed counter fails the set
│   │   │   │   │   ├── latest.rs                                                        # A document is prepared by its latest revision alone; one whose latest is held or failed is left out and reported
│   │   │   │   │   ├── mod.rs                                                           # Tests of the router tokenizer, and of the preparation of a collection over it
│   │   │   │   │   ├── near.rs                                                          # Near duplicates: signatures and bands propose, an exact Jaccard of 0.85 or more confirms, pairs link groups
│   │   │   │   │   ├── oversized.rs                                                     # A unit that cannot fit 700 tokens with its context refuses its document by name; the rest are prepared
│   │   │   │   │   ├── parity.rs                                                        # The parity fixtures: how their file writes them, and what the built-in
│   │   │   │   │   ├── port.rs                                                          # A model port that answers each parity fixture with the native counter's
│   │   │   │   │   ├── qualification.rs                                                 # Qualification: a router tokenizer exists only once the port gives every
│   │   │   │   │   ├── qualification_validation.rs                                      # Rust source: qualification validation
│   │   │   │   │   ├── refusals.rs                                                      # The refusals: what each says, and the cause each keeps
│   │   │   │   │   ├── router_client.rs                                                 # The router client through a router tokenizer, against a stub router: from
│   │   │   │   │   ├── scratch.rs                                                       # What the preparation's tests share: a scratch corpus and kernel, a collection imported and decided
│   │   │   │   │   ├── section_reader.rs                                                # The public reader returns only exact, bounded canonical section spans
│   │   │   │   │   ├── stops.rs                                                         # Why a preparation stops: what each stop says, and the cause each keeps
│   │   │   │   │   ├── stub.rs                                                          # A stub of the model router for the router client's tests: a loopback HTTP
│   │   │   │   │   ├── support.rs                                                       # What the router tokenizer's tests share: model cards, recorded in a
│   │   │   │   │   └── synthetic.rs                                                     # The public synthetic collection prepared end to end: one exact group, two near-duplicate groups
│   │   │   │   ├── bridge.rs                                                            # A model port's asynchronous tokenize, called synchronously: the port's
│   │   │   │   ├── chunking.rs                                                          # Chunking prepared revisions: each prepared input stored, a revision's chunks recorded at once
│   │   │   │   ├── collection.rs                                                        # Preparing a collection: its eligible revisions deduplicated, then chunked into the chunk set they name
│   │   │   │   ├── counter.rs                                                           # The router tokenizer as the chunker counts through it, keeping the typed refusal the chunker holds as text
│   │   │   │   ├── error.rs                                                             # Why a router tokenizer refuses to qualify, or to count
│   │   │   │   ├── exact.rs                                                             # Exact duplicates: the same original bytes and canonical content, prepared once as the smallest revision
│   │   │   │   ├── failure.rs                                                           # Why a preparation stopped, and what a rerun does then
│   │   │   │   ├── left_out.rs                                                          # The documents a preparation leaves out, since the quality gate does not let their latest revision through, each with why
│   │   │   │   ├── manifest.rs                                                          # A chunk set's identity, and its manifest maestro-chunk-set/1, which a complete set pins
│   │   │   │   ├── mod.rs                                                               # Preparing revisions for search: deduplicated, then chunked in the tokens of the selected embedder
│   │   │   │   ├── native-parity.json                                                   # The native counter's ordered IDs for the 41 parity fixtures a router tokenizer must match to qualify
│   │   │   │   ├── near.rs                                                              # Near duplicates: word 5-gram shingles, MinHash bands, exact Jaccard confirmation, groups that delete nothing
│   │   │   │   ├── parity.rs                                                            # The native profile's parity fixtures, native-parity.json: complete
│   │   │   │   ├── qualification.rs                                                     # Strict native token-ID evidence for one v2 model identity
│   │   │   │   ├── report.rs                                                            # What a preparation reports, as JSON: its chunk set, its counts and each refusal
│   │   │   │   └── router_tokenizer.rs                                                  # The router tokenizer: maestro-canonicalization's TokenCounter over the
│   │   │   ├── quality/                                                                 # The quality gate: one disposition per revision before indexing (01 §4, FR-S1-002a)
│   │   │   │   ├── checks/                                                              # The automatic checks, each a rule ID with a documented threshold
│   │   │   │   │   ├── body.rs                                                          # The body as the checks count it: near-empty and navigation-heavy
│   │   │   │   │   ├── flag.rs                                                          # What a check says of a revision it flags: rule, outcome and reason
│   │   │   │   │   ├── mod.rs                                                           # The automatic checks, each a rule ID with a documented threshold
│   │   │   │   │   ├── page.rs                                                          # A page that is not the document: an application error, a sign-in prompt
│   │   │   │   │   ├── record.rs                                                        # Canonicalization failures, missing provenance and assets, incomplete tables
│   │   │   │   │   ├── rules.rs                                                         # Every automatic check, run in the order of the module table
│   │   │   │   │   ├── secret.rs                                                        # text.suspected-secret: PEM private keys and AWS, GitHub, GitLab, Slack tokens
│   │   │   │   │   └── text.rs                                                          # Replacement characters and extraction markers left in the text
│   │   │   │   ├── tests/                                                               # Each check with a document it flags and one it must not; the precedence
│   │   │   │   │   ├── body.rs                                                          # The body checks: near-empty, navigation, application error, sign-in
│   │   │   │   │   ├── mod.rs                                                           # Each check with a document it flags and one it must not; the precedence
│   │   │   │   │   ├── precedence.rs                                                    # A ledger rule outranks the checks but never accepts a failed document
│   │   │   │   │   ├── record.rs                                                        # The record checks: canonicalization, metadata, assets, tables, characters
│   │   │   │   │   ├── secrets.rs                                                       # Each secret format with examples it flags and must not, placeholders among them
│   │   │   │   │   └── support.rs                                                       # Canonical documents made from Markdown, revisions and ledgers for the tests
│   │   │   │   ├── decide.rs                                                            # One decision: the first ledger rule that matches, else the checks
│   │   │   │   ├── error.rs                                                             # Why the gate stopped; what it decided before stays decided
│   │   │   │   ├── gate.rs                                                              # The gate over a collection, and the eligible revisions it lets through
│   │   │   │   ├── ledger.rs                                                            # The quality ledger, maestro-quality-ledger/1: one strict JSON rule a line
│   │   │   │   ├── mod.rs                                                               # The gate, its precedence, and the list of automatic checks
│   │   │   │   ├── outcome.rs                                                           # The five outcomes by their 01 §4 names, and which hold a revision back
│   │   │   │   └── report.rs                                                            # What the gate reports: every revision counted once, the held ones listed
│   │   │   ├── query/                                                                   # Query
│   │   │   │   ├── identifier_numbers.rs                                                # Version, port and error-code identifier patterns
│   │   │   │   ├── identifier_patterns.rs                                               # Parameter, command and path identifier patterns
│   │   │   │   ├── identifier_types.rs                                                  # Public identifier data and internal source-span candidates
│   │   │   │   ├── identifiers.rs                                                       # Identifier detection and overlap resolution
│   │   │   │   ├── index.rs                                                             # Exact identifiers to store beside published prepared inputs
│   │   │   │   ├── kind.rs                                                              # Rule-based query-kind classification in English and French
│   │   │   │   ├── language.rs                                                          # Language detection for normalized queries
│   │   │   │   ├── mod.rs                                                               # Rust source: mod
│   │   │   │   ├── normalize.rs                                                         # Whitespace normalization for queries
│   │   │   │   ├── tests.rs                                                             # Regression tests for deterministic query understanding
│   │   │   │   └── understand.rs                                                        # Public query-understanding result and entry point
│   │   │   ├── search/                                                                  # Search
│   │   │   │   ├── evidence/                                                            # Internal algorithms for assembling authoritative, bounded evidence
│   │   │   │   │   ├── assemble/                                                        # Runs evidence assembly under the handoff's absolute deadline
│   │   │   │   │   │   ├── candidates.rs                                                # Builds source candidates and derives their version/conflict membership
│   │   │   │   │   │   ├── deadline.rs                                                  # Runs evidence assembly under the handoff's absolute deadline
│   │   │   │   │   │   ├── engine.rs                                                    # Loads the pinned candidate pool and drives source selection
│   │   │   │   │   │   ├── ledger.rs                                                    # The duplicate facts a complete chunk set's manifest permits T032 to use
│   │   │   │   │   │   ├── mod.rs                                                       # Runs evidence assembly under the handoff's absolute deadline
│   │   │   │   │   │   ├── output.rs                                                    # Builds trace and gaps, validates the bundle, and rechecks authority
│   │   │   │   │   │   ├── types.rs                                                     # Owned intermediate records shared by the assembly worker's small modules
│   │   │   │   │   │   └── validate.rs                                                  # Validates the bounded T029c handoff before any blocking work starts
│   │   │   │   │   ├── conflicts/                                                       # Conflicts
│   │   │   │   │   │   ├── detect.rs                                                    # Rust source: detect
│   │   │   │   │   │   ├── emit.rs                                                      # Converts selected conflict facts into passage-numbered bundle signals
│   │   │   │   │   │   ├── mod.rs                                                       # Rust source: mod
│   │   │   │   │   │   └── tables.rs                                                    # Rust source: tables
│   │   │   │   │   ├── features/                                                        # Computes case-sensitive shingles and source features for evidence ranking
│   │   │   │   │   │   ├── mod.rs                                                       # Case-sensitive shingles and protected source signatures
│   │   │   │   │   │   ├── mutation_tests.rs                                            # Regression checks for source offsets used by diversity signatures
│   │   │   │   │   │   └── signatures.rs                                                # Computes case-sensitive shingles and source features for evidence ranking
│   │   │   │   │   ├── sections/                                                        # Derives validated section extents and bounded sibling windows
│   │   │   │   │   │   ├── tests/                                                       # Length-only block validation tests
│   │   │   │   │   │   │   ├── index.rs                                                 # Rust source: index
│   │   │   │   │   │   │   ├── mod.rs                                                   # Length-only block validation tests
│   │   │   │   │   │   │   └── validation.rs                                            # The length-only path rejects the same malformed block spans as the text path
│   │   │   │   │   │   ├── index.rs                                                     # Derives validated section extents and bounded sibling windows
│   │   │   │   │   │   ├── mod.rs                                                       # Derives validated section extents and bounded sibling windows
│   │   │   │   │   │   └── validation.rs                                                # Validates canonical block links and source spans before evidence expansion
│   │   │   │   │   ├── selection/                                                       # Budgeted MMR selection over validated source windows
│   │   │   │   │   │   ├── algorithm.rs                                                 # MMR ordering, atomic conflict units and budget-driven source windows
│   │   │   │   │   │   ├── mmr.rs                                                       # Deterministic MMR ranking and conflict-atomic selection units
│   │   │   │   │   │   ├── mod.rs                                                       # Budgeted MMR selection over validated source windows
│   │   │   │   │   │   ├── relevant.rs                                                  # Whole matched-block windows and contiguous table-header prefixes
│   │   │   │   │   │   ├── render.rs                                                    # Renders trial spans as normalized, validated passages
│   │   │   │   │   │   ├── render_cluster.rs                                            # Renders one exact source cluster and attributes supporting candidates
│   │   │   │   │   │   ├── trial.rs                                                     # Validates, renders and charges complete delivery trials
│   │   │   │   │   │   └── types.rs                                                     # Candidate metadata and final selection result
│   │   │   │   │   ├── tests/                                                           # Integration tests
│   │   │   │   │   │   ├── assembly.rs                                                  # Rust source: assembly
│   │   │   │   │   │   ├── assembly_authority.rs                                        # Rust source: assembly authority
│   │   │   │   │   │   ├── assembly_entry.rs                                            # Rust source: assembly entry
│   │   │   │   │   │   ├── assembly_output.rs                                           # Rust source: assembly output
│   │   │   │   │   │   ├── assembly_versions.rs                                         # Rust source: assembly versions
│   │   │   │   │   │   ├── budget.rs                                                    # Rust source: budget
│   │   │   │   │   │   ├── candidate_context.rs                                         # Rust source: candidate context
│   │   │   │   │   │   ├── clock_cutoffs.rs                                             # Evidence assembly and source loading read their cutoff on the clock the
│   │   │   │   │   │   ├── conflict_emission.rs                                         # Rust source: conflict emission
│   │   │   │   │   │   ├── conflict_structures.rs                                       # Rust source: conflict structures
│   │   │   │   │   │   ├── conflicts.rs                                                 # Rust source: conflicts
│   │   │   │   │   │   ├── features.rs                                                  # Rust source: features
│   │   │   │   │   │   ├── mod.rs                                                       # Rust source: mod
│   │   │   │   │   │   ├── packing.rs                                                   # Rust source: packing
│   │   │   │   │   │   ├── parent_chain.rs                                              # Rust source: parent chain
│   │   │   │   │   │   ├── parent_tiers.rs                                              # Rust source: parent tiers
│   │   │   │   │   │   ├── section_expansion.rs                                         # Rust source: section expansion
│   │   │   │   │   │   ├── section_selection.rs                                         # Rust source: section selection
│   │   │   │   │   │   ├── section_validation.rs                                        # Rust source: section validation
│   │   │   │   │   │   ├── sections.rs                                                  # Rust source: sections
│   │   │   │   │   │   ├── selection.rs                                                 # Rust source: selection
│   │   │   │   │   │   ├── selection_conflict.rs                                        # Rust source: selection conflict
│   │   │   │   │   │   ├── signals.rs                                                   # Rust source: signals
│   │   │   │   │   │   ├── source.rs                                                    # Rust source: source
│   │   │   │   │   │   ├── spans.rs                                                     # Rust source: spans
│   │   │   │   │   │   ├── support.rs                                                   # A prepared chunk set and generation for evidence-path tests
│   │   │   │   │   │   └── versions.rs                                                  # Rust source: versions
│   │   │   │   │   ├── anchor.rs                                                        # The source anchor of each passage of an assembled bundle: where its text
│   │   │   │   │   ├── budget.rs                                                        # Token-counting helpers for compact serialized evidence
│   │   │   │   │   ├── candidate_context.rs                                             # Reuses authoritative evidence sections for reranker-only context
│   │   │   │   │   ├── chunk_set_documents.rs                                           # The documents of a chunk set by source_ref: the ladder resolves its
│   │   │   │   │   ├── delivery_graph.rs                                                # Replaceable delivery choices derived from validated canonical structure
│   │   │   │   │   ├── families.rs                                                      # Shared candidate-family identity for conflicts and documentary versions
│   │   │   │   │   ├── mod.rs                                                           # Authoritative section reads and bounded evidence assembly
│   │   │   │   │   ├── section_reader.rs                                                # Reads one authorized canonical section from a completed chunk set
│   │   │   │   │   ├── selection_candidate.rs                                           # Already validated source candidate metadata
│   │   │   │   │   ├── signals.rs                                                       # Rust source: signals
│   │   │   │   │   ├── source.rs                                                        # Loads and caches authoritative source records for one assembly request
│   │   │   │   │   ├── spans.rs                                                         # Candidate source-span unions
│   │   │   │   │   ├── types.rs                                                         # Public evidence request types and their stable error boundary
│   │   │   │   │   └── versions.rs                                                      # Numeric release-label ordering
│   │   │   │   ├── routes/                                                              # Independent route diagnostics
│   │   │   │   │   ├── dense.rs                                                         # Dense diagnostic search through the generation's own embedder and vector
│   │   │   │   │   ├── error.rs                                                         # Typed failures for the admission and diagnostic routes
│   │   │   │   │   ├── identifier.rs                                                    # Exact identifier search from Qdrant payloads and the kernel identifier index
│   │   │   │   │   ├── identifier_cursor.rs                                             # Backend-neutral cursor ordering for identifier payload pagination
│   │   │   │   │   ├── identifier_payload.rs                                            # Validation of identifier-route payload results
│   │   │   │   │   ├── lexical.rs                                                       # BM25 diagnostic search, using the generation's recorded analyzer profile
│   │   │   │   │   ├── mod.rs                                                           # Independent route diagnostics
│   │   │   │   │   ├── outcome.rs                                                       # A route's hits and its independent availability status
│   │   │   │   │   ├── results.rs                                                       # Route-order results and the common first-hit deduplication rule
│   │   │   │   │   ├── structured.rs                                                    # Exact generation inventories and their separately bounded supports
│   │   │   │   │   └── tests.rs                                                         # Rust source: tests
│   │   │   │   ├── tests/                                                               # Integration tests
│   │   │   │   │   ├── admission.rs                                                     # Request-boundary checks before generation admission
│   │   │   │   │   ├── candidate_enrichment.rs                                          # Opt-in reranker enrichment degrades to the indexed chunk, never to a failed search
│   │   │   │   │   ├── clock.rs                                                         # A settable clock for the blocking stages' cutoff tests
│   │   │   │   │   ├── deadlines.rs                                                     # Rust source: deadlines
│   │   │   │   │   ├── fusion.rs                                                        # Rust source: fusion
│   │   │   │   │   ├── handoff.rs                                                       # Route joining and rerank handoff boundaries
│   │   │   │   │   ├── intent.rs                                                        # Guarded additive intent expansion contracts
│   │   │   │   │   ├── intent_guard.rs                                                  # The expansion guard: quantities in any spelling, protected constraint
│   │   │   │   │   ├── intent_request.rs                                                # The expansion port: the one chat request the HyDE adapter sends, the
│   │   │   │   │   ├── inventory_query.rs                                               # Exact English and French inventory-query grammar
│   │   │   │   │   ├── mod.rs                                                           # Rust source: mod
│   │   │   │   │   ├── rank_policy.rs                                                   # Rust source: rank policy
│   │   │   │   │   ├── rank_stage.rs                                                    # The search's candidate stage end to end: exact texts, optional context
│   │   │   │   │   ├── rerank.rs                                                        # Rust source: rerank
│   │   │   │   │   ├── route_setup.rs                                                   # A route's one-time setup, such as loading its model, is not route time
│   │   │   │   │   ├── routes.rs                                                        # Pure route helpers: Qdrant scope filters and ranked-hit cleanup
│   │   │   │   │   ├── section_prior.rs                                                 # The soft section prior: strict demotion and precise, bilingual exemptions
│   │   │   │   │   ├── source_class.rs                                                  # Source classes: the table adapter, the class vocabulary and the prior
│   │   │   │   │   ├── stage_clocks.rs                                                  # Each blocking search stage reads its cutoff on the clock its request
│   │   │   │   │   ├── stages.rs                                                        # The outcome each end of a search, a route or the rerank gives its stage
│   │   │   │   │   ├── support.rs                                                       # Scratch kernel records for the bounded candidate handoff
│   │   │   │   │   ├── top_scores.rs                                                    # Rust source: top scores
│   │   │   │   │   └── window_boundaries.rs                                             # Rust source: window boundaries
│   │   │   │   ├── admission.rs                                                         # Request bounds, scope snapshots and generation admission
│   │   │   │   ├── assembly_settings.rs                                                 # Per-request evidence assembly knobs, separate from copyable search ranking
│   │   │   │   ├── candidate_enrichment.rs                                              # Opt-in reranker context and section classes read from authoritative sources
│   │   │   │   ├── candidates.rs                                                        # Scoped prepared-input loading for the fused candidate IDs only
│   │   │   │   ├── deadline.rs                                                          # Absolute cutoffs shared by routes, candidate loading and T032 handoff
│   │   │   │   ├── filter.rs                                                            # Qdrant's in-route filter for the scopes admitted to one request
│   │   │   │   ├── fusion.rs                                                            # Reciprocal rank fusion over independent retrieval routes
│   │   │   │   ├── hyde.rs                                                              # The hypothetical-document expander: a configured answerer card, called
│   │   │   │   ├── intent.rs                                                            # Optional query expansion behind a replaceable port; generated text is
│   │   │   │   ├── intent_guard.rs                                                      # The mechanical guard every expansion passes before it may vote
│   │   │   │   ├── intent_routes.rs                                                     # Independent additive routes over guarded model-generated retrieval inputs
│   │   │   │   ├── intent_words.rs                                                      # Per-language word lists of the expansion guard: data, not rules
│   │   │   │   ├── inventory_query.rs                                                   # Exact natural-language grammar for S1 document and version inventories
│   │   │   │   ├── mod.rs                                                               # Rust source: mod
│   │   │   │   ├── orchestrate.rs                                                       # Admission, parallel route execution, fusion and the bounded T032 handoff
│   │   │   │   ├── pin.rs                                                               # Pins the published generation before any route touches Qdrant
│   │   │   │   ├── query.rs                                                             # The scopes and generation admitted for a search
│   │   │   │   ├── rank_policy.rs                                                       # Optional position-only rerank policies; model and fusion scores remain untouched
│   │   │   │   ├── rank_stage.rs                                                        # The candidate stage of a search: exact loading, optional enrichment
│   │   │   │   ├── request.rs                                                           # Retrieval results handed to T032 without assembling an evidence bundle
│   │   │   │   ├── rerank.rs                                                            # Reranks the head of a fused list without truncating candidate text
│   │   │   │   ├── route_execution.rs                                                   # Deadline-bounded leaf-route calls and their independent public statuses
│   │   │   │   ├── route_search.rs                                                      # Cached original routes and additive fusion for one pinned search
│   │   │   │   ├── section_prior.rs                                                     # Configured section classes and an optional soft reciprocal-rank penalty
│   │   │   │   ├── source_class.rs                                                      # Source classes: the port that names where a document comes from, and the
│   │   │   │   └── source_class_table.rs                                                # The default source classifier: a strict maestro-source-classes/1 JSON
│   │   │   ├── answer.rs                                                                # Evidence-grounded answering with bounded generation and one validation retry
│   │   │   ├── collection.rs                                                            # A collection's declaration: maestro-collection/1, the strict JSON that
│   │   │   ├── corpus.rs                                                                # A corpus manifest: maestro-corpus/1, one JSON line per document, through
│   │   │   ├── lib.rs                                                                   # The knowledge pipeline of Maestro (docs/architecture/01): collections, their
│   │   │   ├── publish.rs                                                               # Verification of a published generation against the kernel artifacts and
│   │   │   ├── relative_path.rs                                                         # Paths that a declaration or a manifest gives relative to a directory, which
│   │   │   ├── shape.rs                                                                 # The JSON shapes the contracts name, and no other: an object where a
│   │   │   ├── strict_json.rs                                                           # Bound and de-duplicate JSON before any typed resource is materialized
│   │   │   └── suite.rs                                                                 # An evaluation suite: maestro-suite/1, one JSON line per question, which
│   │   ├── tests/                                                                       # Integration tests
│   │   │   ├── fixtures/                                                                # Test fixtures
│   │   │   │   └── synthetic/                                                           # Synthetic
│   │   │   │       └── evals/                                                           # Evals
│   │   │   │           └── model-card-v2.json                                           # JSON data: model card v2
│   │   │   └── it/                                                                      # It
│   │   │       ├── import_contract/                                                     # The import of corpus manifests (T019): refusals, holds, idempotency, identity, report, synthetic collection
│   │   │       │   ├── declared_collections.rs                                          # Declaring a collection records it and its sources, and nothing for a caller who cannot read them
│   │   │       │   ├── document_identity.rs                                             # A document's identity: its collection and source_ref hashed, its path relative to the manifest
│   │   │       │   ├── held_pairs.rs                                                    # Lines sharing a source_ref with different digests: both revisions recorded, both held
│   │   │       │   ├── import_rate.rs                                                   # The import rate on 2,000 generated documents, measured on demand
│   │   │       │   ├── import_report.rs                                                 # What an import reports, as JSON and as import.completed, and what stops it before any work
│   │   │       │   ├── mod.rs                                                           # The import of corpus manifests (T019): refusals, holds, idempotency, identity, report, synthetic collection
│   │   │       │   ├── observed_imports.rs                                              # An observed import: the report every hundred lines and after each last line; a break stops it
│   │   │       │   ├── refused_entries.rs                                               # Refusals, each with its line and reason, and the import goes on
│   │   │       │   ├── repeated_imports.rs                                              # An import is idempotent: a second one writes nothing, a change of metadata gives a new revision
│   │   │       │   ├── support.rs                                                       # What the import's tests share: a scratch corpus and database, declarations, and the kernel read whole
│   │   │       │   └── synthetic_corpus.rs                                              # The public synthetic collection (T014) imported end to end
│   │   │       ├── local_collection/                                                    # The collection this machine names, imported for real and gated on demand
│   │   │       │   ├── dispositions.rs                                                  # The disposition report of a local run: identities, rules, reasons, counts
│   │   │       │   ├── mod.rs                                                           # The collection this machine names, imported for real and gated on demand
│   │   │       │   └── real_import.rs                                                   # The ignored run: a collection imported into the kernel data directory, then gated
│   │   │       ├── qdrant_projection/                                                   # The Qdrant projection (T026), against a fake Qdrant and a real one when named
│   │   │       │   ├── fake/                                                            # A fake Qdrant: a gRPC server in memory, on a loopback port
│   │   │       │   │   ├── collections.rs                                               # The fake's collections: creation, existence, parameters and aliases
│   │   │       │   │   ├── filter.rs                                                    # The fake's exact keyword-condition evaluation shared by query and scroll
│   │   │       │   │   ├── mod.rs                                                       # A fake Qdrant: a gRPC server in memory, on a loopback port
│   │   │       │   │   ├── points.rs                                                    # The fake's points: written, checked and normalized as Qdrant does, found and counted
│   │   │       │   │   ├── query.rs                                                     # Qdrant's filtered nearest-vector query, as the projection tests' fake serves it
│   │   │       │   │   ├── response.rs                                                  # Qdrant point responses filtered by requested payload and vectors
│   │   │       │   │   ├── scroll.rs                                                    # Filtered, point-ID-ordered pages for the fake's Qdrant scroll API
│   │   │       │   │   ├── server.rs                                                    # The fake's server: its two services on a loopback port, in the test's runtime
│   │   │       │   │   └── state.rs                                                     # What the fake keeps, and the refusals and hollow answers a test asked for
│   │   │       │   ├── projection_rebuild/                                              # Crash recovery assertions for lost or partially published projections
│   │   │       │   │   ├── alias_guards.rs                                              # The published alias a guarded replacement restores or refuses to leave
│   │   │       │   │   ├── counter_contract.rs                                          # Recovery refuses chunk sets counted for a different v2 card
│   │   │       │   │   ├── crash_boundaries.rs                                          # Crash boundaries around alias and kernel publication
│   │   │       │   │   ├── mod.rs                                                       # Crash recovery assertions for lost or partially published projections
│   │   │       │   │   ├── rebuild_tests.rs                                             # Explicit replacement of a lost published projection
│   │   │       │   │   └── rebuild_validation.rs                                        # Refusals for malformed journaled projection rebuild state
│   │   │       │   ├── search_routes/                                                   # Dense and lexical routes against the shared fake and, when configured, real Qdrant
│   │   │       │   │   ├── chunk_set_documents.rs                                       # The ladder reads the documents of a chunk set by source_ref
│   │   │       │   │   ├── cold_reranker.rs                                             # A cold or slow reranker costs a search only its rerank
│   │   │       │   │   ├── configured_ask.rs                                            # ask searches with the configuration its caller gives
│   │   │       │   │   ├── configured_search.rs                                         # Request configuration controls the real fused search pipeline
│   │   │       │   │   ├── fused_search.rs                                              # The public search handoff fuses available routes when dense metadata is absent
│   │   │       │   │   ├── fused_search_admission_pinning.rs                            # Search admission and generation-pinning acceptance tests
│   │   │       │   │   ├── identifier_noise_guard.rs                                    # The identifier noise guard: a too-common identifier gets no payload votes
│   │   │       │   │   ├── identifier_route.rs                                          # Exact identifiers combine payload equality with the scoped kernel index
│   │   │       │   │   ├── identifier_route_resilience.rs                               # Independent exact-identifier leg failures and empty results
│   │   │       │   │   ├── idle_unload.rs                                               # A long-lived router client, as the MCP server shares, reloads a model the
│   │   │       │   │   ├── intent_expansion.rs                                          # Real search admission and fusion with a controlled expansion model
│   │   │       │   │   ├── intent_fallbacks.rs                                          # An expanded search never ends worse than the original one: a failed or
│   │   │       │   │   ├── intent_port.rs                                               # A model port whose expansion chat and reranker a test scripts, over the
│   │   │       │   │   ├── intent_rerank_set.rs                                         # What the rerank scores once intent routes voted: the original top-depth
│   │   │       │   │   ├── mod.rs                                                       # Dense and lexical routes against the shared fake and, when configured, real Qdrant
│   │   │       │   │   ├── route_behavior.rs                                            # Behavior of dense and lexical searches over the generation-pinned collection
│   │   │       │   │   ├── route_errors.rs                                              # Refusals from admission, profiles and dense embedding
│   │   │       │   │   ├── route_windows.rs                                             # A route's window comes from the request's budget, less the time the later
│   │   │       │   │   ├── scope_index.rs                                               # A new generation indexes the scope payload it searches by
│   │   │       │   │   ├── search_projection.rs                                         # Publication's identifier payload and readiness marker
│   │   │       │   │   ├── structured_route.rs                                          # Exact document inventories stay separate from their supporting chunks
│   │   │       │   │   └── support.rs                                                   # Qdrant points and pinned generations shared by route tests
│   │   │       │   ├── alias_moves.rs                                                   # The alias moves only to a generation whose collection passes its checks
│   │   │       │   ├── backends.rs                                                      # The Qdrant servers a test runs against, and what it reads back from them
│   │   │       │   ├── built_generations.rs                                             # A generation builds in its own collection; its points, payloads and IDs
│   │   │       │   ├── fake_port.rs                                                     # Publication and standalone verification through a transport-independent projection fake
│   │   │       │   ├── fake_port_failures.rs                                            # Failure, resume, and rollback tests for the fake projection port
│   │   │       │   ├── kernel.rs                                                        # A kernel holding a complete chunk set to publish, in a scratch directory
│   │   │       │   ├── mod.rs                                                           # The Qdrant projection (T026), against a fake Qdrant and a real one when named
│   │   │       │   ├── models.rs                                                        # The embedder the tests publish through, which records and spoils calls on demand
│   │   │       │   ├── publication_verification.rs                                      # Published-generation verification names corrupted inputs, bad counts and
│   │   │       │   ├── refused_batches.rs                                               # A batch whose vectors break a check is refused whole
│   │   │       │   ├── resumed_builds.rs                                                # An interrupted build resumes after its last journaled batch
│   │   │       │   ├── scratch_cleanup.rs                                               # A kernel's scratch directory, which may sit on tmpfs, is gone once the
│   │   │       │   ├── server_version.rs                                                # The client reads the server's version from its health check
│   │   │       │   ├── stopped_builds.rs                                                # Refusals before any work, unreadable chunks, and a Qdrant that refuses or is out of reach
│   │   │       │   ├── stopped_clock.rs                                                 # A stopped clock, for the searches whose deadlines a loaded host must not
│   │   │       │   └── support.rs                                                       # What the projection's tests share: a publication and the names it gives
│   │   │       ├── quality_gate/                                                        # The quality gate (T020) through the import, the ledger and the kernel
│   │   │       │   ├── gate_report.rs                                                   # The report as JSON, and the stops: unknown collection, broken artifact
│   │   │       │   ├── kept_dispositions.rs                                             # A disposition is kept: a rerun decides nothing, the import holds stay
│   │   │       │   ├── latest_revision.rs                                               # A document is eligible by its latest revision alone, never an older one
│   │   │       │   ├── mod.rs                                                           # The quality gate (T020) through the import, the ledger and the kernel
│   │   │       │   ├── recorded_dispositions.rs                                         # Dispositions with rule IDs and reasons, revision.held, eligibility
│   │   │       │   ├── support.rs                                                       # A scratch corpus imported as garden, and what the kernel then records
│   │   │       │   └── synthetic_corpus.rs                                              # The public synthetic collection (T014) passes the gate end to end
│   │   │       ├── suite_check/                                                         # Checking suites against their corpus: every name a suite of a directory
│   │   │       │   ├── check.rs                                                         # The check itself: each suite of a directory read under its contract, and
│   │   │       │   ├── local_suites.rs                                                  # The check on the suites and corpus this machine names, by hand
│   │   │       │   ├── mod.rs                                                           # Checking suites against their corpus: every name a suite of a directory
│   │   │       │   ├── reported_problems.rs                                             # What the check reports as problems: a name that gives no one section nor
│   │   │       │   ├── scratch.rs                                                       # What the check's tests share: a scratch directory holding a corpus, its
│   │   │       │   ├── synthetic_suite.rs                                               # The check on the public synthetic collection: every section its suite
│   │   │       │   └── unanswerable_leads.rs                                            # Leads for unanswerable questions: each document of the manifest that holds
│   │   │       ├── synthetic_gate/                                                      # Public synthetic retrieval regression gate, with a real-Qdrant CI leg
│   │   │       │   ├── pipeline/                                                        # The staged fixture, import, evaluation and report path for the synthetic gate
│   │   │       │   │   ├── baseline.rs                                                  # Strict baseline loading and deterministic candidate comparison
│   │   │       │   │   ├── contract.rs                                                  # Run identity and result types shared by the synthetic pipeline stages
│   │   │       │   │   ├── driver.rs                                                    # Builds, prepares, publishes and evaluates the public synthetic collection
│   │   │       │   │   ├── fixture.rs                                                   # Loads the pinned synthetic collection and verifies every fixture digest
│   │   │       │   │   ├── mod.rs                                                       # Pipeline door: declarations and public entry-point re-exports
│   │   │       │   │   ├── published.rs                                                 # The synthetic collection imported, prepared and published against a fake
│   │   │       │   │   ├── readiness.rs                                                 # Waits for an exact, green Qdrant collection before the real retrieval pass
│   │   │       │   │   ├── report.rs                                                    # Writes public-only reports, provenance and summaries for a gate run
│   │   │       │   │   └── runner.rs                                                    # Runs the test adapter against fake Qdrant or the pinned real service
│   │   │       │   ├── comparison.rs                                                    # Deterministic, exact comparison of complete public synthetic runs
│   │   │       │   ├── failure.rs                                                       # Stages, items and completed-question counts for a failed synthetic run
│   │   │       │   ├── mod.rs                                                           # Public synthetic retrieval regression gate, with a real-Qdrant CI leg
│   │   │       │   ├── models.rs                                                        # Synthetic-only tokenizer canaries plus deterministic fake inference
│   │   │       │   ├── policy_tests.rs                                                  # Synthetic-gate policy checks, built from strict T021 reports
│   │   │       │   ├── qdrant.rs                                                        # Ownership checks and cleanup for the test-owned Qdrant instance
│   │   │       │   ├── real_qdrant.rs                                                   # Exercises the mandatory synthetic pipeline against the pinned Qdrant service
│   │   │       │   ├── search.rs                                                        # Test-only bridge from the integrated dense/lexical routes and fusion to T021
│   │   │       │   ├── span_recorder.rs                                                 # A minimal recording subscriber, written with the tracing API alone: it
│   │   │       │   ├── stage_spans.rs                                                   # The stages of the production publication, search and evidence assembly
│   │   │       │   └── support.rs                                                       # Owns private scratch directories used by synthetic-gate tests
│   │   │       ├── answer_live.rs                                                       # A single buffered ask against a registered answerer, live: an explicit
│   │   │       ├── collection_contract.rs                                               # maestro-collection/1: a strict declaration parses into typed values; an
│   │   │       ├── corpus_contract.rs                                                   # maestro-corpus/1: one line per document parses into typed values; an
│   │   │       ├── eval_synthetic.rs                                                    # The evaluation runner over the public synthetic suite (T014), end to end
│   │   │       ├── lexical_accents.rs                                                   # Properties of bm25-en-fr/1 over generated texts: a text and the same
│   │   │       ├── lexical_fold.rs                                                      # Folding in bm25-en-fr/1: every letter of Latin-1 Supplement and Latin
│   │   │       ├── lexical_golden.rs                                                    # The golden of bm25-en-fr/1: the terms and vectors of sample passages and
│   │   │       ├── lexical_rules.rs                                                     # The rules of bm25-en-fr/1 as the lexical module states them, each with
│   │   │       ├── lexical_sample.rs                                                    # Research R7's public sample on bm25-en-fr/1: 22 passages, 11 in English
│   │   │       ├── lexical_vectors.rs                                                   # The sparse vectors of bm25-en-fr/1: a passage's term weighs BM25's
│   │   │       ├── live_router.rs                                                       # What the live tests share: the router their variables name, and its embedder's model card
│   │   │       ├── main.rs                                                              # The crate's integration tests, built as one test crate: each module proves
│   │   │       ├── prepare_live.rs                                                      # knowledge prepare on this machine's kernel as a leased job, live: the chunk count, wall time and router calls
│   │   │       ├── publish_live.rs                                                      # A chunk set of this machine's kernel published into Qdrant as a leased job, live: the embed and upsert rates
│   │   │       ├── quality_ledger.rs                                                    # maestro-quality-ledger/1: strict rules a line; a missing ledger is empty
│   │   │       ├── router_parity.rs                                                     # The router tokenizer's parity with the native counter, live: an explicit
│   │   │       ├── search_live.rs                                                       # Search this machine's published collection against its live Qdrant and model
│   │   │       ├── suite_contract.rs                                                    # maestro-suite/1: a suite, one JSON line per question, parses into typed
│   │   │       ├── suite_resolution.rs                                                  # Resolving an expected section in its canonicalized document: a heading path
│   │   │       └── synthetic_collection.rs                                              # The public synthetic collection, tests/fixtures/synthetic, which stands in
│   │   └── Cargo.toml                                                                   # Crate manifest: The knowledge pipeline of Maestro, starting with the collection and corpus contracts it imports through
│   ├── maestro-settings/                                                                # Maestro settings
│   │   ├── src/                                                                         # The crate's sources
│   │   │   ├── edit/                                                                    # Editing a preferences file in place: where its keys are, and the set and
│   │   │   │   ├── change.rs                                                            # config set and config unset on a preferences file's text
│   │   │   │   ├── document.rs                                                          # Where a preferences file writes each key and table, from the spans of
│   │   │   │   └── mod.rs                                                               # Editing a preferences file in place: where its keys are, and the set and
│   │   │   ├── file/                                                                    # A preferences file's own I/O: its directory opened without following a
│   │   │   │   ├── bounded.rs                                                           # Bounded reads of a preferences file: no further than [MAX_FILE_BYTES]
│   │   │   │   ├── edit.rs                                                              # One edit of a preferences file, from reading it to keeping or undoing
│   │   │   │   ├── layers.rs                                                            # The file adapter of [LayerSource]: the user's preferences.toml in
│   │   │   │   ├── mod.rs                                                               # A preferences file's own I/O: its directory opened without following a
│   │   │   │   ├── place.rs                                                             # Where a preferences file lives: a directory the caller trusts, the one
│   │   │   │   ├── tests.rs                                                             # Tests for the file adapter's private platform operations
│   │   │   │   ├── unix.rs                                                              # Unix: every name below the trusted directory resolves against an open
│   │   │   │   ├── windows.rs                                                           # Windows: names resolve by path, but the directory of the file is held
│   │   │   │   └── windows_logic.rs                                                     # Platform-neutral Windows decisions, tested on every host
│   │   │   ├── tests/                                                                   # The settings crate's tests, one module per source module, and the proof
│   │   │   │   ├── discovery.rs                                                         # Project-file discovery: the nearest .maestro/config.toml upward from the
│   │   │   │   ├── edit.rs                                                              # config set and config unset on a file's text: only the value's bytes
│   │   │   │   ├── file.rs                                                              # One edit of a preferences file: serialized by its lock, refused when
│   │   │   │   ├── language.rs                                                          # The BCP 47 subset of language: canonical case, and each refused form
│   │   │   │   ├── layer.rs                                                             # A preferences file parsed strictly: its schema marker required, every key
│   │   │   │   ├── mod.rs                                                               # The settings crate's tests, one module per source module, and the proof
│   │   │   │   ├── registry.rs                                                          # The registry: every descriptor checked once, the built-in ones included
│   │   │   │   ├── resolve.rs                                                           # Resolution: per key, an explicit --set, then the project file, then the
│   │   │   │   ├── store.rs                                                             # The file adapter: bounded reads naming the file, and whole-file writes
│   │   │   │   ├── synthetic.rs                                                         # Adding a setting is one descriptor: a synthetic one, appended to the
│   │   │   │   └── value.rs                                                             # Values: each kind read from the command line's text and from a file's
│   │   │   ├── builtin.rs                                                               # The settings Maestro ships, one descriptor each
│   │   │   ├── descriptor.rs                                                            # A setting's descriptor: everything Maestro knows about one setting, as
│   │   │   ├── discovery.rs                                                             # Where a session finds its project file (plan D6): the nearest
│   │   │   ├── language.rs                                                              # The language tags language accepts: the bounded BCP 47 subset S3 ruled
│   │   │   ├── layer.rs                                                                 # One preferences file, maestro-preferences/1, parsed strictly (plan D6
│   │   │   ├── lib.rs                                                                   # Maestro's settings: everything configurable, in one registry of
│   │   │   ├── registry.rs                                                              # The registry: the descriptors of every setting Maestro knows, each checked
│   │   │   ├── resolve.rs                                                               # Resolution (S3 FR-S3-014, plan D6): each key takes the value of the
│   │   │   ├── store.rs                                                                 # The port a session reads its preferences layers through, and its file
│   │   │   └── value.rs                                                                 # A setting's value: read from the command line's text or a file's TOML
│   │   └── Cargo.toml                                                                   # Crate manifest: Maestro's settings: the registry of every setting, its layered files and their resolution
│   ├── maestro-test-clock/                                                              # Maestro test clock
│   │   ├── src/                                                                         # The crate's sources
│   │   │   ├── lib.rs                                                                   # Clock and stage signals used only by the workspace's tests
│   │   │   ├── stage_end.rs                                                             # Watches the outcome fields of tracing stages for a held-clock release
│   │   │   └── stopped.rs                                                               # Holds Tokio time still while ordinary test work runs
│   │   └── Cargo.toml                                                                   # Crate manifest: Stopped Tokio time and stage completion signals for workspace tests
│   └── maestro-test-scratch/                                                            # Maestro test scratch
│       ├── src/                                                                         # The crate's sources
│       │   ├── lib.rs                                                                   # Where the workspace's tests create their scratch directories
│       │   └── tests.rs                                                                 # Rust source: tests
│       └── Cargo.toml                                                                   # Crate manifest: Where the workspace's tests create their scratch directories
├── docs/                                                                                # Documentation
│   ├── adr/                                                                             # Hard-to-reverse decisions, each with the trade-off that produced it
│   │   ├── 0001-fresh-start-with-canonicalization-only.md                               # Fresh start: only the canonicalization crate is carried over
│   │   ├── 0002-one-authority-many-projections.md                                       # One authority, many projections
│   │   ├── 0003-qdrant-server-for-search-projections.md                                 # Qdrant server for vector and lexical projections
│   │   ├── 0004-neo4j-for-the-graph-projection.md                                       # Neo4j Community for the graph projection
│   │   ├── 0005-copilot-native-catalog-formats.md                                       # Copilot-native catalog formats are canonical
│   │   ├── 0006-in-house-event-sourced-workflow-engine.md                               # An in-house, event-sourced workflow engine
│   │   ├── 0007-cedar-for-authorization.md                                              # Cedar for tool and operation authorization
│   │   ├── 0008-chunks-budgeted-in-embedder-tokens.md                                   # Chunks are budgeted in the selected embedder's tokens
│   │   ├── 0009-vendor-specific-material-stays-private.md                               # Vendor-specific material stays private
│   │   ├── 0010-spec-kit-and-executable-gates.md                                        # Spec Kit for delivery; evaluations and tests are the gates
│   │   ├── 0011-models-chosen-by-bake-off.md                                            # No model is preselected
│   │   ├── 0012-catalog-and-runtime-in-separate-repositories.md                         # Catalog content and runtime live in separate repositories
│   │   ├── 0013-extensions-through-events-and-operations-out-of-process.md              # Extensions plug in through events and operations, out of process
│   │   ├── 0014-strict-json-for-collection-and-source-policy.md                         # Strict JSON for collection and source-policy declarations
│   │   ├── 0015-bundle-freshness-and-revocation.md                                      # Bundles need freshness and revocation, not only signatures
│   │   ├── 0016-native-rust-desktop-workbench.md                                        # The workbench is a native Rust desktop application
│   │   ├── 0017-spec-kit-installed-once-for-the-organization.md                         # Spec Kit is installed once for the organization, not committed
│   │   ├── 0018-rustix-on-unix-and-win32-flags-on-windows.md                            # The snapshot store uses rustix on Unix and Win32 flags on Windows
│   │   ├── 0019-reverse-engineering-is-analysis-behind-a-clean-room.md                  # Reverse engineering produces knowledge only, behind a clean-room boundary
│   │   ├── 0020-rust-libraries-with-named-dependency-exceptions.md                      # Rust libraries join the stack; the duplicates they force are named exceptions
│   │   └── README.md                                                                    # Hard-to-reverse decisions, each with the trade-off that produced it
│   ├── architecture/                                                                    # Status: design of record, 2026-09-23, completed 2026-09-24
│   │   ├── 01-knowledge-pipeline.md                                                     # 01 Knowledge pipeline
│   │   ├── 02-retrieval-and-knowledge-graph.md                                          # 02 Retrieval and knowledge graph
│   │   ├── 03-agent-orchestration.md                                                    # 03 Agent orchestration
│   │   ├── 04-intelligence-backend.md                                                   # 04 Intelligence backend
│   │   ├── 05-platform-and-operations.md                                                # 05 Platform and operations
│   │   ├── 06-roadmap.md                                                                # 06 Roadmap
│   │   ├── 07-extensibility.md                                                          # 07 Extensibility: entry points, exit points and extensions
│   │   ├── 08-traceability.md                                                           # 08 Traceability
│   │   ├── 09-reverse-engineering.md                                                    # 09 Reverse engineering and provenance
│   │   └── README.md                                                                    # Status: design of record, 2026-09-23, completed 2026-09-24
│   ├── contracts/                                                                       # Contracts
│   │   └── unit-graph-v1.md                                                             # Unit graph wire contract v1
│   ├── how-to/                                                                          # How to
│   │   ├── acquisition-authority.md                                                     # Acquisition authority
│   │   └── knowledge-mcp.md                                                             # Connect a client to Maestro's knowledge MCP server
│   └── standards/                                                                       # Standards
│       ├── engineering.md                                                               # Engineering rules in maestro-core
│       ├── northstar.md                                                                 # Northstar for maestro-core
│       └── security.md                                                                  # Security rules in maestro-core
├── schemas/                                                                             # The JSON Schemas this repository publishes, each generated from its Rust type
│   └── events/                                                                          # The JSON Schema of each public event's data (07 §3.2), generated from its Rust type in journal/knowledge.rs
│       ├── knowledge.generation.published/                                              # The schemas of maestro.knowledge.generation.published, one file per major version
│       │   └── 1.json                                                                   # A generation of a collection was published: searches read it from now on
│       ├── knowledge.generation.retired/                                                # The schemas of maestro.knowledge.generation.retired, one file per major version
│       │   └── 1.json                                                                   # A published generation was retired: no search reads it, and it is kept for rollback
│       ├── knowledge.import.completed/                                                  # The schemas of maestro.knowledge.import.completed, one file per major version
│       │   └── 1.json                                                                   # An import of a collection's corpus manifest completed: what it did with its entries
│       ├── knowledge.revision.held/                                                     # The schemas of maestro.knowledge.revision.held, one file per major version
│       │   └── 1.json                                                                   # The quality gate held a revision back: it is not indexed
│       ├── README.md                                                                    # The JSON Schema of each public event's data (07 §3.2), generated from its Rust type in journal/knowledge.rs
│       └── index.json                                                                   # The committed event schemas, by name: the regeneration command refuses one whose file is gone
├── specs/                                                                               # Specifications, one directory per slice
│   ├── 000-foundation/                                                                  # 000 foundation
│   │   ├── plan.md                                                                      # Implementation Plan: Foundation
│   │   ├── spec.md                                                                      # Feature Specification: Foundation
│   │   └── tasks.md                                                                     # Foundation Implementation Tasks
│   ├── 001-knowledge-kernel/                                                            # 001 knowledge kernel
│   │   ├── checklists/                                                                  # Checklists
│   │   │   └── requirements.md                                                          # Specification Quality Checklist: Knowledge kernel and hybrid RAG
│   │   ├── plan.md                                                                      # Implementation Plan: Knowledge kernel and hybrid RAG
│   │   ├── research.md                                                                  # Research: Knowledge kernel and hybrid RAG
│   │   ├── spec.md                                                                      # Feature Specification: Knowledge kernel and hybrid RAG
│   │   └── tasks.md                                                                     # Knowledge Kernel and Hybrid RAG Implementation Tasks
│   └── 006-native-acquisition/                                                          # 006 native acquisition
│       ├── plan.md                                                                      # Native Acquisition Implementation Plan
│       ├── research.md                                                                  # S6 research: Rust-first, adaptive acquisition
│       ├── spec.md                                                                      # Feature Specification: Native acquisition
│       └── tasks.md                                                                     # Native Acquisition Implementation Tasks
├── supply-chain/                                                                        # cargo-vet audits, configuration and imports
│   ├── audits.toml                                                                      # cargo-vet audits file
│   ├── config.toml                                                                      # cargo-vet config file
│   └── imports.lock                                                                     # The audits cargo-vet imports, locked
├── tests/                                                                               # Test data shared by the workspace's crates
│   └── fixtures/                                                                        # Test fixtures
│       └── synthetic/                                                                   # The public synthetic collection and its suite, which stand in for the private corpus in public CI (ADR-0009)
│           ├── corpus/                                                                  # The collection's one source: its maestro-corpus/1 manifest beside the Markdown documents it names
│           │   ├── en/                                                                  # The documents written in English
│           │   │   ├── backups/                                                         # Backups
│           │   │   │   ├── backup-policy.md                                             # Sample document: Backup policy
│           │   │   │   └── restoring-a-database.md                                      # Sample document: Restoring a database from a backup
│           │   │   ├── databases/                                                       # Databases
│           │   │   │   └── schema-migrations.md                                         # Sample document: Running schema migrations
│           │   │   ├── http/                                                            # HTTP APIs
│           │   │   │   ├── error-codes.md                                               # Sample document: HTTP error codes
│           │   │   │   └── pagination.md                                                # Sample document: Paginating API results
│           │   │   ├── logging/                                                         # Logging
│           │   │   │   └── log-rotation.md                                              # Sample document: Log rotation
│           │   │   ├── messaging/                                                       # Messaging
│           │   │   │   └── dead-letter-queues.md                                        # Sample document: Dead-letter queues
│           │   │   ├── operations/                                                      # Operations
│           │   │   │   ├── payments/                                                    # The payments team's runbooks
│           │   │   │   │   └── on-call-handover.md                                      # Sample document: On-call handover, a near-duplicate of operations/on-call-handover.md on purpose (T023)
│           │   │   │   ├── glossary.md                                                  # Sample document: Glossary, an exact copy of en/glossary.md on purpose (T023)
│           │   │   │   └── on-call-handover.md                                          # Sample document: On-call handover
│           │   │   ├── scheduling/                                                      # Scheduling
│           │   │   │   ├── job-retries.md                                               # Sample document: Retrying scheduled jobs
│           │   │   │   └── schedule-expressions.md                                      # Sample document: Schedule expressions
│           │   │   ├── tls/                                                             # TLS certificates
│           │   │   │   ├── certificate-renewal-4.1.md                                   # Sample document: Renewing TLS certificates as release 4.1 had it, the older of two versions on purpose (T032)
│           │   │   │   └── certificate-renewal.md                                       # Sample document: Renewing TLS certificates in release 4.2, the newer of two versions
│           │   │   └── glossary.md                                                      # Sample document: Glossary
│           │   ├── fr/                                                                  # The documents written in French
│           │   │   ├── backups/                                                         # Backups
│           │   │   │   ├── politique-de-sauvegarde.md                                   # Sample document: Politique de sauvegarde
│           │   │   │   └── verification-des-sauvegardes.md                              # Sample document: Vérifier les sauvegardes
│           │   │   ├── databases/                                                       # Databases
│           │   │   │   └── migrations-sans-interruption.md                              # Sample document: Migrations sans interruption de service
│           │   │   ├── http/                                                            # HTTP APIs
│           │   │   │   ├── codes-d-erreur.md                                            # Sample document: Codes d'erreur HTTP
│           │   │   │   ├── jetons.md                                                    # Sample document: Jetons d'accès à l'API
│           │   │   │   └── limites-de-debit.md                                          # Sample document: Limites de débit
│           │   │   ├── logging/                                                         # Logging
│           │   │   │   └── journaux-structures.md                                       # Sample document: Journaux structurés
│           │   │   ├── messaging/                                                       # Messaging
│           │   │   │   ├── accuses-de-reception.md                                      # Sample document: Accusés de réception
│           │   │   │   └── ordre-des-messages.md                                        # Sample document: Ordre des messages et partitions
│           │   │   ├── operations/                                                      # Operations
│           │   │   │   └── gestion-des-incidents.md                                     # Sample document: Gestion des incidents
│           │   │   ├── scheduling/                                                      # Scheduling
│           │   │   │   ├── relance-manuelle.md                                          # Sample document: Relancer une tâche à la main
│           │   │   │   └── supervision-des-taches.md                                    # Sample document: Surveiller les tâches planifiées
│           │   │   └── tls/                                                             # TLS certificates
│           │   │       └── certificats-clients.md                                       # Sample document: Certificats clients et TLS mutuel
│           │   └── maestro-corpus.jsonl                                                 # The source's manifest: one maestro-corpus/1 line per document, with its digest and size
│           ├── evals/                                                                   # The collection's evaluation suites, as evals.suite names them: each <name>.jsonl is the suite <name>
│           │   ├── baseline.json                                                        # JSON data: baseline
│           │   └── synthetic.jsonl                                                      # The suite synthetic: one maestro-suite/1 question per line, French and English, each with the sections that answer it
│           ├── .rumdl.toml                                                              # The synthetic collection is test input, not documentation: one of its documents repeats a heading under the same parent, as authors do
│           └── collection.json                                                          # The maestro-collection/1 declaration of the public collection synthetic
├── .editorconfig                                                                        # Editor settings that survive the editor
├── .gitattributes                                                                       # How Git should treat each kind of file
├── .gitignore                                                                           # Paths git never tracks
├── .lycheeignore                                                                        # The example fixture's original.md keeps its author's relative link to assets/flow.svg, which resolves beside input.md, not beside the copy
├── .pre-commit-config.yaml                                                              # Spec Kit writes and refreshes these files in its own style; they stay as it writes them, so a refresh changes nothing by itself
├── AGENTS.md                                                                            # Rules for coding agents: what to read, what never to weaken, how to verify
├── CONTEXT.md                                                                           # The words this repository uses, and the ones it avoids
├── Cargo.lock                                                                           # Exact dependency versions, committed so every build resolves the same
├── Cargo.toml                                                                           # Workspace manifest: its members and the lints every member inherits
├── LICENSE                                                                              # The licence this repository is distributed under
├── README.md                                                                            # The local runtime of Maestro: knowledge kernel, retrieval, orchestration and the command-line tools
├── justfile                                                                             # List every recipe and what it does; this is what just alone prints
├── maestro-quality.toml                                                                 # The organization's quality rules as this repository shapes them: the inputs its CI caller passes, the seams that keep one caller
├── rust-toolchain.toml                                                                  # The pinned Rust toolchain
└── typos.toml                                                                           # Spelling checks for maintained code and documentation
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
