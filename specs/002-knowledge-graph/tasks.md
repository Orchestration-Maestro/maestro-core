# Knowledge Graph Implementation Tasks

> **For agentic workers:** use superpowers:executing-plans for the assigned
> task. The supervisor owns dispatch, review and integration; a lane starts
> no subagents. Checkboxes record integrated evidence, not worker claims.

**Goal:** M2, relationships answered with complete source proofs and measured
quality, starting with a rule-only neighbors pilot.

**Architecture:** SQLite/artifacts own claims; qualified embedded LadybugDB
projects them. Reuse S1 jobs, generations, evidence, eval, CLI and MCP. One
engine, no service or runtime SQL fallback.

**Tech Stack:** Rust 1.98.1 (MSRV 1.98), existing workspace dependencies and
owner-approved `lbug` with the pin/features qualified by G25.

**Input / prerequisites:** [spec.md](spec.md), [plan.md](plan.md), the approved
D1–D5 decisions and the integrated S1 seams. Research evidence comes from G25;
it is not a prerequisite to writing these documents or a result assumed here.

## Format and path conventions

Stable IDs preserve the draft's references; G29's second loader is removed.
**G25 is first**, but the pilot starts beside it. Tasks are dependency-ordered,
not numeric. There are **29 tasks, 108 lane-hours**, each at most four hours
including targeted gates.
Every task has a failing check, exact file targets and acceptance criteria.
Split an overrun before dispatch rather than omit a check. Native CI waits are
reported separately; a four-hour timebox does not turn pending CI into a pass.

Paths below use these exact directory bindings, with no wildcard ownership:

| Binding | Directory |
| --- | --- |
| `K` | `crates/maestro-kernel` |
| `N` | `crates/maestro-knowledge` |
| `C` | `crates/maestro` |
| `G` | `crates/maestro-knowledge/src/graph` |
| `P` | `crates/maestro-knowledge/src/graph/projection` |
| `S2` | `specs/002-knowledge-graph` |
| `PRIVATE` | Owner-bound private collection checkout, never a public path |

A listed path not in the baseline is created by that task. Existing paths are
extended. New modules also register in their parent's `mod.rs` or `lib.rs`;
child `mod.rs` files contain only declarations and re-exports. Integration
modules under `C/tests/it/` and `N/tests/it/` register in their existing
`main.rs`; do not add extra lbug-linked test binaries. These registrations and
generated guide updates are shared-file edits, not wider scope. Each migration
takes the next free number at landing, coordinated by the supervisor with S3
and deployment modes. `NNNN` means that assigned number, not a reservation.

## S2 integration workflow and global gates

Use a fresh clone from `origin/feat/s2-integration`, one signed conventional
commit per task and only the assigned lane branch. Never push the integration
branch or open an individual task PR to main. The supervisor reviews and
fast-forwards accepted work. Documentation here does not mark M1 delivered.

1. Write the **Red** cases and run the task's **Test** command. Save the failing
   output; for a research/private task, first demonstrate that the missing or
   invalid evidence is rejected. Synthetic evidence is not a real receipt.
2. Implement only the **Green** deliverable. Run focused tests and host Clippy
   while editing; no model, engine or network I/O inside a SQLite transaction.
3. Run the **Check** cases and local gates, then commit with hooks enabled.
   Push that tested signed commit with `git push --no-verify` to the lane branch.
   Report its hash, wall time, failures and gate output.
4. Full coverage, native three-OS builds/tests and mutations run on stable
   GitHub CI, not locally. Before S2 merges: at least 95% changed-line / 90%
   total coverage, zero missed mutants and zero timeouts. Review and the
   supervisor's integration check still gate a landing.

Local build environment (every Cargo command runs through the memory cap):

```sh
export PATH="$HOME/.local/bin:$HOME/.cache/maestro/tools/bin:$PATH"
export CARGO_BUILD_JOBS=3 NEXTEST_TEST_THREADS=24
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_TEST_DEBUG=line-tables-only
export CLIPPY_CONF_DIR="$HOME/.cache/maestro/org-clippy"
```

Before a code push: `capped cargo fmt --all --check`, host
`capped cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`capped cargo nextest run --workspace --locked`. Run cross-target Clippy with
G25's approved native recipe; existing bundled SQLite/ring flags alone do not
qualify lbug. The mandatory target commands are:

```sh
capped cargo clippy --workspace --all-targets --locked \
  --target x86_64-pc-windows-gnu -- -D warnings
capped cargo clippy --workspace --all-targets --locked \
  --target aarch64-apple-darwin -- -D warnings
```

Also run the installed commit hooks (Markdown, offline links, line length,
spelling, secrets, hygiene, rules and guide), `rust-gate architecture --local`,
`rust-gate duplication` and `rust-gate licenses` with the lane rule recipes.
A changed lock also requires `capped cargo vet --locked`. Dependencies belong
in root `[workspace.dependencies]`; exceptions belong in
`maestro-quality.toml`, never a hand-edited generated policy. No local full
`just check` or mutation run. A documentation-only task runs the document and
repository hooks, not unrelated Rust builds.

All public examples and process fixtures are synthetic. Keep vendor rules,
quotes, questions, prompts and reports private; use the approved local model
profile. No automatic egress, new version or unapproved dependency. A search,
answer or extraction call uses `Room::Free` and never evicts a chat model.

## Phase 1: Qualification gate

### G25 [US5] Qualify lbug before adopting it

**Time:** 4 h. **After:** none; the separate qualification lane owns this work.
**Files:** `N/tests/it/lbug_qualification.rs`, `S2/research.md`, root
`Cargo.toml`, `N/Cargo.toml`, `Cargo.lock`, `maestro-quality.toml`,
`supply-chain/config.toml`, `supply-chain/audits.toml`,
`.github/workflows/lbug-qualification.yml` (in maestro-core).
**Test:** `capped cargo nextest run -p maestro-knowledge --test it lbug_qualification`.
**Acceptance:** FR-S2-001, SC-S2-007; the six-row adoption bar in plan A1,
with actual versus planned platform evidence distinguished. No silent waiver.

- [ ] **Red.** Assert types/parameters, rollback, the single batch loader,
  per-hop filtered bounded paths and native cancellation. Spawn independent
  writer/CLI/MCP reader processes; test file locks, immutable old pins, a
  second writer and kill/reopen. Unsupported behavior fails/blocks.
- [ ] **Green.** Pin minimum features and bundled native source. Check no
  native-build network/system OpenSSL, licences and at most one named forced
  duplicate. Record metadata/feature tree, clean/warm build and binary delta,
  real-cache cost per mutation shard/coverage run and peak capped memory.
- [ ] **Check.** Apply the six-row bar: clean CI delta ≤15 minutes, unrelated
  changes never rebuild liblbug; shards fit 30 minutes and local builds 8 GiB
  at three jobs. Linux evidence plus a supervisor-approved dated Windows/macOS
  CI plan permits implementation; real native evidence and both cross-Clippy
  recipes still gate M2. Put the probe in core; any reusable toolchain release/
  re-pin is a supervisor step with its wait recorded. Publish the result in
  `S2/research.md` and the supervisor's G25 report, never infer a passed test.

**Checkpoint:** G25 gates engine tasks G11, G21, G22, G26, G27, G28 and G30.
The SQLite pilot and model/evaluation groundwork do not wait for G25 or M1.

## Phase 2: User Story 1 — the rule-only pilot

### G01 [US1] Freeze the pilot and reconcile the design

**Time:** 3 h. **After:** decided D3 and the integrated S1 head; no task dependency.
**Files:** `S2/spec.md`, `S2/plan.md`, `S2/tasks.md`,
`docs/adr/0021-embedded-ladybug-graph-projection.md`, `docs/adr/README.md`,
`docs/adr/0004-neo4j-for-the-graph-projection.md`,
`docs/adr/0020-rust-libraries-with-named-dependency-exceptions.md`,
`docs/architecture/README.md`, `docs/architecture/01-knowledge-pipeline.md`,
`docs/architecture/02-retrieval-and-knowledge-graph.md`,
`docs/architecture/04-intelligence-backend.md`,
`docs/architecture/05-platform-and-operations.md`,
`docs/architecture/06-roadmap.md`, `docs/architecture/08-traceability.md`,
`tests/fixtures/synthetic/graph/defaults.md`,
`tests/fixtures/synthetic/graph/defaults.json`, `N/tests/it/graph_fixture.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge --test it graph_fixture`.
**Acceptance:** FR-S2-004, FR-S2-021, SC-S2-001; one synthetic table fixture,
reviewed requirements and a private scope receipt reference, no vendor text.

- [ ] **Red.** Fixture checks reject mismatched digest, UTF-8 spans and
  defaults; check the requirement table equals the Acceptance lines exactly.
  Inventory ADR-0020's neo4rs wording, architecture 08's open Neo4j-first row,
  architecture 06's platform deferral until after S4, and architecture 05's
  Neo4j service row, the graph design in 02, README's graph stack, 01's
  entity-vector row and 04's B6/D02 graph references.
- [ ] **Green.** Reconcile those rows with embedded-first S2, later selected
  Neo4j, three-OS gates and the minimal typed-edge seam. Pin one table rule
  and synthetic fixture; keep private receipt content private. Allocate no
  migration numbers here. G24, not this task, finalizes ADR qualification
  against G25. Do not call the pilot M2.
- [ ] **Check.** Fixture and document hooks pass; architecture rows distinguish
  planned work from delivered evidence and name deferred graph algorithms.
  Verify existing search/ask, backup, scopes and generation seams at dispatch.

G01's frozen public rule/oracle and dispatch inspection are in plan A0 and
Starting point. The private reference `PRIVATE/graph/receipts/pilot-inputs.json`
is planned and unverified; no S1 receipt applies. G05/G07 private reads remain
blocked pending owner scope confirmation. The `graph_fixture` test checks this
fixture and the requirement map below; it does not implement G02–G04 or prove
private pilot success. Checkboxes remain for supervisor integration evidence.

### G02 [US1] Store verified immutable claims

**Time:** 4 h. **After:** G01.
**Files:** `K/src/facts/types.rs`, `K/src/facts/write.rs`,
`K/src/facts/error.rs`, `K/src/facts/tests/claims.rs`,
`K/src/facts/tests/supports.rs`, `K/src/evidence/claim_support.rs`,
`K/migrations/NNNN_graph_claims.sql`, `K/src/store/migration.rs`,
`K/src/store/tests/migrations.rs`.
**Test:** `capped cargo nextest run -p maestro-kernel facts`.
**Acceptance:** FR-S2-002, FR-S2-003; scoped claims with typed endpoints,
conditions/time/profile/review fields and immutable verified supports.

- [ ] **Red.** Reject missing/empty anchors, wrong revision/digest, out-of-range
  or split-UTF-8 spans, denied grants and replacement writes. Force rollback
  after a support failure; replay the same admitted claim idempotently.
- [ ] **Green.** Add the forward migration and one scoped write path that
  checks original bytes/eligibility independently of knowledge. Persist frozen
  membership and provenance; expose typed claim/support records for G03, not
  unscoped SQL or canonicalization types.
- [ ] **Check.** Run the Test command plus
  `capped cargo nextest run -p maestro-kernel store::tests::migrations`.
  Failed writes leave no accepted relation, fresh/reopened stores agree, and
  valid quotes never automatically promote semantic truth/review state.

### G03 [US1] Extract the first table rule

**Time:** 4 h. **After:** G02.
**Files:** `G/rules.rs`, `G/verify.rs`, `G/structure.rs`,
`G/tests/rules.rs`, `G/tests/verify.rs`, `C/src/cli/graph/build.rs`,
`C/src/cli/args.rs`, `C/src/cli/run.rs`, `C/tests/it/graph_build.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph` and
`capped cargo nextest run -p maestro graph_build`.
**Acceptance:** FR-S2-003, FR-S2-004; synthetic import → rule build produces
only correctly located `DEFAULTS_TO` claims through G02.

- [ ] **Red.** Refuse duplicate/unknown keys, changed digests, executable
  fields, misleading headings and invalid/ambiguous quotes. Identical spelling,
  normalized name, kind and collection resolves across documents; different
  colliding spellings or kinds remain ambiguous. Assert `DEFAULTS_TO` has a
  typed literal object and creates no literal/Document/Section nodes.
- [ ] **Green.** Parse the standalone closed `rule` object defined in plan A0:
  `id`, its bound `source_sha256` and explicit subject/type/lexeme columns;
  never accept the test envelope or its `expected` oracle as rule input. Build
  the temporary import declaration/manifest from A0's fixed metadata, not S1's
  corpus. Reuse block/source references and submit candidates through G02.
  Preserve literal types and source lexemes, not floating-point rewrites or
  invented entities. Carry source references without projecting structural
  nodes; retain rejections.
- [ ] **Check.** Run both Test commands; valid Unicode source slices agree
  byte-for-byte with the original. A rule/profile change changes the frozen
  inputs. The build runs without a model, script or graph service.

### G04 [US1] Return scoped neighbors from the pilot

**Time:** 4 h. **After:** G03.
**Files:** `K/src/facts/read.rs`, `K/src/facts/tests/neighbors.rs`,
`G/query.rs`, `G/tests/neighbors.rs`, `C/src/cli/graph/read.rs`,
`C/src/cli/args.rs`, `C/src/cli/run.rs`, `C/tests/it/graph_neighbors.rs`.
**Test:** `capped cargo nextest run -p maestro-kernel neighbors` and
`capped cargo nextest run -p maestro graph_neighbors`.
**Acceptance:** FR-S2-010, FR-S2-014, SC-S2-001; versioned CLI neighbors carry
relation, revision, original span/quote and explicit ambiguity/coverage.

- [ ] **Red.** Assert hidden/unknown ID parity, current grants, version
  selection, ambiguous names, deterministic ties, bounded work and cancellation
  in CLI JSON and kernel reads. No result must mean no documented graph link,
  not no relation in the corpus.
- [ ] **Green.** Add indexed SQLite one-hop reads and the CLI operation over
  frozen verified pilot claims. Keep authoritative evidence checks in the
  kernel. This is the temporary pilot path that G11 replaces, not recursion.
- [ ] **Check.** Run both Test commands and query the G01 fixture after import
  and build with plan A0's temporary declaration/manifest and version `1.0`.
  Confirm every emitted byte span/quote and no hidden title/count.

### G05 [US1] Independently check the private pilot

**Time:** 3 h. **After:** G03, G04; decided D4 and the approved private receipt.
**Files:** `PRIVATE/graph/rules/defaults.json`,
`PRIVATE/graph/receipts/pilot-inputs.json`,
`PRIVATE/graph/receipts/pilot-review.jsonl`,
`PRIVATE/graph/receipts/pilot-results.json`,
`PRIVATE/graph/receipts/scratch.json`.
**Test:** `maestro knowledge graph neighbors --collection ctm --entity "$ENTITY" --json`
for each frozen pilot entity, with plan Validation's explicit scratch bindings.
**Acceptance:** SC-S2-001, FR-S2-021; every accepted claim and its quote has an
independent review disposition; no private material is copied to core or CI.

- [ ] **Red.** Refuse missing/overlapping scratch bindings, wrong backup
  digest, changed receipt, unapproved version and a deliberately false relation.
- [ ] **Green.** Confirm the owner-approved receipt covers the subset/version/
  windows, restore a named backup into an isolated scratch kernel with private
  Qdrant ports/name pairs, then import/build there. Never open the live kernel.
  Independently check every relation, condition, revision, block, span and
  quote; retain all failures in the private receipts.
- [ ] **Check.** Run the Test query matrix, including denied/unknown and
  ambiguous names. Reconcile every input/candidate/output with its private
  receipt. Report defects rather than changing source truth to fit the rule.

**Checkpoint:** the 18-hour SQLite pilot is independently usable while G25
runs. Inspect its failures before expanding; it is not M2 acceptance.

## Phase 3: User Story 4 — freeze measurement early

### G06 [US4] Score construction, complete proofs and answers

**Time:** 4 h. **After:** G01.
**Files:** `N/src/eval/graph/labels.rs`, `N/src/eval/graph/score.rs`,
`N/src/eval/graph/tests.rs`, `N/src/eval/ladder.rs`,
`N/src/eval/bootstrap.rs`, `C/src/cli/eval/graph.rs`,
`C/src/cli/eval/runner.rs`, `C/src/cli/eval/manifest.rs`,
`C/src/cli/args.rs`, `C/src/cli/run.rs`,
`tests/fixtures/synthetic/graph/eval.jsonl`, `C/tests/it/graph_eval.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge eval::graph` and
`capped cargo nextest run -p maestro graph_eval`.
**Acceptance:** FR-S2-018, SC-S2-002, SC-S2-003, SC-S2-004; digest-bound
labels and hand-computed stage scores, using S1 statistics rather than a second
runner. Add only `eval graph check`; runs/comparisons use `eval ladder`.

- [ ] **Red.** Partial chains score zero, a complete alternative once. Reject
  duplicate families, false exact quotes, missing reviews and digest mismatch.
  With 80 independent answerable items, seed-0/2,000 paired resamples accept
  4 wins/0 losses at five points and reject 3 wins; assert 50th/1,950th bounds.
- [ ] **Green.** Add strict labels, one construction scorer and graph metrics
  to the existing ladder. `eval graph check --manifest` validates without
  inference; existing ladder rungs run and compare, keeping every attempt.
  Implement the spec's gate-rule table, not a second runner.
- [ ] **Check.** Run both Test commands. Assert same-run Qdrant-only comparison,
  all-three-runs pass logic, 95% precision point estimate, all-unanswerable
  refusal denominator and warm cohort limits. Historical G08 drift is separate.

### G07 [US4] Draft the private graph questions

**Time:** 4 h. **After:** G06; owner confirmation of suite size and source coverage.
**Files:** `PRIVATE/evals/ctm/ctm-graph.jsonl`,
`PRIVATE/evals/ctm/ctm-graph-splits.json`,
`PRIVATE/evals/ctm/ctm-graph-manifest.json`.
**Test:** `maestro eval graph check --manifest "$PRIVATE/evals/ctm/ctm-graph-manifest.json"`.
**Acceptance:** FR-S2-018, FR-S2-021; provisionally 100 FR/EN questions,
twenty per type, all held out, 80 answerable independent families. Suite size
is pending owner confirmation; pilot/synthetic development cases are separate.

- [ ] **Red.** Reject missing anchors, wrong quotes, duplicate acceptance
  families, overlap with pilot/synthetic development and missing complete proof.
- [ ] **Green.** After owner confirmation, draft twenty questions per required
  type from approved official sources. All 100 stay acceptance-only; no label
  or failure guides tuning. Label truth, conditions and alternate chains;
  require one independent family per item. Extra source versions need approval.
- [ ] **Check.** Run Test; confirm type counts, both languages, all source
  anchors and unanswerable rationales. Hand off the draft digest for G08,
  without calling the unreviewed set a golden set.

### G08 [US4] Review every chain and record the S1 baseline

**Time:** 4 h. **After:** G07; integrated S1 search/ask and eligible local card.
**Files:** `PRIVATE/evals/ctm/ctm-graph-review.jsonl`,
`PRIVATE/evals/ctm/ctm-graph-manifest.json`,
`PRIVATE/graph/receipts/baseline.json`, `PRIVATE/graph/receipts/baseline.md`.
**Test:** `maestro eval ladder --manifest "$PRIVATE/evals/ctm/ctm-graph-manifest.json"`,
with plan Validation's scratch environment.
**Acceptance:** FR-S2-018, SC-S2-002, SC-S2-004; frozen reviewed labels,
profile/input digests and S1 baseline with a fixed at-most-4B answerer.

- [ ] **Red.** Refuse a freeze with an unreviewed chain, unresolved flagged
  wording/answerability change, changed input digest or missing failed attempt.
- [ ] **Green.** Restore the named backup into the isolated scratch kernel;
  validate private Qdrant bindings and never use the live kernel. A different
  model reviews every chain; owner rulings resolve flags. Freeze source/suite
  digests and the S1 default at-most-4B answerer card by digest, including its
  current reasoning/sampling settings. Record S1 passages as the drift baseline.
- [ ] **Check.** Run Test on that scratch restore, retaining every failure.
  Neither the held-out labels nor baseline failures may guide tuning. G23 uses
  this exact answerer card but compares gain with its same-run Qdrant-only rung.

## Phase 4: User Stories 1 and 2 — finish authoritative construction

### G09 [US1, US2] Make builds resumable and attachments immutable

**Time:** 4 h. **After:** G03.
**Files:** `G/build/run.rs`, `G/build/checkpoint.rs`, `G/build/tests.rs`,
`K/src/facts/build.rs`, `K/src/facts/tests/builds.rs`,
`K/migrations/NNNN_graph_builds.sql`, `K/src/store/migration.rs`,
`C/src/cli/graph/build.rs`, `C/tests/it/graph_resume.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::build` and
`capped cargo nextest run -p maestro graph_resume`.
**Acceptance:** FR-S2-005; one frozen claim/profile attachment per generation,
leased checkpoints and bounded durable acceptance/rejection receipts.

- [ ] **Red.** Kill before/after checkpoint commit, expire/take over a lease,
  race two workers, change a profile and retry an accepted batch. Partial
  builds stay invisible and old pins cannot observe new claims.
- [ ] **Green.** Reuse kernel jobs/resources and journal progress. Separate
  extraction/verification I/O from transactional receipt writes. Attach only a
  verified frozen claim set, once; new inputs require a new generation.
- [ ] **Check.** Run Test plus `capped cargo nextest run -p maestro-kernel facts`.
  Resume produces the same claims/rejections as a clean build and rejects late
  writes by an expired worker. Limits survive restart rather than resetting.

### G10 [US2] Resolve sourced identities, versions and contradictions

**Time:** 4 h. **After:** G02, G05.
**Files:** `G/resolve.rs`, `G/tests/resolve.rs`, `K/src/facts/resolve.rs`,
`K/src/facts/tests/resolution.rs`, `K/migrations/NNNN_graph_resolution.sql`,
`K/src/store/migration.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::tests::resolve`
and `capped cargo nextest run -p maestro-kernel facts::tests::resolution`.
**Acceptance:** FR-S2-006; scoped sourced aliases/mentions, reviewed reversible
resolution and preserved temporal/conditional contradictions.

- [ ] **Red.** Same spelling/normalized name/kind/collection joins across
  documents reversibly; different colliding spellings, kinds or collections do
  not auto-merge. Test typed literals remain claim properties with no literal/
  Document/Section nodes; also conflicting defaults, version bounds, unknown
  validity and superseded claims under old pins.
- [ ] **Green.** Implement that identity rule with sourced reversible review
  decisions. Do not add fuzzy/vector linking or turn literal values into
  entities. Duplicate supporting copies are one group, not corroboration.
- [ ] **Check.** Run both Test commands; known bounds are half-open, record
  time never supplies unknown world time and current grants protect aliases
  and review records as well as claims.

## Phase 5: User Story 5 — embedded files and publication

### G26 [US5] Package and diagnose local embedded operation

**Time:** 3 h. **After:** G01, G25.
**Files:** `C/src/cli/setup/graph.rs`, `C/src/cli/setup/command.rs`,
`C/src/cli/health/graph.rs`, `C/src/cli/health/doctor.rs`,
`C/src/cli/health/status.rs`, `C/tests/it/graph_operations.rs`,
`docs/how-to/knowledge-graph.md`, `S2/research.md`.
**Test:** `capped cargo nextest run -p maestro graph_operations`.
**Acceptance:** FR-S2-022; G25's actual runtime/files packaged without a graph
server, port, Docker, JVM or first-use download; health never mutates a graph.

- [ ] **Red.** Graph `none` is disabled with zero opens/probes, unlike an
  unavailable selected graph. Relocate data and simulate permissions, locks
  and corruption. Cleanup refuses live readers and non-owned/authority files.
- [ ] **Green.** Extend existing setup/health with previewed owned directories
  and read-only diagnostics. Document qualified reader/writer ownership and
  offline rebuild/removal. Do not add a graph service manager.
- [ ] **Check.** Run Test with network unavailable. Record install/open/reopen
  time, binary size, RSS and disk from the qualified runtime; diagnostics name
  actionable repairs without deleting files or fetching assets.

### G27 [US2, US5] Expose the typed-edge port and verify unpublished projections

**Time:** 4 h. **After:** G25, G09, G10.
**Files:** `P/port.rs`, `P/writer.rs`, `P/schema.rs`, `P/tests/writer.rs`,
`P/tests/port.rs`, `K/src/facts/projection.rs`,
`K/src/facts/tests/projection.rs`, `K/migrations/NNNN_graph_projection.sql`,
`K/src/store/migration.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::projection` and
`capped cargo nextest run -p maestro-kernel facts::tests::projection`.
**Acceptance:** FR-S2-007, FR-S2-008, FR-S2-023; public application-ID
write/read operations at `crates/maestro-knowledge/src/graph/projection/port.rs`,
edge-family separation and kernel-controlled readiness. S3 C27a depends on this
port for catalog dependency edges, which never become evidence-span claims.

- [ ] **Red.** Test partial writes, wrong IDs/stamps/counts, expired leases,
  two writers and publication races. Round-trip another slice's authoritative
  typed edges without source spans; those edges must never become knowledge
  claims/proofs or cross family/scope boundaries.
- [ ] **Green.** Expose typed-edge operations on application IDs, frozen
  generations and explicit families, never engine IDs/raw Cypher. Keep lbug
  calls in `G/projection/` and `G/cypher.rs`; deployment-modes D07 later wraps
  this API in `GraphStore`. Write unpublished files only; verify IDs/families/
  digests/counts/schema/indexes after flush/close/reopen before kernel readiness.
- [ ] **Check.** Run both Test commands and a public-API consumer test. No
  engine I/O in SQLite transactions, lbug types in the public port, or changed
  pinned reads. Catalog edges require catalog authority, not evidence spans.

### G28 [US5] Load the frozen snapshot with one resumable batch loader

**Time:** 4 h. **After:** G26, G27.
**Files:** `P/import.rs`, `P/tests/import.rs`, `C/src/cli/graph/rebuild.rs`,
`C/src/cli/args.rs`, `C/src/cli/run.rs`, `C/tests/it/graph_rebuild.rs`,
`docs/how-to/knowledge-graph.md`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::projection::tests::import`
and `capped cargo nextest run -p maestro graph_rebuild`.
**Acceptance:** FR-S2-008, FR-S2-019; the single G25-qualified parameterized
batch loader fills a fresh unpublished snapshot; no COPY/CSV or second loader.

- [ ] **Red.** Test Unicode, quotes/newlines and malicious-looking bound
  values. Inject disk-full, repeat a batch, kill after native commit/before
  receipt, change the snapshot and restart before readiness; old pins survive.
- [ ] **Green.** Load the complete frozen snapshot in bounded parameterized
  transactions. Checkpoint verified IDs/digests, not blind counters; uncertain
  commits recheck durable content. Verify G27 schema/indexes/readiness and
  preserve withdrawal/retirement membership without changing published files.
- [ ] **Check.** Run both Test commands; clean/resumed IDs/digests/counts
  match. Record full rebuild time and peak disk, including G11's explicit
  scale benchmark. No model inference, Qdrant claim source or live-file copy.

G29 is removed: resuming G28 is not a separate incremental implementation.
A second loader is outside S2; proposing it later requires a supervisor-set
100,000-edge build-time ceiling registered before the measurement, and a miss.

## Phase 6: User Stories 2 and 3 — admissible paths and complete evidence

### G11 [US2] Replace pilot traversal with bounded Cypher paths

**Time:** 4 h. **After:** G04, G25, G28.
**Files:** `G/query.rs`, `G/cypher.rs`, `G/tests/cypher.rs`,
`G/tests/paths.rs`, `K/src/facts/recheck.rs`, `K/src/facts/read.rs`,
`K/src/facts/tests/recheck.rs`, `C/src/cli/graph/read.rs`,
`N/tests/it/graph_scale.rs`, `S2/research.md`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::tests` and
`capped cargo nextest run -p maestro-kernel recheck`.
**Acceptance:** FR-S2-009, FR-S2-010, SC-S2-005; LadybugDB neighbors and
shortest admissible paths, every result rechecked in the kernel.

- [ ] **Red.** Test hidden/ineligible middle hops, forbidden shortest versus
  allowed longer path, revocation after admission, cycles, fan-out, unknown
  conditions, version boundaries, depths four/five, ties, injection, native
  cancellation and locked/missing/stale graphs. Catalog edge families and
  literal defaults cannot become documentary multi-hop proof links.
- [ ] **Green.** Bind fixed Cypher templates; filter scope/eligibility/
  generation/version/conditions at every hop before ranking and limits. Bound
  native expansion and time as well as output. Recheck every entity, claim and
  support against current kernel grants/eligibility. Remove the pilot SQL
  traversal branch; retain SQLite claim/export/evidence reads only.
- [ ] **Check.** Run Test on small fixtures. Run the ignored 10,000/100,000-edge
  profile explicitly, outside per-mutant runs:
  `capped cargo nextest run -p maestro-knowledge --test it --run-ignored only graph_scale`.
  Mark that test `#[ignore]`; record build time, expansion caps and cleanup.
  Inaccessible and unknown responses match; no graph failure enables SQL or
  Neo4j fallback, and limited traversal never claims corpus-wide absence.

### G12 [US3] Version graph evidence without weakening S1

**Time:** 3 h. **After:** G04, G06.
**Files:** `K/src/evidence/graph.rs`, `K/src/evidence/tests/graph.rs`,
`K/src/evidence/tests/search_wire.rs`, `N/src/search/request.rs`,
`N/src/search/tests/handoff.rs`, `N/src/eval/graph/score.rs`,
`N/src/eval/graph/tests.rs`.
**Test:** `capped cargo nextest run -p maestro-kernel evidence` and
`capped cargo nextest run -p maestro-knowledge handoff`.
**Acceptance:** FR-S2-012; strict graph `/2` round trips and proof metadata
carried through `EvidenceInput`, with non-graph `/1` unchanged.

- [ ] **Red.** Reject graph fields in `/1`, unknown/duplicate `/2` keys,
  dangling claims/passages, duplicate supports and mixed-generation proofs.
  Ensure handoff retains every link even when intermediate candidates rank low.
- [ ] **Green.** Add versioned graph types with claims, paths, supports,
  attachment identity, coverage and known gaps. Reuse S1 provenance rather than
  copying it into another store; no permissive decoder fallback.
- [ ] **Check.** Run both Test commands and the old `/1` wire fixtures.
  Serialization/deserialization checks the entire proof contract, and an
  incomplete proof cannot be labeled complete by a caller-written flag.

### G13 [US3] Assemble and budget whole proof groups

**Time:** 4 h. **After:** G11, G12.
**Files:** `N/src/search/evidence/graph.rs`,
`N/src/search/evidence/assemble/engine.rs`,
`N/src/search/evidence/assemble/output.rs`,
`N/src/search/evidence/budget.rs`, `N/src/search/evidence/versions.rs`,
`N/src/search/evidence/tests/graph.rs`,
`N/src/search/evidence/tests/graph_budget.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge search::evidence`.
**Acceptance:** FR-S2-013; all required links, conditions, versions and citation
metadata survive assembly or become explicit sanitized gaps.

- [ ] **Red.** Rechunk source spans, revoke one intermediate support, overlap
  two supports and cross exact-fit/one-over budgets. Test different versions,
  contradictory claims and an exact counter failure; never silently use bytes
  in place of a failed exact count.
- [ ] **Green.** Map anchors into the pinned chunk set and reserve whole proof
  units through selection/merging. Charge passages plus graph metadata; keep
  source spans stable and duplicate copies from becoming extra votes.
- [ ] **Check.** Run Test; the retained proof is whole or absent with a gap.
  Every passage/claim reference resolves and no high-scoring endpoint replaces
  an omitted intermediate link. Non-graph assembly retains its baseline.

### G14 [US3] Add question-seeded R4 to the existing fusion

**Time:** 4 h. **After:** G13.
**Files:** `N/src/query/understand.rs`, `G/seeds.rs`,
`N/src/search/routes/graph.rs`, `N/src/search/fusion.rs`,
`N/src/search/request.rs`, `N/src/search/admission.rs`,
`N/src/search/route_execution.rs`, `N/src/search/orchestrate.rs`,
`N/src/search/tests/graph.rs`, `N/src/search/tests/fusion.rs`,
`C/src/cli/eval/manifest.rs`, `C/src/cli/eval/graph.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge search::tests` and
`capped cargo nextest run -p maestro cli::eval::tests::manifest`.
**Acceptance:** FR-S2-011, SC-S2-002, SC-S2-005; FR/EN question-only seeds,
complete support handoff, deterministic fusion and graph-disabled parity.

- [ ] **Red.** Assert one-based RRF/ties, no duplicate echo vote, no seed from
  dense/BM25 hits, exact disabled-search parity and unavailable/deadline
  behavior without suppressing passage routes. Graph `none` makes zero calls,
  including opens/probes, and is not reported unavailable. Record pre-fusion recall.
- [ ] **Green.** Add Graph once to the route/configuration/observation seams.
  Use exact names/identifiers/reviewed aliases; preserve proof groups through
  the existing fusion/rerank path. Simple search remains simple.
- [ ] **Check.** Run both Test commands. Missing/stale/locked/rebuilding graph
  yields a named unavailable reason and no hidden fallback. The existing
  retrieval fixture baseline does not regress when graph is disabled.

### G15 [US2, US3] Expose graph reads with whole-proof transport limits

**Time:** 4 h. **After:** G11, G13.
**Files:** `C/src/knowledge/requests.rs`,
`C/src/knowledge/operations/graph.rs`, `C/src/knowledge/output/policy.rs`,
`C/src/knowledge/output/tests.rs`, `C/src/mcp/graph_tools.rs`,
`C/src/mcp/server/handler.rs`, `C/src/cli/graph/read.rs`,
`C/src/cli/args.rs`, `C/tests/it/graph_mcp.rs`, `docs/how-to/knowledge-graph.md`.
**Test:** `capped cargo nextest run -p maestro graph_mcp` and
`capped cargo nextest run -p maestro knowledge::output`.
**Acceptance:** FR-S2-014, SC-S2-008; neighbors/path/entity-resolve/evidence-trace
share one scoped operation across CLI/MCP and return no SQL/Cypher interface.

- [ ] **Red.** Test ambiguous names, explicit version/generation, current
  revocation, timeout and hidden/unknown parity. Make an intermediate support
  the largest field in a proof crossing the complete framed 64 KiB limit.
- [ ] **Green.** Add four bounded tools and CLI equivalents over shared
  operations. Replace passage-wise truncation for graph `/2` with complete
  proof-group removal and disclosure; recheck authorization before output.
- [ ] **Check.** Run both Test commands with actual stdio framing and `/1`
  regression cases. No dangling support survives a limit. Record synthetic
  Pi/Claude Code smoke evidence; Copilot needs owner approval. Private
  responses never enter public receipts.

## Phase 7: User Stories 3 and 4 — small-model answers and extraction

### G16 [US3] Answer only from complete cited graph proofs

**Time:** 3 h. **After:** G14, G15; integrated S1 ask.
**Files:** `N/src/answer/prompt.rs`, `N/src/answer/generate.rs`,
`N/src/answer/validate.rs`, `N/src/answer/types.rs`,
`N/src/answer/tests/graph.rs`, `C/src/knowledge/operations/ask/run.rs`,
`C/tests/it/knowledge_ask.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge answer` and
`capped cargo nextest run -p maestro knowledge_ask`.
**Acceptance:** FR-S2-015, SC-S2-003, SC-S2-004; frozen at-most-4B local
answerer card/settings, uncalibrated scores and at most one retry.

- [ ] **Red.** Reject invented commands/links, incomplete citations, inferred
  transitivity, unavailable required graph proof and unresolved contradictory
  versions. A second bad answer must refuse, not request another retry.
- [ ] **Green.** Map proposed conclusions to complete proof groups before
  generation; extend existing buffered citation/command guards through final
  kernel rechecks. Keep the question's language and exact source commands.
- [ ] **Check.** Run both Test commands; no incomplete proof becomes a verified
  answer before or after wire limits. Report local answer latency and refusal
  results in later frozen evaluation; do not claim general calibration here.

### G17 [US4] Add the qualified extractor role to the model registry

**Time:** 4 h. **After:** G01; decided D2.
**Files:** `K/src/gateway/card_types.rs`,
`K/src/gateway/card_v2/validation.rs`, `K/src/model/records.rs`,
`K/src/model/write.rs`, `K/src/model/read.rs`,
`K/src/model/tests/extractor.rs`, `K/src/gateway/tests/extractor_card.rs`,
`K/migrations/NNNN_extractor_role.sql`, `K/src/store/migration.rs`.
**Test:** `capped cargo nextest run -p maestro-kernel extractor`.
**Acceptance:** FR-S2-016; distinct Extractor registration, evaluation and
selection while existing role histories, pinned cards and guards survive.

- [ ] **Red.** Upgrade a populated S1 database; test cards/evaluations/
  selections for the new role, old roles, wrong-role card selection and
  synthetic-as-real refusal. Reopen and verify unchanged card digests/pins.
- [ ] **Green.** Forward-migrate all three role CHECK constraints, preserving
  immutable records, foreign keys and real eligible evaluation guards. Extend role/card
  validation without treating Answerer qualification as Extractor qualification.
- [ ] **Check.** Run Test and `capped cargo nextest run -p maestro-kernel model`.
  A failing upgrade rolls back, old cards still read unchanged, and only a
  real qualified extractor evaluation permits production selection.

### G18 [US4] Make one bounded constrained extraction call

**Time:** 4 h. **After:** G17.
**Files:** `K/src/gateway/extract.rs`, `K/src/gateway/port.rs`,
`K/src/gateway/router.rs`, `K/src/gateway/fake.rs`,
`K/src/gateway/tests/extract.rs`, `K/src/gateway/tests/chat.rs`.
**Test:** `capped cargo nextest run -p maestro-kernel gateway`.
**Acceptance:** FR-S2-016; closed-schema extraction, pinned card/sampling,
`Room::Free` and at most 1,024 output tokens without weakening Answerer chat.

- [ ] **Red.** Inspect stub-router wire JSON and refusal paths: wrong role,
  card mismatch, invalid/partial JSON, duplicate/unknown keys, unsupported
  sampling/template controls, output overflow and insufficient room.
- [ ] **Green.** Add the constrained extraction request through the existing
  port/router and deterministic fake. The schema allows typed candidates only,
  not tools, arbitrary JSON instructions or caller-chosen authority fields.
- [ ] **Check.** Run Test; no room means no unload, invalid output admits no
  partial claims, and all existing embed/rerank/tokenize/chat tests still pass.

### G19 [US1, US4] Extract offline through the verified build pipeline

**Time:** 4 h. **After:** G09, G10, G18.
**Files:** `G/extract/windows.rs`, `G/extract/run.rs`,
`G/extract/tests.rs`, `G/build/run.rs`, `C/src/cli/graph/build.rs`,
`C/tests/it/graph_extract.rs`, `PRIVATE/graph/prompts/extract.md`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::extract` and
`capped cargo nextest run -p maestro graph_extract`.
**Acceptance:** FR-S2-017, FR-S2-003, FR-S2-005; bounded offline candidates use
G03's block/quote checks and G02's authoritative write, with retained rejections.

- [ ] **Red.** Test negation, Unicode, repeated ambiguous quotes, misleading
  source instructions, window/output/token overflow and retry after a crash.
  Rejected JSON or unsupported relation types cannot create accepted claims.
  A held-out label/failure must never influence window selection.
- [ ] **Green.** Choose windows from pilot/synthetic development failures
  only; selection receives no held-out labels/results. Freeze prompt/card/
  window/profile digests in G09 jobs. Call G18 outside transactions and retain
  candidates/rejections before advancing durable progress.
- [ ] **Check.** Run both Test commands; clean and resumed runs respect the
  same cumulative work/token budget. Public tests use synthetic prompts only;
  no answer, summary or model-invented quote becomes source evidence.

### G20 [US4] Compare rules with rules plus the 4B extractor

**Time:** 3 h. **After:** G08, G19; D2/D4, private inputs and owner-confirmed
precision reviewer protocol.
**Files:** `PRIVATE/graph/receipts/extractor-manifest.json`,
`PRIVATE/graph/receipts/extractor-attempts.jsonl`,
`PRIVATE/graph/receipts/extractor-comparison.md`,
`PRIVATE/graph/receipts/extractor-card.json`.
**Test:** `maestro eval ladder --manifest "$PRIVATE/graph/receipts/extractor-manifest.json"`,
with plan Validation's scratch environment and G06's construction scorer.
**Acceptance:** SC-S2-003, FR-S2-016, FR-S2-018; three retained runs with
independent-model semantic precision/recall, owner rulings on flags, quote
validity and tokens/throughput/VRAM. The review protocol is pending owner confirmation.

- [ ] **Red.** Refuse comparison when suite/profile/corpus digests differ,
  failed attempts are missing, synthetic evidence is called real or a candidate
  violates the precision floor despite exact quotes.
- [ ] **Green.** Restore the named backup into an isolated scratch kernel
  with private Qdrant bindings; never open the live kernel. Compare rules with
  rules-plus-Qwen3-4B there. A different model reviews every accepted held-out
  claim, and the owner resolves flags. All three precision point estimates
  must reach 95%; development tuning never sees held-out labels or failures.
- [ ] **Check.** Run Test; keep every rejection/failed attempt and select a
  card only on real eligible evidence. A recall miss at fixed precision may
  trigger a newly estimated Qwen3-8B download, not the router's Qwen3.8-27B,
  a larger answerer, targeted held-out window tuning or a lower floor.

## Phase 8: User Story 5 — recovery and native proof

### G21 [US5] Consume projection events durably in the foreground

**Time:** 4 h. **After:** G09, G25, G28.
**Files:** `G/consumer.rs`, `G/tests/consumer.rs`,
`K/src/telemetry/span.rs`, `K/src/telemetry/tests/stages.rs`,
`C/src/cli/graph/build.rs`, `C/tests/it/graph_consumer.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::tests::consumer`
and `capped cargo nextest run -p maestro graph_consumer`.
**Acceptance:** FR-S2-020; S1 cursors drive idempotent durable projection
and local telemetry effects, without a daemon or OTLP dependency.

- [ ] **Red.** Kill before effect, after effect and before ack; test replay,
  lag, hidden scopes, redaction and loss of graph files after cursor advancement.
- [ ] **Green.** Foreground catch-up schedules a full G28 build for each
  newly attached generation, not an incremental loader. Deduplicate receipts
  and ack only durable build effects. Lost files rebuild the frozen snapshot,
  regardless of cursor advancement; never trust only the tail.
- [ ] **Check.** Run both Test commands; a restart neither duplicates effects
  nor skips missing projection content. Telemetry contains no quotes/prompts or
  hidden-source identifiers and is never the authoritative progress record.

### G30 [US5] Prove deletion and rebuild produce identical results

**Time:** 4 h. **After:** G11, G13, G28, G21.
**Files:** `N/tests/it/ladybug_rebuild.rs`,
`N/tests/it/ladybug_rebuild/fixture.rs`, `N/tests/it/ladybug_rebuild/compare.rs`,
`N/tests/it/ladybug_rebuild/processes.rs`, `S2/research.md`.
**Test:** `capped cargo nextest run -p maestro-knowledge --test it ladybug_rebuild`.
**Acceptance:** FR-S2-019, SC-S2-006; actual disposable-file deletion and exact
ordered semantic equality, not only equal node counts or query success.

- [ ] **Red.** Freeze kernel/queries and save neighbors, paths, claims,
  supports, evidence and coverage. Ensure the oracle detects reordered paths,
  changed quote/span/conditions, missing contradictions and wrong generations.
- [ ] **Green.** Drain readers/close handles, delete only owned disposable
  LadybugDB files and rebuild from SQLite/artifacts. Compare with the saved
  results, excluding only timing/transport IDs. Repeat G28 after interrupted
  batches, not a second loader. Use no model, graph backup or Qdrant authority.
- [ ] **Check.** Run Test across collections, retained generations, hidden
  hops and version conditions. Prove deletion really occurred and other
  collections/kernel/artifacts survived; record process/pin evidence.

### G22 [US3, US5] Restore the authority and repeat the native drill

**Time:** 4 h. **After:** G14, G15, G16, G19, G30.
**Files:** `C/tests/it/graph_restore.rs`,
`C/tests/it/graph_processes.rs`, `C/src/cli/backup/restore.rs`,
`C/src/failure.rs`, `K/src/store/error.rs`, `K/src/store/tests/migrations.rs`,
`N/tests/it/ladybug_rebuild.rs`, `.github/workflows/integration.yml`,
`.github/workflows/lbug-qualification.yml`.
**Test:** `capped cargo nextest run -p maestro graph_restore` and
`capped cargo nextest run -p maestro graph_processes`.
**Acceptance:** FR-S2-019, SC-S2-006, SC-S2-007; SQLite/artifact restore →
G30 rebuild → evidence equality, plus native writer/CLI/MCP process CI.

- [ ] **Red.** Test missing/corrupt artifacts, partial restore, denied
  supports, old pins and interrupted builds. Retained S1 must refuse an S2
  scratch database with `UnknownMigration` and no mutation. A graph alone
  cannot repair authority; test current kernel/CLI version-mismatch guidance.
- [ ] **Green.** Add native Linux/Windows/macOS process cases to the core
  workflow/drill. Its recovery receipt and diagnostic name the next action:
  update the binary or restore the named pre-upgrade backup. Add that guidance
  to current errors; do not claim the historical S1 executable prints it.
  Reusable toolchain release/re-pin waits belong to the supervisor.
- [ ] **Check.** Run both Test commands plus
  `capped cargo nextest run -p maestro-kernel store::tests::migrations`.
  Observe native CI, crash/reader-drain evidence and unchanged authority after
  S1 refusal. Queued jobs are not passes; public CI uses no vendor fixtures.

## Phase 9: User Story 4 — M2 acceptance, then release

### G23 [US4] Measure the three retrieval variants against fixed gates

**Time:** 3 h. **After:** G08, G16, G20; owner confirmation of both provisional gates.
**Files:** `PRIVATE/graph/receipts/m2-manifest.json`,
`PRIVATE/graph/receipts/m2-attempts.jsonl`,
`PRIVATE/graph/receipts/m2-comparison.md`,
`PRIVATE/graph/receipts/ctm-retrieval-regression.json`.
**Test:** `maestro eval ladder --manifest "$PRIVATE/graph/receipts/m2-manifest.json"`,
with plan Validation's scratch environment.
**Acceptance:** FR-S2-018, SC-S2-002, SC-S2-003, SC-S2-004, SC-S2-005;
Qdrant-only, LadybugDB-only and pairing on frozen inputs without duplicate
embeddings. Every D3 floor is a gate, not a report-only aspiration.

- [ ] **Red.** Refuse mismatched generator/card/context/sampling/suite inputs,
  omitted failures and a manifest that tunes on held-out families. Keep cold,
  warm and unavailable latency cohorts separate.
- [ ] **Green.** Restore the named backup into the isolated scratch kernel,
  validate private Qdrant bindings and run all rungs on that same binary/input.
  Use G08's frozen at-most-4B card/settings; repeat stochastic work three times
  and retain every attempt. Compare pairing with same-run Qdrant-only; G08 is
  only a drift check. Rerun `ctm-retrieval` on these same-run rungs.
- [ ] **Check.** Run Test; every run must independently pass: ≥5-point proof
  gain with the seed-0/2,000-resample 50th delta strictly above zero, no
  Recall@10/MRR@10/supported-answer point-estimate loss, relation precision
  ≥95%, exact spans/quotes/commands and ≥16/20 correct refusals. Warm private
  p95: graph span/tools ≤500 ms, search <2.5 s, ask <10 s. Missing, failing or
  inconclusive evidence blocks M2. Do not average away a failure or retune.

### G24 [US1, US2, US3, US4, US5] Map evidence and release M2

**Time:** 3 h. **After:** G23; every other task integrated and CI fixes complete.
**Files:** `S2/acceptance.md`, `S2/tasks.md`, `S2/research.md`,
`docs/architecture/08-traceability.md`, `docs/how-to/knowledge-graph.md`,
`docs/adr/0021-embedded-ladybug-graph-projection.md`,
`docs/adr/0004-neo4j-for-the-graph-projection.md`, `docs/adr/README.md`.
**Test:** requirement-by-requirement evidence check plus the final CI gate and
synthetic Pi/Claude Code smoke commands in `docs/how-to/knowledge-graph.md`.
**Acceptance:** SC-S2-001, SC-S2-002, SC-S2-003, SC-S2-004, SC-S2-005,
SC-S2-006, SC-S2-007, SC-S2-008; supervisor-approved release evidence.

- [ ] **Red.** Acceptance check rejects a missing requirement, duplicate map,
  pending/failed G25 or native run, stale commit receipt, lowered target,
  unproved equality or any unmet M2 exit. Design status is not code evidence.
- [ ] **Green.** Map every FR-S2/SC-S2 and its architecture portion to an
  integrated commit/test/receipt or an explicit blocker; keep private content
  and receipt bodies private. Record packaging, process, rebuild and client
  evidence and remaining named deferrals. Finalize ADR-0021 against G25's
  actual evidence here, not in G01.
- [ ] **Check.** Verify final CI coverage/mutation/three-OS gates and G25
  against the release commit. Owner authorizes live migration only after a
  named backup and the `maestro-s1`/client update. Obtain supervisor release
  approval; lanes never merge, deploy or upgrade the owner's kernel.

## Dependencies and parallel opportunities

The **After** line is authoritative. Every G dependency appears earlier above;
external private/CI evidence is named separately; M1 release is not a wait. G16 additionally waits for
G15 so answer validation cannot outrun the whole-proof transport contract;
G30 waits for G13 so its equality oracle can compare assembled graph evidence.
G24 waits for every task, not only the quality report.

| Once these land | Independent work, subject to shared-file ownership |
| --- | --- |
| G01 | G02 claims, G06 scoring and G17 model role start beside G25; G26 alone also needs G25. |
| G03 | G04 pilot reads and G09 resumable builds; do not edit CLI registration concurrently. |
| G06 | G07/G08 private suite and baseline can proceed beside public construction. |
| G27 | G28's single loader and S3 C27a's catalog-edge consumer use the shared typed-edge port; coordinate its public contract. |
| G13 | G14 R4 and G15 tools can proceed with a shared contract, then G16. |

These are opportunities, not permission to write shared files in parallel.
The supervisor assigns at most six lanes and owns common module, manifest,
lockfile, migration and CLI-registration conflicts. Model/GPU measurements and
writers serialize on their actual resources. Each task is one reviewable
behavior, not a framework assembled before the first useful result.

## Requirement coverage

Generated from each task's **Acceptance** line, not inferred extra ownership.
These are planned owners, not delivered statuses. G01 checks exact equality in
`crates/maestro-knowledge/tests/it/graph_fixture.rs`; missing, extra and duplicate
map rows fail. Owners stay in task/document order, not numeric order (for
example `G28, G30, G22`); only `### GNN [US…]` headings define tasks. G24
replaces planning claims with integrated evidence/blockers before release.

| Requirement | Tasks |
| --- | --- |
| FR-S2-001 | G25 |
| FR-S2-002 | G02 |
| FR-S2-003 | G02, G03, G19 |
| FR-S2-004 | G01, G03 |
| FR-S2-005 | G09, G19 |
| FR-S2-006 | G10 |
| FR-S2-007 | G27 |
| FR-S2-008 | G27, G28 |
| FR-S2-009 | G11 |
| FR-S2-010 | G04, G11 |
| FR-S2-011 | G14 |
| FR-S2-012 | G12 |
| FR-S2-013 | G13 |
| FR-S2-014 | G04, G15 |
| FR-S2-015 | G16 |
| FR-S2-016 | G17, G18, G20 |
| FR-S2-017 | G19 |
| FR-S2-018 | G06, G07, G08, G20, G23 |
| FR-S2-019 | G28, G30, G22 |
| FR-S2-020 | G21 |
| FR-S2-021 | G01, G05, G07 |
| FR-S2-022 | G26 |
| FR-S2-023 | G27 |
| SC-S2-001 | G01, G04, G05, G24 |
| SC-S2-002 | G06, G08, G14, G23, G24 |
| SC-S2-003 | G06, G16, G20, G23, G24 |
| SC-S2-004 | G06, G08, G16, G23, G24 |
| SC-S2-005 | G11, G14, G23, G24 |
| SC-S2-006 | G30, G22, G24 |
| SC-S2-007 | G25, G22, G24 |
| SC-S2-008 | G15, G24 |

## Minimal-first delivery strategy

1. Run qualification beside the SQLite pilot; inspect exact-source failures.
2. Freeze measurement before tuning; finish immutable claims and safe files.
3. Replace pilot traversal; carry whole proofs to tools before graph answers.
4. Add only bounded offline extraction, then prove recovery and measured gain.
5. Release only on the evidence map and fixed M2 gates. Edge, fuzzy linking,
   communities, general calibration and other slices remain separate.
