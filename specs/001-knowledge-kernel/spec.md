# Feature Specification: Knowledge kernel and hybrid RAG

**Feature Branch**: `001-knowledge-kernel`

**Created**: 2026-09-23

**Status**: Draft

**Input**: "First slice: lay the building blocks of the intelligence backend and
deliver a hybrid, evaluated RAG over the existing Control-M corpus, usable from
Pi and Copilot through MCP."

Architecture: [01](../../docs/architecture/01-knowledge-pipeline.md),
[02](../../docs/architecture/02-retrieval-and-knowledge-graph.md),
[04 §3](../../docs/architecture/04-intelligence-backend.md#3-the-kernel-building-blocks),
[05 §3](../../docs/architecture/05-platform-and-operations.md#3-model-selection),
[06 S1](../../docs/architecture/06-roadmap.md#s1-knowledge-kernel--hybrid-rag--m1).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Ask Control-M from my agent (Priority: P1) 🎯 MVP

As a developer working in Pi or Copilot CLI, I ask a Control-M question and my
agent receives verbatim, cited passages from the full corpus through the Maestro
MCP server.

**Why this priority**: it is the first user-visible value and exercises every
building block (scopes, artifacts, generations, retrieval, MCP).

**Independent Test**: with the published `ctm` generation, `knowledge_search`
called from Pi and from Copilot CLI returns an evidence bundle whose passages
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

As the maintainer, I import the 7,988-document corpus, prepare chunks and
publish a searchable generation, knowing that bad inputs are refused, re-runs are
idempotent and a crash can be resumed.

**Why this priority**: without a trustworthy published generation there is
nothing to search.

**Independent Test**: import, prepare and publish on the full corpus; kill the
process during publish and resume; compare counts and digests.

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
   atomically and a `GenerationPublished` event is journaled.

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

---

### User Story 6 - Know what is healthy (Priority: P3)

As a developer, `maestro status` and `maestro doctor` tell me which services,
collections and generations are ready and how to fix what is not.

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
  consumer cursors and acknowledgements, used by the projections and telemetry
  ([07 §3.3](../../docs/architecture/07-extensibility.md#33-subscriptions-and-delivery)).
- **FR-S1-009**: The eval runner MUST produce per-item results, metrics with
  confidence intervals and latency, stored as artifacts and journaled.
- **FR-S1-010**: The system MUST back up and restore the kernel and rebuild
  projections from it.
- **FR-S1-011**: Long operations MUST run as leased jobs resumable from the journal.
- **FR-S1-012**: The CLI MUST support `--json` output with versioned schemas and the
  exit codes 0, 1, 2.

### Key Entities

- **Collection, Source, Document, Revision, Occurrence**: see
  [01 §11](../../docs/architecture/01-knowledge-pipeline.md#11-kernel-records-used-by-the-pipeline).
- **Chunk set**: chunks of a collection under one chunk profile and counter.
- **Generation**: a verified projection bound to a chunk set and representation
  profiles.
- **Model card**: the recorded identity and limits of a selected model.
- **Evidence bundle**: the search response contract.
- **Eval suite / report**: questions with expected sections; measured results.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-S1-001**: 100 % of the corpus is either imported or refused with an explained
  reason, and every imported revision has a quality outcome.
- **SC-S1-002**: The published configuration is the best rung of the ladder on
  `ctm-retrieval`, with its report recorded.
- **SC-S1-003**: Command exactness is 100 % and no-answer accuracy ≥ 80 % on
  `ctm-answers` (initial target, revisited after the baseline).
- **SC-S1-004**: `knowledge_search` p95 < 1.5 s on the reference workstation.
- **SC-S1-005**: The MCP tools work from Pi, Codex, Claude Code and Copilot CLI.
- **SC-S1-006**: Backup → restore → rebuild yields identical synthetic-suite rankings.
- **SC-S1-007**: CI green with coverage ≥ 90 % and the synthetic suite gating.

## Assumptions

- The existing Python outputs (`corpus-manifest.jsonl` and Markdown files) are the
  corpus of record for S1; native acquisition comes in S6.
- The model router can expose `/tokenize` for the embedding candidates; if its
  dedicated endpoints do not pass that path, a small router change lands first in
  `maestro-model-router`.
- Candidate models are available locally or downloadable under their licences;
  downloads happen only with the maintainer's approval.
- A single local user; multi-user scopes are exercised by tests, not deployed.
