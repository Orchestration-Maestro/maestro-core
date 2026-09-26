# Implementation Plan: Knowledge kernel and hybrid RAG

**Branch**: `docs/001-knowledge-kernel` | **Date**: 2026-09-25 | **Spec**: [spec.md](spec.md)

**Input**: the clarified spec, [06 S1](../../docs/architecture/06-roadmap.md#s1-knowledge-kernel--hybrid-rag--m1),
the architecture it cites, and the repository, the router and the corpus as
measured on 2026-09-25. Tasks: [tasks.md](tasks.md).

## Summary

Build the first product slice: a Rust knowledge kernel and a hybrid, evaluated
RAG over the Control-M corpus, reachable from Pi, Codex, Claude Code and Copilot
CLI through MCP. Three new crates appear, each only as its first working
behaviour lands: `maestro-kernel` (the building blocks B1–B7, B9–B11 on SQLite
and content-addressed artifacts), `maestro-knowledge` (collections, import,
quality, prepare, representations, Qdrant generations, search, `ask`, evals)
and `maestro` (the binary: CLI and MCP server). The canonicalization crate gains
one seam, a `TokenCounter` trait, so chunks can be counted by the router as well
as by the native counter. The router gains one option: load a model only into
free memory, as a guest it unloads first. The private collection gains the
corpus mapping, the collection
declaration and the golden set. Model roles are filled by the first bake-off,
and the published generation uses its winners. The work runs as 40 tasks in
13 waves of up to six parallel tasks, on a critical path of twelve (D16).

## Technical Context

**Language/Version**: Rust 1.98.1 (edition 2024) from `rust-toolchain.toml`.
The workspace MSRV is 1.85 until `qdrant-client` enters in T026: its gRPC
stack, `tonic` 0.14, needs **1.88**, as `rmcp` does in T034; the MSRV job
checks the dependencies that declare none.

**Primary Dependencies** (latest on crates.io, checked 2026-09-25; each enters
with the task that first needs it): `rusqlite` 0.40.2 (bundled), `qdrant-client`
1.19.0, `rmcp` 3.4.1, `tokio` 1.53.1, `reqwest` 0.13.5, `clap` 4.6.7,
`schemars` 1.2.2, `serde` and `serde_json`, `sha2` (already locked), `ulid`
3.0.0, `thiserror` 2.0.21, `tracing` 0.1.44 without its default features.
Not taken:

- `zstd`: nothing in S1 needs compression.
- OTLP export (`tracing-opentelemetry` 0.34.0, `opentelemetry-otlp` 0.33.0):
  it builds on `prost-derive` 0.14, which still uses `syn` 2 beside the
  workspace's `syn` 3, a duplicate major version the gate refuses (DEP-001).
  Spans already carry pinned attribute names, so adding the exporter once
  `prost-derive` moves to `syn` 3 renames nothing.
- `tracing`'s default `attributes` feature: `tracing-attributes` uses `syn` 2
  too, so `#[instrument]` is not available.

**Storage**: SQLite (WAL) and a content-addressed artifact tree under the
kernel's data directory: `$XDG_DATA_HOME/maestro/` when that is absolute,
else `$HOME/.local/share/maestro/` on Linux and macOS and
`%LOCALAPPDATA%\maestro\` on Windows; Qdrant 1.19 as the search projection.

**Testing**: `cargo test` with the workspace lints; a synthetic collection and
suite in public CI with a deterministic fake embedder; Qdrant integration tests
against a pinned Qdrant container; explicit local runs for the router, native
tokenizer parity and every `ctm-*` suite; `cargo mutants` on each diff;
coverage ≥ 90 % of lines.

**Target Platform**: every crate builds and passes its tests on Linux, macOS
and Windows, as ADR-0016 and ADR-0018 require: CI runs them on `ubuntu-24.04`,
`macos-15` and `windows-2025`. Where a platform differs, the difference sits
in a small function every host tests. The router, the GPU measurements and
the end-to-end proof of M1 run on the reference workstation (Linux x86_64
under WSL2, with an RTX 5090).

**Project Type**: Cargo workspace: libraries plus one binary (CLI and MCP stdio
server).

**Performance Goals**: `knowledge_search` p95 < 1.5 s with the search models
loaded (SC-S1-004); import streams with memory bounded by the largest document.

**Constraints**: public repository, so no Control-M content, personal path or
secret (ADR-0009, ENF-001, SEC-001); a search never unloads a chat model
(FR-S1-015a); every model role comes from a bake-off (ADR-0011); the
organization's rulesets, including `branch-names`. New code meets the
organization's generated lints and source rules (rust-workflows v4.3.0): no
indexing or slicing, no one-letter names, paths within two segments, `mod.rs`
modules whose `mod.rs` holds only `mod` and `use` lines, no import cycle, no
file over 500 lines of code, `Debug` on every type.

**Scale/Scope**: `ctm`'s export of 2026-09-26, 17,272 documents (19,165
revisions), 56.2 MiB of Markdown by each document's latest revision (3.3 KiB
on average); prepare (T023) cut 99,125 chunks, 14.6 million tokens, from its
17,173 eligible documents (147 tokens a chunk on average, 700 at most); one
local user.

## Starting point (measured 2026-09-25)

| Measure | Value |
| --- | --- |
| Workspace | Two crates, `maestro-canonicalization` and `maestro-conventions`; 333 tests pass; line coverage 94.41 % (`just check` on `main`) |
| Token counting | No `TokenCounter` trait: `chunk_documents` takes `&NativeTokenizer`; a counting seam exists only as a test helper, `chunk_with_count` |
| Corpus of record | 7,988 manifest lines, 31.3 MiB; every line carries the same 7 keys (`path`, `title`, `source_url`, `collection`, `source_tree`, `bytes`, `sha256`), fewer than `maestro-corpus/1` names; `source_tree` takes 6 values and `collection` 30 |
| Router | Dedicated endpoints forward any path (`/models/<id>/tokenize` reaches the model); entries `embed` (bge-m3 Q8) and `rerank` (bge-reranker-v2-m3 Q8), 1,280 MiB each, on demand; the largest chat entries estimate 28,928–30,464 MiB of the 32 GiB card; when room is short, admission unloads the coldest idle model, and no request option forbids it |
| Reranking cost | 12 ms per pair on the card, from the router catalogue's note (about a quarter of a second for twenty pairs); the cost at 80–120 pairs is unmeasured |
| MSRV | Workspace 1.85; `rmcp` 3.4.1 needs 1.88 |
| Merged since | maestro-core #14 (rust-workflows v2.5.1 and its lints), #17, #20 and #21 (Linux, macOS and Windows, ADR-0018), then the moves to rust-workflows v4.x, where `just check` became `rust-gate ci --local`; the S1 branches rebased on them on 2026-09-26 |

## Constitution Check

The organization's golden rules come first (ADR-0017); this repository maps
them in [`docs/standards/`](../../docs/standards/engineering.md).

| Rule | Status |
| --- | --- |
| FND-002, P-001 Simplicity, YAGNI | Pass: each crate arrives with its first behaviour; no compression, no broker, no HTTP API, no graph in S1 |
| P-004, P-005 WET, rule of three | Pass: a trait appears only for a real variation: `TokenCounter` (the native counter and the router) and the model port (the router and the deterministic fake CI needs) |
| P-011, P-013 Fail fast, parse don't validate | Pass: `collection.json` and `maestro-corpus/1` parse into typed values and refuse unknown keys (ADR-0014); a missing binding refuses before work starts |
| P-012 Illegal states unrepresentable | Pass: every read path takes a `ScopeSet`; there is no unscoped read function |
| P-014 Least privilege | Pass: MCP tools check scopes per call; the search models load only into free room |
| ENF-001 No machine paths | Pass: paths come from `bindings.toml` and XDG variables, never committed |
| ENF-002 Claimed platforms tested | Pass: Linux only, and only Linux is claimed |
| ENF-005 Failing test first | Pass: every task starts from a failing test; each diff is mutation-tested |
| ENF-006, ENF-008 Gates | Pass: coverage stays ≥ 90 %; `just check` mirrors CI; the Qdrant tests run in CI, not only locally |
| ENF-012 Pinned inputs | Pass: Qdrant pinned by version and digest; models by model card digest; crates by `Cargo.lock` |
| SEC-001 Minimise sensitive data | Pass: corpus, golden set and `ctm-*` reports live only in the private collection (FR-S1-016) |
| SEC-002 Input is data | Pass: corpus text and retrieved passages never become instructions; `ask` validates before delivery |
| SEC-003 Validate boundaries | Pass: artifact paths derive from digests only; restore refuses traversal and links |
| SEC-008 Truthful evidence | Pass: every bake-off attempt, including failures, is kept; unavailable routes are flagged, never hidden |

No exception is needed; Complexity Tracking is empty.

## Project Structure

### Documentation (this feature)

```text
specs/001-knowledge-kernel/
├── spec.md
├── plan.md                 # this file: design, data model, contracts, research
├── research.md             # the measured answers to the research rows
├── checklists/requirements.md
└── tasks.md
```

### Source code (repository root, after S1)

```text
crates/
├── maestro-canonicalization/   # + TokenCounter, implemented by NativeTokenizer
├── maestro-conventions/
├── maestro-kernel/
│   ├── src/  paths, binding, store (SQLite, migrations), artifact, journal
│   │         (events, cursors, schemas), scope, job, document, generation,
│   │         evidence, capability, gateway (router client, model cards),
│   │         telemetry
│   ├── migrations/             # forward-only SQL, one file per schema version
│   └── schemas/events/         # released event schemas, for compatibility tests
├── maestro-knowledge/
│   └── src/  collection, import, quality, prepare (router tokenizer), represent,
│             index (Qdrant), search (routes, fusion, rerank, evidence), answer,
│             eval (suites, metrics, ladder, bake-off)
└── maestro/
    └── src/  main, cli (knowledge, eval, job, setup, status, doctor, backup,
              restore), mcp
tests/fixtures/synthetic/       # the public synthetic collection and suite
.github/workflows/integration.yml   # Qdrant integration tests
```

Outside this repository: `maestro-model-router` gains the free-room option;
the private `ctm-collection` gains `export.jq`, `collection.json`,
`quality/ledger.jsonl`, `evals/ctm/` and `bakeoff/candidates.toml`.

**Structure decision**: the layout of [README §8](../../docs/architecture/README.md#8-workspace-layout-of-maestro-core).
Evaluation lives in `maestro-knowledge::eval` rather than a separate crate: it
has one consumer in S1.

## Design

### D1 Kernel store

SQLite through `rusqlite` (bundled), WAL, `busy_timeout` 5 s, foreign keys on,
short transactions with no I/O inside them, one writer connection behind a
mutex and readers on their own connections. Migrations are embedded SQL files,
numbered in advance per task, applied in number order and recorded by name in
a `migrations` table, so parallel tasks can merge in any order; each creates
only its own tables, and a database holding a migration the binary does not
know is refused. Triggers refuse `UPDATE` and `DELETE` on `events`. Paths:
`kernel.sqlite3` and `artifacts/` in the kernel's data directory (Technical
Context), which `paths::data_dir` resolves.

### D2 Artifacts (B3)

`artifacts/sha256/<2>/<2>/<64 hex>`. A write goes to a temporary file in the
same directory, is flushed, renamed, and the directory flushed; each directory
the store creates is flushed into its parent. An intact copy is never
rewritten; a damaged one, whose bytes no longer match or which is not a
regular file, is replaced by the same write. Every read verifies the digest
and reads only a regular file. The files and directories the store creates
are its owner's only (`0600` and `0700`). An `artifacts` table
indexes digest, size, media type, pin count and creation time. Garbage
collection deletes only unreferenced, unpinned artifacts, and reports what it
would delete before it does.

### D3 Journal and events (B2)

`events(id, stream, sequence, type, subject, scope, time, data)`: the ID is a
ULID; the sequence is the next per-stream number, taken in the same
transaction as the insert. Cursors `(consumer, stream, position)` survive
restarts; `ack` moves a cursor forward only. The table is the outbox: a
consumer reads after the commit, never inside it. Events leave the kernel in
the CloudEvents 1.0 envelope of [07 §3.1](../../docs/architecture/07-extensibility.md#31-event-envelope).
The four public knowledge events (FR-S1-008b) have JSON Schemas generated with
`schemars` and committed under `schemas/events/`; a test fails if a generated
schema removes or narrows anything its committed predecessor held.

### D4 Scopes (B1)

A scope is a path (`workspace/default/collection/ctm/source/docs-core`). Grants
tie a principal to a scope with rights; `visible(principal)` returns a
`ScopeSet`, and every read function takes one. The CLI and the MCP server run as
the local user's principal, granted from `config.toml`; an unknown scope is no
access.

### D5 Jobs (B4)

A job has a kind, an idempotency key (a digest of the command and its frozen
inputs), a state (`queued`, `running`, `succeeded`, `failed`, `cancelled`), a
lease with a heartbeat and an expiry, and its progress in the journal. A second
lease on the same key is refused, which is how two publishes of one collection
are refused. `maestro job wait <id>` follows a job; the glossary keeps "run" for
workflows, so this replaces the `maestro run wait` of
[01 §12](../../docs/architecture/01-knowledge-pipeline.md#12-commands).

### D6 Documents and quality (B5)

The tables of [01 §11](../../docs/architecture/01-knowledge-pipeline.md#11-kernel-records-used-by-the-pipeline).
Import streams `maestro-corpus/1` line by line: parse, check the digest,
derive the document ID from `source_ref`, canonicalize, store the original and
the canonical document as artifacts, record the revision (`imported`,
`unchanged` or `refused` with its reason). The quality gate of
[01 §4](../../docs/architecture/01-knowledge-pipeline.md#4-corpus-quality-gate)
then records one disposition per revision; only `accepted` and
`accepted_with_warnings` go on.

### D7 Token counting

`maestro-canonicalization` gains `pub trait TokenCounter { contract_id, verify,
token_ids }`. `NativeTokenizer` implements it, `chunk_documents` takes
`&impl TokenCounter`, and the test helper `chunk_with_count` becomes a test
implementation. `RouterTokenizer` (in `maestro-knowledge`) implements it over
`/models/<id>/tokenize`; its contract ID is a digest of the model card and the
router build, so a router chunk set has its own identities (ADR-0008). Parity:
for every fixture of the native qualification profile, the router's ordered IDs
equal the native counter's; an explicit native test, like S0's.

### D8 Model gateway (B10) and the free-room option

A model card is a strict JSON artifact (role, router entry, file digest,
template digest, server build, dimensions, limits, suite results). The gateway
calls the router's dedicated endpoints: `/v1/embeddings`, `/v1/rerank`,
`/tokenize`, `/v1/chat/completions`, each call bound to a card, never to a bare
model name. The router reports no file digest, so the gateway checks a card
against what the model's own server reports through `/props`, its build and
its chat template, and refuses a mismatch before any call; the file digest is
recorded when the card is made (T030). Search calls carry a request header
that the router change introduces, `X-Model-Router-Room: free`: admission may
then use only free room, and answers `503 insufficient_room` rather than
unload anything. A model
loaded that way is a guest: when another request needs room, idle guests are
unloaded before any other model, so a search's models never push a chat model
out later either. The gateway maps the refusal to an unavailable route, which
search flags (FR-S1-015a).

### D9 Representations and generations (B6)

One Qdrant collection per generation (`maestro-<collection>-g<n>`), behind the
alias `maestro-<collection>`. Points carry a dense vector (dimension from the
embedder's card) and a sparse vector that maestro computes with the
`bm25-en-fr/1` analyzer and Qdrant weights with `modifier: idf` (R7), with the
chunk ID, revision ID, section path, scope tags, version and source kind as
payload. Qdrant neither stores nor checks an analyzer policy, so the
generation records its sparse profile, and a query is analysed with the
profile of the generation it searches. Batches are
journaled, so a publish resumes at the last committed batch. Verification checks
the point count against the chunk set, the vectors' dimension and finiteness,
and a set of spot queries; only then does the alias switch, atomically, and
`generation.published` is journaled. The previous generation is kept until the
next publish.

### D10 Search

Routes R1 (dense), R2 (BM25), R3 (exact identifiers) and R6 (exact inventories)
run in parallel under a deadline; each deduplicates its own hits, then one-based
RRF with K = 60 fuses them. The reranker scores the top of the fused list with
results mapped back by index; the depth is a ladder parameter, starting at the
design's 80–120 and set by measurement. Evidence assembly follows
[02 §6](../../docs/architecture/02-retrieval-and-knowledge-graph.md#6-evidence-assembly)
into a `maestro-evidence/1` bundle, with a trace kept apart. A route or the
reranker that could not run is named in the bundle with its reason.

### D11 `ask`

The generator answers from the bundle with structured output. Every command,
parameter or code span in the answer must appear verbatim in the evidence;
otherwise the answer is regenerated once, then refused with its reason and the
closest passages. The answer takes the question's language.

### D12 CLI and MCP

`clap` noun-then-verb commands with `--json` output under versioned schemas,
exit codes 0, 1, 2, and the job ID printed first for long commands. The MCP
server uses `rmcp` 3.4.1 over stdio: `knowledge_collections`,
`knowledge_search`, `knowledge_get`, `knowledge_ask`, each with a JSON Schema,
a scope check per call and a 64 KiB response limit that reports truncation.

### D13 Evaluation and the bake-off

Suites are JSONL, `maestro-suite/1`, one file per suite: each `<name>.jsonl`
in the directory a collection's `evals.suite` names is the suite `<name>`,
with one question per line, giving its id, language (`fr` or `en`), text,
answerable flag and expected sections, each named by its document's
`source_ref` and heading path, plus a 1-based occurrence when that path
repeats, or by an empty heading path for a document without sections, never
by a section ID, which changes with the revision; the runner resolves the
names to the section IDs of the generation it evaluates, and a document named
whole to its document ID, which its passages carry.
Metrics: Recall@5 and @10, MRR@10, nDCG@10, no-answer accuracy, command
exactness, latency p50 and p95. Confidence intervals by paired bootstrap
(2,000 resamples). Each failure is classified as not retrieved, misranked or
wrong answer, with the route that missed it. The ladder report compares each
rung with the one below it. The bake-off follows
[05 §3.3](../../docs/architecture/05-platform-and-operations.md#33-protocol):
each embedder candidate gets its own chunk profile and generation, because
chunks are counted in the embedder's tokens; the golden set's expected answers
are sections, so the same set judges every candidate.

### D14 Setup, doctor, backup

`maestro setup` previews, then on approval installs Qdrant 1.19 from its
release archive, checked against a digest pinned in the code, under
`$XDG_DATA_HOME/maestro/qdrant/`, with a systemd user unit; a second run changes
nothing. `maestro doctor` checks the kernel database, the artifact tree,
Qdrant, the router and each role's model card, with a next action for every
failure. `maestro backup` uses SQLite's online backup and a manifest of the
artifact tree; `maestro restore` refuses traversal and links, then rebuilds
the projections from the kernel.

### D15 The private collection

`export.jq` maps the Python manifest to `maestro-corpus/1`: `source_url` to
`source_ref`, `source_tree` to `source_kind` (6 kinds), `collection` to `set`
(30 document sets, an optional field added to the contract so nothing is
dropped), the directory name to `version`; `lang`, `captured_at`, `component`
and `platform` are left out, never guessed. `path` stays relative to the
manifest's own directory. 832 lines carry no URL, all of the GitHub and
internal documents: their `source_ref` is `corpus-path:` followed by `path`,
unique and derived rather than invented, and without the release so a document
keeps its identity in the next one. Five URLs are each shared by two documents
with different content; the importer holds both of each pair (T019).
`collection.json` declares `ctm` (ADR-0014). The golden set is drafted after
canonicalization, because its expected answers point at section IDs, or at the
document ID of a document without sections, never at free text: agents sample
the corpus stratified by source kind, write the questions, and the owner
validates 30 stratified by topic, language and answerability.

### D16 Parallel delivery, CI and pull requests

The work is cut for parallel agents, not for one writer: 40 tasks in 13 waves,
each task naming the tasks it waits for and the files it owns
([tasks.md](tasks.md#parallel-delivery)). A task starts when its prerequisites
have merged, so a wave never waits for its slowest member. The critical path
is twelve tasks, from the artifact store to the release; the 28 others run
beside it. Up to four agents work at once, one task and one branch each,
because review is the limit; shared files are append-only and migration
numbers are reserved per task, so the merge order inside a wave is free.

`integration.yml` runs the Qdrant tests against the `qdrant/qdrant` 1.19 image
pinned by digest; `ci.yml` runs everything else through rust-workflows. Branch
names carry a conventional type first (`feat/s1-t005-kernel-store`), as the
organization's `branch-names` ruleset requires; `ctm-collection` and the
router take their pull requests in the waves that need them.

## Data model

Kernel tables, beside the document tables of
[01 §11](../../docs/architecture/01-knowledge-pipeline.md#11-kernel-records-used-by-the-pipeline):

| Table | Key columns | Rules |
| --- | --- | --- |
| `scopes` | `path, parent, tags_json` | Tree; unknown path is no access |
| `grants` | `principal, scope, rights, granted_by, granted_at` | Journaled; no implicit grant |
| `events` | `id, stream, sequence, type, subject, scope, time, data_json` | Append-only (triggers); unique `(stream, sequence)` |
| `cursors` | `consumer, stream, position, updated_at` | Moves forward only |
| `artifacts` | `digest, bytes, media, pins, created_at` | Content-addressed; verified on read |
| `jobs` | `id, kind, idempotency_key, state, lease_owner, lease_expires, attempt, outcome_json` | One live lease per key |
| `model_cards` | `id, role, digest, card_json, recorded_at` | A generation names its cards |
| `eval_reports` | `id, suite, generation_id, report_digest, created_at` | The report itself is an artifact |

## Contracts

| Contract | Shape |
| --- | --- |
| `maestro-corpus/1` | JSONL; required `schema`, `path` (relative to the manifest's directory), `sha256`, `bytes`, `source_ref` (the origin URL, or `corpus-path:` and `path` when there is none), `title`, `source_kind`; optional `set`, `version`, `lang`, `captured_at`, `product`, `component`, `platform`, `extractor`, `access`; unknown keys refused; lines sharing a `source_ref` with different digests are held |
| `maestro-collection/1` | Strict JSON of [01 §1](../../docs/architecture/01-knowledge-pipeline.md#1-collections-sources-and-scopes) (ADR-0014) |
| `maestro-evidence/1` | Passages with title, section path, version, URL, digest, span and text; conflict flags; known gaps; routes and their availability; trace apart |
| Events | `maestro.knowledge.{import.completed, revision.held, generation.published, generation.retired}.v1`, CloudEvents envelope, schemas in `schemas/events/` |
| CLI | `knowledge collection add`, `knowledge import`, `knowledge quality`, `knowledge prepare`, `knowledge publish`, `knowledge status`, `knowledge verify`, `knowledge search`, `knowledge ask`; `eval run`, `eval compare`, `eval bakeoff`; `job wait`; `setup`, `status`, `doctor`, `backup`, `restore` |
| MCP | The four tools of D12, inputs and outputs as in [02 §9](../../docs/architecture/02-retrieval-and-knowledge-graph.md#9-mcp-tools-knowledge) |

## Research

| # | Question | Decision | Rationale | Alternatives |
| --- | --- | --- | --- | --- |
| R1 | Does the router pass `/tokenize` to a model? | Yes, through `/models/<id>/tokenize` | Its dedicated endpoints forward any path (router `main`, `eddeb59`) | A router change, not needed |
| R2 | Can the router load a model without unloading another? | Not today; add `X-Model-Router-Room: free`, and unload what it loads first | Admission picks the coldest idle model to unload; only the router knows the room, so only it can refuse honestly | Checking `/metrics` first: racy, and still unloads when wrong |
| R3 | Which MSRV? | 1.88, raised in T026 when `qdrant-client` enters | `tonic` 0.14.6, under `qdrant-client` 1.19, and `rmcp` 3.4.1 declare it; the other crates declare less or nothing, and the MSRV job checks those | Keeping 1.85 with a hand-written Qdrant client and without MCP |
| R4 | Where does the corpus mapping live? | `export.jq` in the private repository, run with `jaq` from the toolbelt | Seven keys to rename and three to leave out; no program needed | A Rust exporter; a Python script |
| R5 | How can a chunk profile depend on a model the bake-off has not chosen? | Each candidate has its own chunk profile inside the bake-off | ADR-0008 counts chunks in the embedder's tokens; the golden set judges sections, which every profile shares | One neutral profile for all: breaks ADR-0008 |
| R6 | Where do Qdrant tests run? | A CI job with a pinned Qdrant service | Reusable workflows take no service containers; the adapter must still be tested in CI | Local only: breaks ENF-006 |
| R7 | Can Qdrant's server-side BM25 hold the French and English policy? | No: maestro computes the sparse vectors (`bm25-en-fr/1`, T040); Qdrant keeps the sparse index, IDF and fusion | Measured by T004 on Qdrant 1.19.1 ([research](research.md#r7-server-side-bm25)): no configuration passes the 23 checks, the best 18; folding runs before the French stemmer, no tokenizer keeps `max_retries` or `job-id` whole, and a misspelled option is ignored with HTTP 200 | Server-side BM25 with a language per passage and folding: fails French inflections and identifiers |
| R8 | What does reranking 80–120 pairs cost? | About 11 ms a pair, batched or not; the ladder starts at depth 80, the deepest whose p95 leaves 0.3 s for the rest of a search ([research](research.md#r8-the-cost-of-reranking-on-the-card)) | Measured by T008 on the RTX 5090: a `/v1/rerank` call is batched on the wire only, since the reranker's one slot scores its pairs one after another; p95 0.29–0.43 s at 20 pairs, 0.90–1.13 s at 80 and 1.41–1.42 s at 120; the depth stays a ladder parameter | A fixed depth; reranking on the CPU, 16 s for 20 pairs |

## Validation

The end-to-end proof of M1, run on the reference workstation:

1. `maestro setup`, then `maestro doctor`: every check passes or names its fix.
2. `maestro knowledge collection add collection.json`, `import`, `quality`,
   `prepare`: 7,988 documents accounted for, each with a disposition.
3. `maestro eval bakeoff`: a model card per role, every attempt kept.
4. `maestro knowledge publish --collection ctm`: a verified generation, the
   alias switched, the event journaled.
5. `maestro eval run ctm-retrieval` and `ctm-answers`: SC-S1-002, SC-S1-003 and
   SC-S1-008 met, the reports kept private.
6. From Pi, Codex, Claude Code and Copilot CLI: `knowledge_search` returns cited
   passages (SC-S1-005); p95 measured (SC-S1-004).
7. `maestro backup`, wipe, `maestro restore`: identical synthetic rankings
   (SC-S1-006).

## Complexity Tracking

None: no golden rule is waived.

## Risks

| Risk | Response |
| --- | --- |
| Reranking does not fit 1.5 s at the design's depth | The depth is measured (R8) and set on the ladder; a shallower rung ships only if it pays for itself |
| The router change waits on review | It is small and lands first; search works without it but reports the dense route and reranking unavailable whenever room is short |
| Beside the largest chat models, free room holds the embedder but not the reranker too | A free-room request unloads nothing, idle guests included, so reranking is flagged unavailable (FR-S1-015a); T008 and the ladder measure how often, and if it costs the quality target, a free-room request may replace an idle guest |
| The lexical analyzer fits T004's sample but not the corpus | The ladder measures the BM25 route alone on the golden set (FR-S1-005a); the profile is versioned, so a better analyzer is a new generation |
| An agent-drafted golden set misses real questions | Stratified drafting, the owner's 30-question check, and real questions added as they come (risk R8 of the roadmap) |
| The organization's gate moves under S1 (rust-workflows went from 2.5 to 4.3 in a day) | Each S1 branch rebases on `main` and runs `just check` before its push; the lints and rules only get stricter |
| Model downloads need the owner's approval | Candidates are listed with size and licence before the bake-off; nothing downloads without approval |
| The corpus manifest lacks language and capture time | Left out, never guessed; the analyzer policy and version filters do not depend on them |
