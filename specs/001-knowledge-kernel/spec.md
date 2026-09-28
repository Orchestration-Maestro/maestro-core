# Feature Specification: Knowledge kernel and hybrid RAG

**Feature Branch**: `docs/001-knowledge-kernel` (the organization's `branch-names`
ruleset takes a conventional type before the Spec Kit name)

**Created**: 2026-09-23

**Updated**: 2026-09-25, checked against the design of record after S0

**Status**: Draft

**Input**: "First slice: lay the building blocks of the intelligence backend and
deliver a hybrid, evaluated RAG over the existing Control-M corpus, usable from
Pi and Copilot through MCP."

Architecture: [01](../../docs/architecture/01-knowledge-pipeline.md),
[02](../../docs/architecture/02-retrieval-and-knowledge-graph.md),
[04 §3](../../docs/architecture/04-intelligence-backend.md#3-the-kernel-building-blocks),
[05 §3](../../docs/architecture/05-platform-and-operations.md#3-model-selection),
[06 S1](../../docs/architecture/06-roadmap.md#s1-knowledge-kernel--hybrid-rag--m1),
[07 §7](../../docs/architecture/07-extensibility.md#7-delivery-by-slice).

Rules: the organization's golden rules come first, as this repository maps them
in [`docs/standards/`](../../docs/standards/engineering.md)
([ADR-0017](../../docs/adr/0017-spec-kit-installed-once-for-the-organization.md)).

## Clarifications

### Session 2026-09-25 (superseded by the 2026-09-26 clarification)

- Q: Who writes the evaluation questions, and how many does the owner check? →
  A: Agents draft them from the corpus; the owner validates a stratified sample
  of 30.
- Q: When a search and the largest chat model cannot both fit on the GPU, which
  gives way? → A: The search. Its models load on demand and chat models keep the
  card; the latency target applies with the search models loaded.

### Session 2026-09-26

- Q: Who checks the evaluation set's questions? → A: An independent review,
  by a model other than the drafter's, checks every question against the
  corpus; the owner decides each change it proposes to a question's wording or
  answerability. This replaces the owner's sample of 30.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Ask Control-M from my agent (Priority: P1) 🎯 MVP

As a developer working in Pi, Codex, Claude Code or Copilot CLI, I ask a
Control-M question and my agent receives verbatim, cited passages from the full
corpus through the Maestro MCP server.

**Why this priority**: it is the first user-visible value and exercises every
building block (scopes, artifacts, generations, retrieval, MCP).

**Independent Test**: with the published `ctm` generation, `knowledge_search`
called from each of the four clients returns an evidence bundle whose passages
match the expected sections of sample questions.

**Acceptance Scenarios**:

1. **Given** a published generation, **When** an agent calls `knowledge_search` with
   "How do I install Control-M/Agent on UNIX?", **Then** it receives passages with
   title, section path, version, URL, digest and verbatim text within the token
   budget.
2. **Given** a French question, **When** searched, **Then** English passages are
   retrieved (cross-lingual) and the response reports the detected language.
3. **Given** a question naming a parameter or error code, **When** searched, **Then**
   a passage containing that exact identifier ranks in the top 5.
4. **Given** a caller without the `ctm` scope, **When** it calls the tool, **Then** the
   collection is invisible and no count or title leaks.
5. **Given** the reranker is down, **When** searched, **Then** the response is flagged
   `rerank: "unavailable"` rather than silently degraded.

---

### User Story 2 - Import and publish the corpus safely (Priority: P1)

As the maintainer, I import the owner-approved private product collection,
prepare chunks and publish a searchable generation, knowing that bad inputs are
refused, re-runs are idempotent and a crash can be resumed.

**Why this priority**: without a trustworthy published generation there is
nothing to search.

**Independent Test**: use the current owner-pinned receipt privately; import,
prepare and publish; interrupt publishing and resume; compare counts and
digests in the private report.

**Acceptance Scenarios**:

1. **Given** a manifest entry whose file digest differs, **When** imported, **Then**
   that entry is refused with both digests and the rest continue.
2. **Given** an imported corpus, **When** import runs again, **Then** every revision
   reports `unchanged` and nothing is rewritten.
3. **Given** a unit that cannot fit 700 tokens, **When** prepared, **Then** the document
   is refused with the unit identified, and nothing is truncated.
4. **Given** a publish interrupted after some batches, **When** resumed, **Then** it
   continues from the last journaled batch and the alias still points at the
   previous generation until verification passes.
5. **Given** a verified new generation, **When** published, **Then** the alias switches
   atomically and a `maestro.knowledge.generation.published.v1` event is
   journaled.

---

### User Story 3 - Measure quality and choose models (Priority: P1)

As the maintainer, I run evaluation suites, compare configurations along the
ladder (BM25 → dense → hybrid → + identifiers → + rerank) and record the models
that win each role.

**Why this priority**: every later choice (models, fusion weights, the graph)
depends on a trustworthy measurement.

**Independent Test**: `maestro eval run ctm-retrieval` produces a report with
per-question results, metrics with confidence intervals and latency; the bake-off
report names a winner per role with its model card.

**Acceptance Scenarios**:

1. **Given** the golden set, **When** the suite runs twice on the same generation,
   **Then** deterministic metrics are identical.
2. **Given** two configurations, **When** compared, **Then** the report shows paired
   differences with confidence intervals.
3. **Given** a candidate model, **When** it violates a hard constraint (licence,
   latency, memory), **Then** it is reported as ineligible, not ranked.
4. **Given** a question the published configuration gets wrong, **When** the
   report is read, **Then** the failure is classified as not retrieved,
   misranked or wrong answer, with the route that missed it.
5. **Given** a rung of the ladder, **When** it does not improve on the rung below
   it with a paired difference whose interval excludes zero, **Then** it is not
   shipped, and the report says so.

---

### User Story 4 - A grounded answer or an honest refusal (Priority: P2)

As a developer on the command line, I run `maestro knowledge ask` and receive an
answer that cites its passages, quotes commands verbatim, or refuses when the
evidence is insufficient.

**Independent Test**: on `ctm-answers`, command exactness is 100 % and refusals
occur on the unanswerable subset.

**Acceptance Scenarios**:

1. **Given** a procedural question, **When** answered, **Then** the steps are quoted
   from the cited section and every command appears verbatim in the evidence.
2. **Given** an unanswerable question, **When** asked, **Then** the answer is a refusal
   with its reason and the closest passages.
3. **Given** an answer that invents a command, **When** checked, **Then** it is
   rejected, regenerated once, then refused.

---

### User Story 5 - Back up, restore and rebuild (Priority: P2)

As the maintainer, I back up the kernel, restore it on a clean machine state and
rebuild the Qdrant projection, obtaining the same search results.

**Independent Test**: backup → wipe → restore → rebuild → the synthetic suite gives
identical rankings.

**Acceptance Scenarios**:

1. **Given** a backup, **When** it is restored on a clean state and the projections
   are rebuilt, **Then** the synthetic suite returns identical rankings.
2. **Given** a backup taken while a publish was running, **When** it is restored,
   **Then** the interrupted job shows as interrupted and resumes from its last
   journaled batch.

---

### User Story 6 - Set up and know what is healthy (Priority: P3)

As a developer, `maestro setup` installs the search service Maestro needs as a
user service, and `maestro status` and `maestro doctor` tell me which services,
collections and generations are ready and how to fix what is not.

**Acceptance Scenarios**:

1. **Given** a machine without the search service, **When** `maestro setup` runs,
   **Then** it previews the service it will install, installs it on approval, and
   a second run changes nothing.
2. **Given** the model router is unreachable, **When** `maestro doctor` runs,
   **Then** it names the router, the address it tried and the next action.

### Edge Cases

- A corpus manifest line is malformed JSON → that line is refused with its line
  number; the import continues.
- The embedding model returns a vector of the wrong dimension or a non-finite
  value → the batch is refused; the generation is not published.
- Two publishes on the same collection → the second is refused by the job lease.
- The router's tokenizer and the native counter disagree on a fixture → the
  `RouterTokenizer` is not qualified and chunking refuses to run with it.
- A question in a language absent from the corpus → answered from cross-lingual
  retrieval or refused; never answered from the model's memory.
- The evidence budget cannot hold one full section → the section is windowed
  around the matched chunk and the window is marked.
- The embedder or the reranker could only be loaded by unloading a chat model →
  the search does not load it; it runs the lexical route, flags the dense route
  or the reranking as `unavailable` with the reason, and never presents the
  result as a full hybrid search.
- An evaluation report would quote Control-M text → it is written only to the
  private collection's location, never to the public repository or a public CI
  log.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-S1-001**: The system MUST provide the kernel blocks B1–B7, B9–B11 of
  [04 §3](../../docs/architecture/04-intelligence-backend.md#3-the-kernel-building-blocks)
  with the invariants listed there.
- **FR-S1-002**: The system MUST import a `maestro-corpus/1` manifest with digest
  verification, idempotency and a per-entry report, for a collection declared in
  a strict `collection.json` (ADR-0014).
- **FR-S1-002a**: Every imported revision MUST receive an explicit quality outcome
  (`accepted`, `accepted_with_warnings`, `needs_reextraction`, `quarantined`,
  `excluded`) from the checks of
  [01 §4](../../docs/architecture/01-knowledge-pipeline.md#4-corpus-quality-gate);
  only accepted revisions are indexed.
- **FR-S1-003**: The system MUST canonicalize, deduplicate (exact and near-duplicate)
  and chunk revisions with the existing crate, counting tokens through a
  `TokenCounter` qualified by parity against the native counter.
- **FR-S1-004**: The system MUST build dense and BM25 representations with profiles
  bound to model cards and publish them as a verified Qdrant generation switched by
  alias.
- **FR-S1-005**: Search MUST run routes R1–R3 (and R6 for exact inventories) in
  parallel under a deadline, fuse them with one-based RRF after per-route
  deduplication, rerank the top 80–120 with index-mapped results and no
  truncation, and assemble a `maestro-evidence/1` bundle with small-to-big
  expansion, span unions, MMR diversity, version collapse, conflict flags, known
  gaps and a separate trace.
- **FR-S1-005a**: Independent `search_dense` and `search_bm25` diagnostics MUST
  exist from the first published generation, so evidence recall is measured per
  route before fusion.
- **FR-S1-006**: Every read MUST be filtered by the caller's scopes inside each route.
- **FR-S1-007**: `ask` MUST apply the guards of
  [02 §7](../../docs/architecture/02-retrieval-and-knowledge-graph.md#7-grounded-generation-ask),
  including verbatim command verification.
- **FR-S1-008**: The MCP server MUST expose `knowledge_collections`,
  `knowledge_search`, `knowledge_get` and `knowledge_ask` over stdio with bounded
  responses.
- **FR-S1-008a**: The kernel journal MUST keep per-stream sequences with durable
  consumer cursors and acknowledgements. S1 ships and tests this cursor
  primitive; built-in projection and telemetry consumers move to S2
  ([07 §3.3](../../docs/architecture/07-extensibility.md#33-subscriptions-and-delivery),
  [07 §7](../../docs/architecture/07-extensibility.md#7-delivery-by-slice)).
- **FR-S1-008b**: The public event catalogue MUST start with the knowledge events
  of [07 §3.2](../../docs/architecture/07-extensibility.md#32-event-catalogue)
  (`import.completed`, `revision.held`, `generation.published`,
  `generation.retired`, all `.v1`), each with a JSON Schema generated from its
  type and a compatibility test against its released predecessor.
- **FR-S1-009**: The eval runner MUST produce per-item results, metrics with
  confidence intervals and latency, stored as artifacts and journaled; every
  failure is classified as not retrieved, misranked or wrong answer.
- **FR-S1-010**: The system MUST back up and restore the kernel and rebuild
  projections from it.
- **FR-S1-011**: Long operations MUST run as leased jobs resumable from the journal.
- **FR-S1-012**: The CLI MUST provide `knowledge …`, `eval …`, `setup`, `status`,
  `doctor`, `backup` and `restore`, with `--json` output under versioned schemas
  and the exit codes 0, 1, 2.
- **FR-S1-013**: An evaluation set of at least 100 Control-M questions MUST exist
  before any model is selected: drafted by agents from the corpus, French and
  English, about 15 % unanswerable, each with its expected sections. An
  independent review, by a model other than the drafter's, checks every
  question against the corpus. A change it proposes to a question's wording or
  answerability is the owner's decision; a change to its expected sections
  lands only when the corpus confirms it and the suite check accepts it. No
  question found wrong is kept as drafted.
- **FR-S1-014**: Each model role (embedder, reranker, answerer) MUST be filled by
  a recorded bake-off whose winner is a model card
  ([ADR-0011](../../docs/adr/0011-models-chosen-by-bake-off.md)); no model is
  preselected, including those the router already declares.
- **FR-S1-015**: `setup` MUST install the search service as a user service after
  a preview, idempotently, and `doctor` MUST check it, the model router and each
  model role with a named next action for every failure.
- **FR-S1-015a**: A search MUST never cause a chat model to be unloaded: the search
  models load only into room that is free, and a search that cannot get them
  runs the routes it can and flags the rest.
- **FR-S1-016**: Vendor material MUST stay private
  ([ADR-0009](../../docs/adr/0009-vendor-specific-material-stays-private.md)):
  the corpus, its exporter, `collection.json`, the quality ledger, the golden set
  and every Control-M evaluation report live in the private collection; public CI
  runs on a synthetic collection and suite only.

### Key Entities

- **Collection, Source, Document, Revision, Occurrence**: see
  [01 §11](../../docs/architecture/01-knowledge-pipeline.md#11-kernel-records-used-by-the-pipeline).
- **Chunk set**: chunks of a collection under one chunk profile and counter.
- **Generation**: a verified projection bound to a chunk set and representation
  profiles.
- **Model card**: the recorded identity and limits of a selected model.
- **Evidence bundle**: the search response contract.
- **Eval suite / report**: questions with expected sections; measured results,
  with every failure classified.
- **Golden set**: the private product question set used by evaluation suites,
  drafted by agents and independently reviewed in full; the owner decides only
  flagged changes.
- **Ladder report**: the paired comparison of each retrieval rung against the one
  below it, deciding which rungs ship.
- **Public event**: a journaled event in the public catalogue, with its schema.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-S1-001**: 100 % of the corpus is either imported or refused with an explained
  reason, and every imported revision has a quality outcome.
- **SC-S1-002**: The published generation uses the bake-off winners, and every
  shipped rung of the ladder improves on the rung below it by a paired
  difference whose confidence interval excludes zero.
- **SC-S1-003**: Command exactness is 100 % and no-answer accuracy ≥ 80 % on
  `ctm-answers` (initial target, revisited after the baseline).
- **SC-S1-004**: `knowledge_search` p95 < 1.5 s on the reference workstation with
  the search models loaded; searches that waited for a load or ran without a
  route are counted and reported separately, never averaged in.
- **SC-S1-005**: The MCP tools work from Pi, Codex, Claude Code and Copilot CLI.
- **SC-S1-006**: Backup → restore → rebuild yields identical synthetic-suite rankings.
- **SC-S1-007**: CI green with coverage ≥ 90 % and the synthetic suite gating.
- **SC-S1-008**: Evidence recall is reported per route before fusion, and 100 %
  of the golden set's failures carry a classification.
- **SC-S1-009**: The public repository and its CI logs contain no Control-M
  content: public CI reads only synthetic fixtures, and a content check refuses
  corpus text in any public file.

## Out of Scope

- The knowledge graph, extraction and entity resolution (S2).
- The catalog, its routing and `maestro init` (S3).
- Workflow runs, the daemon, the extension host and the local HTTP API (S4).
- Native acquisition and extraction from HTML, PDF or Office; S1 imports the
  Markdown the existing pipeline produced (S6).
- Memory, code intelligence and the workbench (S7).
- Other Control-M versions, and any second collection besides a synthetic one
  for tests.

## Traceability

[08](../../docs/architecture/08-traceability.md) records design dispositions;
those statuses are not delivery evidence. The earlier aggregate claim that S1
touches 94 rows has not been reconciled into an approved row inventory (for
example, §11.3 contains ten rows, not the previously counted nine). Current
integrated examples are `9c21827` (grouped evaluation labels count once),
`25bbc47` (stable dense and lexical ties), and `d30573d` (the synthetic
retrieval regression gate). These commits are examples, not a complete map or a
claim that all 94 rows are delivered.

T039 owns the owner-approved exact row keys, each delivered portion and its
task, integrated code/test evidence, and named remaining slice. Its check must
reject missing, duplicate, or extra keys. Until that map is approved and
completed, this spec makes no complete 94-row delivery claim. Each FR-S1 and
SC-S1 item is mapped to its commits and tests, or marked open with its owner,
in [08 §20](../../docs/architecture/08-traceability.md#20-s1-delivery-evidence).

## Assumptions

- The owner's product collection is private. Its scope rule, counts and
  receipts live in the private collection repository; public CI uses synthetic
  fixtures only. Re-measure acceptance from the current owner-pinned receipt,
  and keep the resulting report private.
- The model router's dedicated endpoints forward any path to the model, so
  `/models/<id>/tokenize` reaches the embedder's tokenizer; no router change is
  needed (verified on the router's `main`, 2026-09-25).
- The largest chat models in the router's catalogue are estimated at 28.9 to
  30.5 GiB of the 32 GiB card, so the search models cannot always sit beside
  them. Loading a model only into free room, without unloading another, may
  need a router option; the plan checks, and if it is missing the change lands
  first in `maestro-model-router`.
- The router already declares a `bge-m3` embedder and a `bge-reranker-v2-m3`
  reranker with earlier measurements; those justify testing them, never
  selecting them ([05 §3.2](../../docs/architecture/05-platform-and-operations.md#32-candidate-pools)).
- Answers are written in the question's language; commands, parameters and
  quoted passages stay verbatim. **S3 review amendment, 2026-09-28 (planned
  C05d):** an explicit session language overrides this default, but absence of
  a language preserves it. All evaluation runs (ladder and eval suites) ignore
  session preferences and retain question language with their pinned profile.
  Unsupported language detection is recorded as unchecked, never a false pass
  or an automatic refusal. This note changes the S3 contract, not S1's delivered
  behavior or measured qualification.
- The corpus is imported once and re-imported by hand; watching a source is S6
  (product.CD2).
- Candidate models are available locally or downloadable under their licences;
  downloads happen only with the maintainer's approval.
- A single local user; multi-user scopes are exercised by tests, not deployed.
- The organization's lint changes in flight (pull request #13) touch code and
  manifests, not this specification; the plan starts from `main` as it stands
  when planning begins.
