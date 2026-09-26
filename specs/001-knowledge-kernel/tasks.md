# Knowledge Kernel and Hybrid RAG Implementation Tasks

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task by task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** a developer or an agent asks a Control-M question from Pi, Codex,
Claude Code or Copilot CLI and receives cited passages from the full corpus,
with measured quality (M1 "Ask Control-M").

**Architecture:** `maestro-kernel` (building blocks on SQLite and
content-addressed artifacts), `maestro-knowledge` (the pipeline, search, `ask`
and evaluation) and `maestro` (CLI and MCP server), with Qdrant as the search
projection and the model router serving every model. See [plan.md](plan.md)
for the decisions (D1–D16) and the research (R1–R8).

**Tech Stack:** Rust 1.98.1 (MSRV 1.85, then 1.88 from T034), rusqlite 0.40,
qdrant-client 1.19, rmcp 3.4, tokio 1.53, reqwest 0.13, clap 4.6, schemars 1.2,
Qdrant 1.19, `maestro-model-router`.

**Spec:** [spec.md](spec.md) · **Plan:** [plan.md](plan.md)

## Parallel delivery

The 40 tasks run in **13 waves**. Every task that shares its wave is marked
`[P]` and runs beside the others of its wave: they own different files and
none waits for another. A task starts as soon as the tasks in its **After** line have merged,
even before the rest of its wave. Up to **four agents** work at once, one task
each, because pull-request review, not code, is the limit.

| Wave | Tasks, in parallel | Starts when |
| --- | --- | --- |
| 1 | T001 kernel artifacts · T002 router free room · T003 private mapping · T004 BM25 spike | Now |
| 2 | T005 database · T006 model gateway · T007 capabilities · T008 rerank spike · T009 knowledge contracts | T001 (T008: T002) |
| 3 | T010 journal · T011 scopes · T012 records · T013 counting seam · T014 synthetic collection · T040 lexical analyzer | T005 (T011: T010; T013: now; T014 and T040: T009) |
| 4 | T015 public events · T016 jobs · T017 evidence · T018 router tokenizer · T019 import | Their After lines |
| 5 | T020 quality and real import · T021 evaluation runner · T022 the binary | Their After lines |
| 6 | T023 prepare · T024 golden set · T025 setup and doctor | Their After lines |
| 7 | T026 Qdrant projection · T027 the owner's 30 questions | Their After lines |
| 8 | T028 publish · T029 routes and fusion · T030 bake-off | Their After lines |
| 9 | T031 reranking · T032 evidence assembly · T033 backup and restore | Their After lines |
| 10 | T034 MCP and search · T035 `ask` · T036 the synthetic gate | Their After lines |
| 11 | T037 the ladder and the first publication | T028, T030, T031, T035 |
| 12 | T038 four agents ask Control-M | T034, T037 |
| 13 | T039 measure, map and release | Everything |

**Critical path**, twelve tasks: T001 → T005 → T012 → T019 → T020 → T023 →
T026 → T029 → T031 → T037 → T038 → T039. The other 28 tasks run beside it.
T023 also waits on the counting seam, T013, which can start at once: #14,
what it waited for, has merged.

**Waits outside the code:** the owner's go to redeploy the router (T002), check of 30 questions (T027) and approval of model downloads
(T030); the workstation's GPU for the measurements (T008, T030, T037).

### Rules that keep parallel work from colliding

1. One task, one branch, one pull request: `feat/s1-t005-kernel-store`,
   `docs/s1-t004-bm25-spike`… (the organization's `branch-names` ruleset wants
   the type first).
2. A task changes only the files in its **Files** line, plus the shared files
   below.
3. Shared files are append-only: a crate's `lib.rs` module lines, workspace
   members and `[workspace.dependencies]`, `Cargo.lock` (regenerated after a
   rebase), `.cargo/mutants.toml`, the `[typos] words` of
   `maestro-quality.toml` (`rust-gate sync` writes `typos.toml` from them), the
   Copilot guide (`rust-gate guide`, regenerated after a rebase), and
   `tasks.md`, where a task ticks only its own steps.
4. Migration numbers are reserved below, so parallel tasks never take the same
   one; the store applies them by number and records each by name (D1), so
   the merge order of a wave does not matter.
5. Before merging: rebase on `main`, then `just check` again. `just check` is
   `rust-gate ci --local`: it checks the committed branch as CI will, so
   commit first; the pre-push hook runs it too.

| Migration | Task | Tables |
| --- | --- | --- |
| `0001` | T005 | `artifacts` |
| `0002` | T010 | `events`, `cursors` |
| `0003` | T011 | `scopes`, `grants` |
| `0004` | T012 | the document and generation tables of 01 §11 |
| `0005` | T016 | `jobs` |
| `0006` | T021 | `eval_reports` |
| `0007` | T030 | `model_cards` |

## Global Constraints

- Every task starts with a failing test, seen failing, and ends with
  `just check` passing and the diff mutation-tested (ENF-005).
- Crates appear only with their first working behaviour; a trait appears only
  for a real variation (P-004, P-005).
- The workspace lints hold everywhere: no `unwrap`, `expect` or `panic`
  outside tests, every item documented, files at most 500 counted lines.
- New code meets the organization's generated lints and source rules
  (rust-workflows v4.3.0) from its first commit: a `mod.rs` holds only `mod`
  and `use` lines, no import cycle, no file over 500 lines of code.
- Every crate builds and passes its tests on Linux, macOS and Windows
  (ADR-0018); a platform difference sits in a small function every host
  tests. Clippy for the other two runs locally with the gate's settings:
  `CLIPPY_CONF_DIR=<dir> cargo clippy --target x86_64-pc-windows-gnu` and
  `--target aarch64-apple-darwin`.
- No Control-M text, personal path or secret in this repository: corpus,
  golden set and `ctm-*` reports stay in the private collection (FR-S1-016).
- A search never unloads a chat model (FR-S1-015a); no measurement does
  either.
- Commits and pull requests follow the organization's rules.

`CORE` is this repository, `ROUTER` is `maestro-model-router` and `PRIVATE` is
the private collection repository.

---

## Phase 1: Wave 1 — start now

### T001 [P] The kernel crate, starting with the artifact store [US2]

**After:** nothing. **Files:** `crates/maestro-kernel/{Cargo.toml, src/lib.rs,
src/paths.rs, src/artifact.rs}`, `Cargo.lock`. **Requirements:** FR-S1-001
(B3).

- [x] **Step 1: Failing tests.** `put` returns the SHA-256 digest and stores
  the bytes under `sha256/<2>/<2>/<digest>`; `get` returns them and refuses a
  file whose bytes no longer match; a second `put` of the same bytes writes
  nothing; a digest that is not 64 lowercase hex characters is refused before
  any path is built.
- [x] **Step 2: Create the crate** with the workspace lints; its only
  dependency is `sha2`, already in the lockfile.
- [x] **Step 3: Implement** `paths` (XDG data home, `~/.local/share` fallback)
  and `artifact` (temporary file, flush, rename, directory flush; verify on
  read).
- [x] **Step 4: Gate.** `just check` passes; `cargo mutants` on the diff leaves
  no survivor.
- [x] **Step 5: Pull request** `feat: add the kernel's content-addressed
  artifact store`.

### T002 [P] The router loads into free room on request [US1]

**After:** nothing. **Files:** `ROUTER`: admission, the proxy head, README.
**Requirements:** FR-S1-015a, R2.

- [x] **Step 1: Failing test in `ROUTER`.** With the card full and an idle
  model loaded, a request carrying `X-Model-Router-Room: free` is refused with
  `503 insufficient_room` and the idle model stays loaded; without the header,
  admission behaves as today.
- [x] **Step 2: Implement** the header in admission and document it in the
  router's README.
- [x] **Step 3: Gate** (`just check` in `ROUTER`) and pull request `feat: load
  a model only into free room on request`.
- [ ] **Step 4: Deploy** the new router on the workstation and confirm the
  behaviour against the real card.

### T003 [P] The private corpus mapping and declaration [US2]

**After:** nothing. **Files:** `PRIVATE`: `export.jq`, `collection.json`,
`justfile`. **Requirements:** FR-S1-016, D15, R4.

- [x] **Step 1: Write `export.jq`** mapping the seven keys of the Python
  manifest to `maestro-corpus/1` (D15), leaving out what the corpus does not
  carry.
- [x] **Step 2: Check it** on the full manifest: 7,988 lines out, each with
  every required key and no unknown key, none invented.
- [x] **Step 3: Declare `ctm`** in `collection.json` with its source, bindings
  and scope tags.
- [x] **Step 4: Pull request** in `PRIVATE` `feat: map the corpus to
  maestro-corpus/1`.

### T004 [P] Spike: Qdrant's BM25 and the French and English policy [US1]

**After:** nothing. **Files:** `specs/001-knowledge-kernel/plan.md` (R7).
**Requirements:** FR-S1-004, R7.

- [x] **Step 1: Run Qdrant 1.19** on the workstation, pinned by version and
  checksum.
- [x] **Step 2: Index a public sample** in French and English with
  server-side BM25, and query it with accents, plurals and identifiers.
- [x] **Step 3: Record the verdict** in R7 with the commands and their
  output: server-side BM25, or client-side sparse vectors.

## Phase 2: Wave 2 — on the kernel crate

### T005 [P] The kernel database and its migrations [US2]

**After:** T001. **Files:** `crates/maestro-kernel/src/store.rs`,
`crates/maestro-kernel/migrations/0001_artifacts.sql`, and `Store::remove` in
`artifact.rs`. **Requirements:** FR-S1-001, D1.

- [x] **Step 1: Failing tests.** A new database is created in WAL mode and
  migrated; reopening applies nothing; migrations apply by number and are
  recorded by name, so one merged out of order still applies; a database
  holding a migration the binary does not know is refused.
- [x] **Step 2: Implement** the store: one writer connection behind a mutex,
  readers apart, `busy_timeout` 5 s, foreign keys on.
- [x] **Step 3: Index artifacts** from T001 in `artifacts`, with pins and a
  garbage collection that lists before it deletes.
- [x] **Step 4: Gate and pull request** `feat: add the kernel database and its
  migrations`.

### T006 [P] The model gateway and model cards [US3]

**After:** T001. **Files:** `crates/maestro-kernel/src/gateway.rs`.
**Requirements:** FR-S1-014, D8.

- [x] **Step 1: Failing tests** against a stub router: embeddings, reranking,
  tokenization and chat calls each carry their model card; a card that does
  not match what the model's server reports (`/props`: its build and chat
  template) is refused before any call; `503 insufficient_room` becomes an
  `Unavailable` result with the router's reason.
- [x] **Step 2: Implement** model cards (strict JSON artifacts) and the
  gateway.
- [x] **Step 3: The deterministic fake** embedder, reranker and generator CI
  uses, behind the same port.
- [x] **Step 4: Gate and pull request** `feat: reach models only through
  cards`.

### T007 [P] Capability registry and telemetry [US6]

**After:** T001. **Files:** `crates/maestro-kernel/src/{capability,
telemetry}.rs`. **Requirements:** FR-S1-001 (B9, B11).

- [x] **Step 1: Failing tests.** A tool registers with its schema, effects and
  required scopes; registering a name twice is refused; `health()` reports
  each component as ready, degraded or down with a reason.
- [x] **Step 2: Implement** the registry and the telemetry helpers (spans with
  pinned attribute names; no OTLP export in S1, see the plan's dependencies).
- [x] **Step 3: Gate and pull request** `feat: register capabilities and
  report health`.

### T008 [P] Spike: the cost of reranking on the card [US1]

**After:** T002 deployed. **Files:** `specs/001-knowledge-kernel/plan.md` (R8).
**Requirements:** FR-S1-005, SC-S1-004, R8.

- [ ] **Step 1: Confirm** the router refuses rather than unloads when room is
  short, so the measurement never costs a chat model.
- [ ] **Step 2: Measure** `/v1/rerank` at 20, 80 and 120 pairs of public text,
  30 runs each, batched: p50 and p95.
- [ ] **Step 3: Record** in R8 the depth that leaves room for retrieval inside
  1.5 s.

### T009 [P] The knowledge crate: collection and corpus contracts [US2]

**After:** T001. **Files:** `crates/maestro-knowledge/{Cargo.toml, src/lib.rs,
src/collection.rs, src/corpus.rs}`, `crates/maestro-kernel/src/binding.rs`,
and `config_dir` in `paths.rs`. **Requirements:** FR-S1-002, ADR-0014.

- [x] **Step 1: Failing tests.** A valid `maestro-collection/1` parses;
  unknown or duplicate keys, a source ID declared twice and numbers out of
  range are refused (no field of S1's declaration names another declared
  name, so ADR-0014's dangling-reference check arrives with the first one,
  in S6's source policy; a file the declaration names is checked when first
  read, so the quality ledger and the suite may not exist yet); a
  `maestro-corpus/1` line parses and one with an unknown key is refused; a
  missing binding is a typed refusal before any work.
- [x] **Step 2: Create `maestro-knowledge`** and implement the declaration,
  the corpus line and the bindings file.
- [x] **Step 3: Gate and pull request** `feat!: parse collections and corpus
  manifests` (the kernel's `Environment` gains the configuration variables
  and becomes non-exhaustive).

## Phase 3: Wave 3 — on the database

### T010 [P] The journal: events, streams and cursors [US2]

**After:** T005. **Files:** `crates/maestro-kernel/src/journal.rs`,
`migrations/0002_journal.sql`. **Requirements:** FR-S1-008a, D3.

- [ ] **Step 1: Failing tests.** Recording an event gives it a ULID and the
  next sequence of its stream; sequences never skip or repeat under
  concurrent writers; a cursor survives a reopen; `ack` never moves a cursor
  backwards.
- [ ] **Step 2: Implement** `record`, `events(filter)`, `cursor` and `ack`.
- [ ] **Step 3: Crash test.** An event committed before a simulated crash is
  read after the reopen; one not committed is not.
- [ ] **Step 4: Gate and pull request** `feat: journal events with durable
  cursors`.

### T011 [P] Scopes and grants [US1]

**After:** T005, T010. **Files:** `crates/maestro-kernel/src/scope.rs`,
`migrations/0003_scopes.sql`. **Requirements:** FR-S1-006, D4.

- [ ] **Step 1: Failing tests.** A principal sees only granted scopes and
  their descendants; an unknown scope is no access; a grant is journaled with
  its actor; there is no read function without a `ScopeSet`.
- [ ] **Step 2: Implement** scopes, grants, `visible` and the `ScopeSet` type.
- [ ] **Step 3: Grants from configuration.** The local principal's grants come
  from `config.toml`; a missing file grants nothing.
- [ ] **Step 4: Gate and pull request** `feat: scope every kernel read`.

### T012 [P] Document and generation records [US2]

**After:** T005. **Files:** `crates/maestro-kernel/src/{document,
generation}.rs`, `migrations/0004_documents.sql`. **Requirements:** FR-S1-001
(B5, B6).

- [ ] **Step 1: Failing tests.** Revisions are immutable once recorded; a
  failed revision stays inspectable and never eligible; a generation moves
  only through `building`, `verified`, `published`, `retired`; exactly one
  generation per collection is published.
- [ ] **Step 2: Implement** the tables of 01 §11 and their record types.
- [ ] **Step 3: Gate and pull request** `feat: record documents and
  generations`.

### T013 [P] A token-counting seam in canonicalization [US2]

**After:** nothing (#14 merged on 2026-09-25). **Files:**
`crates/maestro-canonicalization/src/{tokenizer,chunks}/`. **Requirements:**
FR-S1-003, D7.

- [x] **Step 1: Failing test.** `chunk_documents` accepts any `TokenCounter`;
  the test helper `chunk_with_count` becomes a test implementation of the
  trait.
- [x] **Step 2: Implement** the trait; `NativeTokenizer` implements it; every
  existing test and fixture byte is unchanged.
- [x] **Step 3: Native run.** The four native tests pass with the local
  binding.
- [x] **Step 4: Gate and pull request** `feat: count chunk tokens through a
  trait`.

### T014 [P] A public synthetic collection and suite [US3]

**After:** T009. **Files:** `tests/fixtures/synthetic/`. **Requirements:**
SC-S1-007.

- [ ] **Step 1: Write** a synthetic collection on non-vendor topics, written
  for the purpose, as `maestro-corpus/1` with its Markdown.
- [ ] **Step 2: Label** its questions: French and English, some unanswerable,
  each with its expected sections.
- [ ] **Step 3: Failing test, then passing.** Every fixture parses under the
  contracts of T009.
- [ ] **Step 4: Gate and pull request** `test: add the public synthetic
  collection`.

### T040 [P] The lexical analyzer `bm25-en-fr/1` [US1]

**After:** T009. **Files:** `crates/maestro-knowledge/src/lexical/`.
**Requirements:** FR-S1-004, FR-S1-005a, R7.

- [ ] **Step 1: Failing tests** on T004's public sample
  ([research.md](research.md#the-sample-and-its-checks)): all 23 checks pass,
  scored with BM25 (k 1.2, b 0.75) and IDF over the sample, with no language
  given for any passage or query, since the corpus declares none and nothing
  is guessed; the same text always gives the same vector.
- [ ] **Step 2: Implement** `bm25-en-fr/1`: identifiers kept whole as well as
  split into their parts, French and English forms that meet with or without
  their accents, stopwords, BM25 term weights with the generation's average
  length, and a stable 32-bit token ID. A change to any rule is a new profile
  version.
- [ ] **Step 3: Gate and pull request** `feat: analyze text for the lexical
  route`.

## Phase 4: Wave 4 — the journal and the records at work

### T015 [P] Public knowledge events and their schemas [US2]

**After:** T010. **Files:** `crates/maestro-kernel/src/journal/envelope.rs`,
`schemas/events/`. **Requirements:** FR-S1-008b.

- [ ] **Step 1: Failing tests.** An event reads back as a CloudEvents 1.0
  envelope with `maestrosequence` and `maestroscope`; the four knowledge
  events have schemas generated from their types; removing a field from a
  type fails the compatibility test against the committed schema.
- [ ] **Step 2: Implement** the envelope and the four event types.
- [ ] **Step 3: Commit the schemas** under `schemas/events/` and the
  compatibility test that guards them.
- [ ] **Step 4: Gate and pull request** `feat: publish the knowledge events
  with their schemas`.

### T016 [P] Jobs with leases [US2]

**After:** T010. **Files:** `crates/maestro-kernel/src/job.rs`,
`migrations/0005_jobs.sql`. **Requirements:** FR-S1-011, D5.

- [ ] **Step 1: Failing tests.** A second lease on the same idempotency key is
  refused; an expired lease can be taken over; a retried command returns the
  existing job; states move only forward.
- [ ] **Step 2: Implement** jobs, leases, heartbeats and outcomes, with
  progress in the journal.
- [ ] **Step 3: Resume test.** A job interrupted mid-way resumes from its last
  journaled progress.
- [ ] **Step 4: Gate and pull request** `feat: run long work as leased jobs`.

### T017 [P] Evidence bundles [US1]

**After:** T012. **Files:** `crates/maestro-kernel/src/evidence.rs`.
**Requirements:** FR-S1-001 (B7).

- [ ] **Step 1: Failing tests.** `resolve(chunk)` returns the exact text from
  the authority with its digest, span and version; a digest mismatch is an
  error; a bundle serializes to `maestro-evidence/1` and back unchanged.
- [ ] **Step 2: Implement** the bundle types and `resolve`.
- [ ] **Step 3: Gate and pull request** `feat: resolve evidence from the
  kernel`.

### T018 [P] The router tokenizer and its parity [US2]

**After:** T006, T009, T013. **Files:**
`crates/maestro-knowledge/src/prepare/router_tokenizer.rs`.
**Requirements:** FR-S1-003, ADR-0008.

- [ ] **Step 1: Failing tests** against the gateway's fake: token IDs come
  back in order; the contract ID changes with the model card or the router
  build.
- [ ] **Step 2: Implement** `RouterTokenizer` over `/models/<id>/tokenize`.
- [ ] **Step 3: Parity run.** For every fixture of the native profile, the
  router's ordered IDs equal the native counter's (explicit local test); a
  disagreement refuses to qualify.
- [ ] **Step 4: Gate and pull request** `feat: count tokens through the
  router`.

### T019 [P] Import a corpus manifest [US2]

**After:** T009, T010, T011, T012. **Files:**
`crates/maestro-knowledge/src/import.rs`. **Requirements:** FR-S1-002,
SC-S1-001.

- [ ] **Step 1: Failing tests** on a synthetic manifest: a digest mismatch
  refuses that entry with both digests and the rest continue; a malformed
  line is refused with its number; two lines sharing a `source_ref` with
  different digests are both held with the reason, never one silently
  replacing the other; `path` resolves against the manifest's directory; a
  second import reports every revision `unchanged` and writes nothing;
  memory stays bounded by the largest document.
- [ ] **Step 2: Implement** the streaming import: record the collection, its
  sources and scope tags; parse, verify and canonicalize each line; store
  originals and canonical documents as artifacts; record revisions.
- [ ] **Step 3: Report** imported, unchanged and refused counts as JSON and as
  `import.completed`.
- [ ] **Step 4: Gate and pull request** `feat: import corpus manifests`.

## Phase 5: Wave 5 — the corpus in the kernel

### T020 [P] The quality gate, and `ctm` imported for real [US2]

**After:** T019, T003. **Files:** `crates/maestro-knowledge/src/quality.rs`;
`PRIVATE`: the disposition report. **Requirements:** FR-S1-002a, SC-S1-001.

- [ ] **Step 1: Failing tests.** Each rule of 01 §4 gives its disposition with
  rule IDs and reasons; only `accepted` and `accepted_with_warnings`
  revisions are eligible; a held revision emits `revision.held`; a missing
  quality ledger reads as an empty one.
- [ ] **Step 2: Implement** the gate and `knowledge quality`.
- [ ] **Step 3: Import `ctm` for real:** all 7,988 documents accounted for.
- [ ] **Step 4: Run the gate on `ctm`** and keep the disposition report in
  `PRIVATE`.
- [ ] **Step 5: Gate and pull request** `feat: give every revision a quality
  disposition`.

### T021 [P] The evaluation runner [US3]

**After:** T017. **Files:** `crates/maestro-knowledge/src/eval/`,
`migrations/0006_eval_reports.sql`. **Requirements:** FR-S1-009, SC-S1-008.

- [ ] **Step 1: Failing tests.** Recall@k, MRR@10, nDCG@10 and no-answer
  accuracy match hand-computed values; a paired bootstrap gives the same
  interval with the same seed; each failure gets its class and the route that
  missed it.
- [ ] **Step 2: Implement** `eval run` and `eval compare`, reports stored as
  artifacts and journaled.
- [ ] **Step 3: Gate and pull request** `feat: evaluate retrieval with
  intervals`.

### T022 [P] The `maestro` binary: import, quality, status and jobs [US6]

**After:** T016, T019. **Files:** `crates/maestro/{Cargo.toml, src/main.rs,
src/cli/}`. **Requirements:** FR-S1-012.

- [ ] **Step 1: Failing contract tests.** `knowledge collection add`,
  `import`, `quality`, `status` and `job wait` print versioned JSON on stdout
  and diagnostics on stderr; exit codes are 0, 1, 2; a long command prints its
  job ID first.
- [ ] **Step 2: Create the crate** and implement those commands over the
  knowledge library.
- [ ] **Step 3: Gate and pull request** `feat: add the maestro command line`.

## Phase 6: Wave 6 — chunks, questions and setup

### T023 [P] Prepare: deduplicate and chunk [US2]

**After:** T020, T018. **Files:** `crates/maestro-knowledge/src/prepare.rs`.
**Requirements:** FR-S1-003.

- [ ] **Step 1: Failing tests.** Exact duplicates keep every occurrence;
  near-duplicates are grouped, not deleted; a unit over 700 tokens refuses its
  document with the unit named; a chunk set records its profile and counter.
- [ ] **Step 2: Implement** `knowledge prepare` over the canonicalization
  crate.
- [ ] **Step 3: Measure** the chunk count on `ctm` and record it in the plan's
  scale line.
- [ ] **Step 4: Gate and pull request** `feat: prepare chunk sets`.

### T024 [P] The golden set, drafted [US3]

**After:** T020. **Files:** `PRIVATE`: `evals/ctm/`. **Requirements:**
FR-S1-013.

- [ ] **Step 1: Sample** the canonical sections stratified by source kind and
  set.
- [ ] **Step 2: Draft** at least 100 questions, French and English, about
  15 % unanswerable, each with its expected section IDs.
- [ ] **Step 3: Check** that every expected section exists and every
  unanswerable question has none.
- [ ] **Step 4: Pull request** in `PRIVATE` `feat: draft the ctm golden set`.

### T025 [P] Setup, status and doctor [US6]

**After:** T022, T006, T007. **Files:**
`crates/maestro/src/cli/{setup,doctor}.rs`. **Requirements:** FR-S1-015.

- [ ] **Step 1: Failing tests.** `setup` previews the Qdrant service it will
  install and changes nothing without approval; a second run changes nothing;
  `doctor` names every failed check with its next action.
- [ ] **Step 2: Implement** the pinned Qdrant install (digest in the code),
  the systemd user unit, `status` and `doctor`.
- [ ] **Step 3: Run** them on the workstation.
- [ ] **Step 4: Gate and pull request** `feat: set up and diagnose the local
  services`.

## Phase 7: Wave 7 — the search projection

### T026 [P] The Qdrant projection [US2]

**After:** T023, T006, T012, T040. **Files:**
`crates/maestro-knowledge/src/{represent,index}.rs`,
`.github/workflows/integration.yml`. **Requirements:** FR-S1-004, D9, R6, R7.

- [ ] **Step 1: Failing integration tests** against a pinned Qdrant: a
  generation builds in its own collection; a vector of the wrong dimension or
  a non-finite value refuses the batch; an interrupted build resumes at its
  last journaled batch; the alias moves only after verification; a
  generation records its sparse profile.
- [ ] **Step 2: Implement** representations (dense through the gateway,
  sparse through T040's analyzer, weighted by Qdrant with `modifier: idf`)
  and the generation lifecycle.
- [ ] **Step 3: CI job** `integration.yml` with the Qdrant image pinned by
  digest.
- [ ] **Step 4: Gate and pull request** `feat: publish search generations in
  Qdrant`.

### T027 [P] The owner checks 30 questions [US3]

**After:** T024. **Files:** `PRIVATE`: `evals/ctm/validation.jsonl`.
**Requirements:** FR-S1-013.

- [ ] **Step 1: The owner validates** a sample of 30 stratified by topic,
  language and answerability.
- [ ] **Step 2: Apply** every correction; a rejected question is fixed or
  removed.
- [ ] **Step 3: Freeze** the set with its digest for the bake-off.

## Phase 8: Wave 8 — publish, search, choose

### T028 [P] Publish, status and verify [US2]

**After:** T026, T016. **Files:** `crates/maestro-knowledge/src/publish.rs`.
**Requirements:** FR-S1-004, FR-S1-011.

- [ ] **Step 1: Failing tests.** A second publish of one collection is refused
  by its lease; a verified generation switches the alias and emits
  `generation.published`; the previous one is retired with
  `generation.retired`; `verify` replays digests and recounts.
- [ ] **Step 2: Implement** `knowledge publish`, `status` and `verify` as
  jobs.
- [ ] **Step 3: Gate and pull request** `feat: publish, inspect and verify
  generations`.

### T029 [P] Retrieval routes and fusion [US1]

**After:** T026, T011. **Files:**
`crates/maestro-knowledge/src/search/{routes,fusion}.rs`. **Requirements:**
FR-S1-005, FR-S1-005a, FR-S1-006.

- [ ] **Step 1: Failing tests.** R1, R2, R3 and R6 run in parallel under a
  deadline; each deduplicates its own hits; RRF is one-based with K = 60 and
  stable ties; a route past its deadline is reported, not waited for; every
  route filters by scope.
- [ ] **Step 2: Implement** the routes and the fusion, with `search_dense` and
  `search_bm25` as independent diagnostics.
- [ ] **Step 3: Gate and pull request** `feat: search by four routes fused
  with RRF`.

### T030 [P] Bake-off round 1 [US3]

**After:** T021, T026, T027. **Files:** `PRIVATE`: `bakeoff/`;
`crates/maestro-knowledge/src/eval/bakeoff.rs`,
`migrations/0007_model_cards.sql`. **Requirements:** FR-S1-014, ADR-0011.

- [ ] **Step 1: List candidates** per role from 05 §3.2 with size, licence and
  source.
- [ ] **Step 2: The owner approves** the downloads.
- [ ] **Step 3: Failing test.** A candidate violating a hard constraint is
  reported ineligible, never ranked; every attempt is kept, failures
  included.
- [ ] **Step 4: Run** the protocol of 05 §3.3: each embedder with its own
  chunk profile and generation, then the rerankers, then the answerers.
- [ ] **Step 5: Record** a model card per role.

## Phase 9: Wave 9 — reranking, evidence and recovery

### T031 [P] Reranking within the budget [US1]

**After:** T029, T008, T006. **Files:**
`crates/maestro-knowledge/src/search/rerank.rs`. **Requirements:** FR-S1-005,
FR-S1-015a.

- [ ] **Step 1: Failing tests.** Results map back by index; no candidate is
  truncated; a reranker without room answers `rerank: "unavailable"`.
- [ ] **Step 2: Implement** reranking with the depth T008 measured, as a
  ladder parameter.
- [ ] **Step 3: Gate and pull request** `feat: rerank fused candidates`.

### T032 [P] Evidence assembly [US1]

**After:** T017, T029. **Files:**
`crates/maestro-knowledge/src/search/evidence.rs`. **Requirements:**
FR-S1-005.

- [ ] **Step 1: Failing tests.** Small-to-big expansion, span unions, MMR
  diversity, version collapse, conflict flags and known gaps each change the
  bundle as 02 §6 says; a section too large is windowed and marked.
- [ ] **Step 2: Implement** the assembly into `maestro-evidence/1`, trace
  apart.
- [ ] **Step 3: Gate and pull request** `feat: assemble evidence bundles`.

### T033 [P] Backup, restore and the rebuild drill [US5]

**After:** T028. **Files:** `crates/maestro/src/cli/backup.rs`.
**Requirements:** FR-S1-010, SC-S1-006.

- [ ] **Step 1: Failing tests.** A backup holds the database and an artifact
  manifest; restore refuses traversal and links; an interrupted publish
  restores as interrupted and resumes.
- [ ] **Step 2: Implement** `backup` and `restore` with the projection
  rebuild.
- [ ] **Step 3: Drill:** backup, wipe, restore, rebuild, identical synthetic
  rankings.
- [ ] **Step 4: Gate and pull request** `feat: back up and restore the
  kernel`.

## Phase 10: Wave 10 — MCP, `ask` and the synthetic gate

### T034 [P] The MCP server, and search from the command line [US1]

**After:** T022, T031, T032. **Files:** `crates/maestro/src/{mcp.rs,
cli/search.rs}`. **Requirements:** FR-S1-008, FR-S1-012.

- [ ] **Step 1: Failing tests** over stdio: `knowledge_collections`,
  `knowledge_search` and `knowledge_get` answer with their schemas; a caller
  without the `ctm` scope sees no count or title; a response over 64 KiB is
  truncated and says so; `knowledge search` and `knowledge get` print
  versioned JSON.
- [ ] **Step 2: Implement** the server with `rmcp` 3.4.1 and the two
  commands, and raise the workspace MSRV to 1.88, the lowest `rmcp` accepts
  (R3).
- [ ] **Step 3: Gate and pull request** `feat: serve knowledge over MCP`.

### T035 [P] `ask` with its guards [US4]

**After:** T032, T006. **Files:** `crates/maestro-knowledge/src/answer.rs`,
`crates/maestro/src/mcp.rs` (one tool). **Requirements:** FR-S1-007.

- [ ] **Step 1: Failing tests** with the fake generator: an invented command
  is rejected, regenerated once, then refused; an unanswerable question is
  refused with its reason and the closest passages; the answer takes the
  question's language.
- [ ] **Step 2: Implement** `knowledge ask` and the `knowledge_ask` tool.
- [ ] **Step 3: Gate and pull request** `feat: answer from evidence or
  refuse`.

### T036 [P] The synthetic suite gates pull requests [US3]

**After:** T014, T021, T032. **Files:** `.github/workflows/ci.yml`,
`tests/synthetic_suite.rs`. **Requirements:** SC-S1-007.

- [ ] **Step 1: Failing CI check.** The suite runs with the deterministic fake
  models and fails the pull request below its recorded baseline.
- [ ] **Step 2: Wire** the suite into CI and record the baseline.
- [ ] **Step 3: Gate and pull request** `ci: gate pull requests on the
  synthetic suite`.

## Phase 11: Wave 11 — the first published generation

### T037 The ladder, the first publication and `ctm-answers` [US3, US4]

**After:** T028, T030, T031, T035. **Files:** `PRIVATE`: `reports/`.
**Requirements:** SC-S1-002, SC-S1-003, SC-S1-008.

- [ ] **Step 1: Run the ladder** BM25 → dense → hybrid → identifiers → rerank
  on `ctm-retrieval`.
- [ ] **Step 2: Ship** only the rungs whose paired gain has an interval above
  zero.
- [ ] **Step 3: Publish** the `ctm` generation with the winners and the
  shipped rungs.
- [ ] **Step 4: Classify** every remaining failure of the golden set.
- [ ] **Step 5: Run `ctm-answers`:** command exactness 100 %, no-answer
  accuracy at least 80 %.

## Phase 12: Wave 12 — four agents ask Control-M

### T038 Four agents ask Control-M [US1]

**After:** T034, T037. **Files:** `docs/` how-to; client configuration stays
local. **Requirements:** SC-S1-005.

- [ ] **Step 1: Pi** calls `knowledge_search` and receives cited passages.
- [ ] **Step 2: Codex** does the same.
- [ ] **Step 3: Claude Code** does the same.
- [ ] **Step 4: Copilot CLI** does the same.
- [ ] **Step 5: Document** the one-line registration for each client.

## Phase 13: Exit — M1 "Ask Control-M"

### T039 Measure, map and release [US1, US3]

**After:** every other task. **Files:**
`docs/architecture/08-traceability.md`, the release. **Requirements:**
SC-S1-001 to SC-S1-009.

- [ ] **Step 1: Measure p95** of `knowledge_search` on the workstation with
  the search models loaded; report waits and missing routes apart.
- [ ] **Step 2: Check every success criterion** with its evidence.
- [ ] **Step 3: Map** each of the 94 rows of 08 to the task that delivered it,
  or to the slice that keeps it.
- [ ] **Step 4: Release** M1 through the organization's release path.
