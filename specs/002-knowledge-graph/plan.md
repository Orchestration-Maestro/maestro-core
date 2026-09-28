# Implementation Plan: Knowledge graph

**Branch**: `docs/s2-spec` | **Date**: 2026-09-28 | **Spec**: [spec.md](spec.md)

**Input**: the approved S2 draft and its 2026-09-28 01:56 owner decision.
Tasks: [tasks.md](tasks.md). D1–D5 remain decided; the revised 100-question
suite and precision-review protocol are pending owner confirmation. This plan
includes the supervisor's review ruling and cross-slice projection seam.

## Summary

Build a small sourced graph before adding model extraction or graph fusion.
Run `lbug` qualification (G25) first in the task list, in parallel with the
SQLite pilot: import one approved subset/version, extract one `DEFAULTS_TO`
table rule and return scoped neighbors with exact quotes (G01–G05). The pilot
waits for neither G25 nor M1 release; it needs no model/service and is not M2.

SQLite claims and artifacts remain authoritative. LadybugDB is the only S2
traversal engine: an in-process Cypher projection, rebuilt from frozen kernel
claims. After G11 replaces the pilot read, a selected but missing graph is
unavailable, not a fallback; graph `none` is disabled without calls. Reuse S1
jobs, evidence, evaluation, CLI and MCP. G27 exposes the shared application-ID
typed-edge port; deployment-modes D07 later wraps it for backend choice,
including a separately planned user-selected Neo4j adapter. M2 requires measured
gain, complete proofs, three-platform qualification and delete/rebuild equality.

## Technical Context

**Language/Version**: Rust 1.98.1, edition 2024; workspace MSRV 1.98.

**Primary Dependencies**: existing `rusqlite`, `serde`, `serde_json`, `schemars`,
`tokio`, `clap`, `rmcp`, `qdrant-client` and the model gateway. Only `lbug` is
newly approved. G25 records its exact pin, minimum features, native links,
licences, added crates and forced duplicate versions before adoption. Do not
infer a compatible version from an older architecture example.

**Storage**: kernel SQLite in WAL mode and content-addressed artifacts; one
owned graph area below the resolved data directory. File layout and process
ownership follow G25's safe mode. Qdrant Server stays the passage projection.
No engine file becomes a backup authority.

**Testing**: focused Rust tests first, synthetic public integration fixtures,
real native processes for locking/crash tests, private scratch-restore
runs. New knowledge integration cases are modules in `tests/it/main.rs`'s one
test binary. The large scale profile is ignored by default and run explicitly,
not on each mutant. CI owns native/coverage/mutation gates; public CI needs no
model or private collection.

**Target Platform**: Linux, Windows and macOS, as ADR-0018 requires. G25 must
prove C++20/CMake with GCC at least 13 on Linux and corresponding supported
Windows/macOS toolchains, plus local Clippy for `x86_64-pc-windows-gnu` and
`aarch64-apple-darwin`. Unsupported cross compilation cannot be silently skipped.

**Project Type**: existing Cargo workspace; libraries plus CLI/MCP binary.

**Performance Goals**: p95 graph at most 500 ms, graph search under 2.5 s and
complete ask under 10 s on the reference workstation. Profile synthetic
10,000/100,000-edge graphs and separate cold, warm and unavailable cohorts.

**Constraints**: no new daemon, graph port, Docker, JVM, unsafe wrapper or
first-use download. No engine/model I/O inside SQLite transactions. Every
result rechecked by the kernel; private content never enters this repository.
Local depth at most two, path length four, evidence 50; finite expansion/time
caps and cancellation are mandatory. G11 records the measured expansion cap in
its query profile; reaching it discloses incomplete coverage.

**Scale/Scope**: one approved subset/version for the pilot; provisionally 100
FR/EN acceptance questions, twenty per type, all held out (80 answerable,
one independent family per item). Suite size is pending owner confirmation.
Development uses pilot/synthetic cases only. Source scope/counts/receipts stay
private. The estimate assumes 4B extraction; Qwen3-8B or an engine re-plan needs
a revised estimate.

## Starting point

G01 rechecked `origin/feat/s2-integration` at `821851a` on 2026-09-28.
This is source inspection, not a claim that M1 or native qualification passed.
Recheck moving seams at later dispatches; no migration number is allocated here.

| Existing seam | S2 use |
| --- | --- |
| `crates/maestro-kernel/src/store/migration.rs` | Forward-only migrations; `0011_exact_identifiers.sql` is already used. Each S2/S3 migration takes the next free number at landing, never a reserved or gapped number. |
| `crates/maestro-kernel/src/evidence/resolve.rs` | Authorized original-byte/digest checks; add claim support reads, not a kernel dependency on canonicalization. |
| `crates/maestro-knowledge/src/search/evidence/section_reader.rs` | Reuse the existing evidence section reader and canonical block/source mapping. |
| `crates/maestro-knowledge/src/search/request.rs` | `EvidenceInput` already carries pinned generation, principal, scopes, ranked candidates and deadline; add graph proof metadata here. |
| `crates/maestro-knowledge/src/search/fusion.rs` | `Route` has Dense, Lexical, Identifier, Structured only; extend it once with Graph and preserve one-based RRF. |
| `crates/maestro-kernel/src/evidence/bundle.rs` | `/1` accepts only V1 and denies unknown fields; graph data needs a separately versioned `/2` boundary. |
| `crates/maestro/src/knowledge/output/policy.rs` | Shared CLI/MCP 65,536-byte response policy drops passages today; S2 must drop complete proof groups instead. |
| `crates/maestro/src/cli/args.rs`, `crates/maestro/src/knowledge/operations/search.rs`, `crates/maestro/src/knowledge/operations/ask/run.rs` | Search/ask already exist, with pinned model cards and final scope checks; MCP dispatches both in `crates/maestro/src/mcp/server/handler.rs`. Extend these operations, not a second CLI. |
| `crates/maestro/src/cli/backup/command.rs`, `crates/maestro/src/cli/backup/restore.rs` | Read-only SQLite online backup plus digest manifest/artifacts; restore validates into an empty slot and installs the database last. Reuse for isolated scratch runs, not the live kernel. |
| `crates/maestro-kernel/src/scope/grant.rs`, `crates/maestro-kernel/src/generation/lifecycle.rs` | `visible` reloads current grants; scoped generation reads and `publish_generation_if_current` already exist. Preserve request pins and recheck grants; graph attachment is new S2 work. |
| `crates/maestro-kernel/src/gateway/port.rs` | Chat requires Answerer and caps output at 1,024 tokens; add a constrained Extractor call without weakening chat. |
| `crates/maestro-kernel/migrations/0009_model_cards.sql` | Three role CHECK constraints in cards/evaluations/selections; preserve immutable history and real-evaluation selection guards. |
| `crates/maestro-knowledge/src/index/rebuild.rs`, kernel journal cursors | Reuse snapshot, job and progress patterns, not Qdrant aliases or live-file copying. |
| `crates/maestro/src/cli/eval/command.rs` | Existing `eval ladder --manifest`; extend evaluation rather than assume a nonexistent generic `eval run` command. |

No graph module, S2 migration or lbug dependency was present at that head.
The separate G25 lane writes the supervisor's G25 report and publishes
sanitized evidence in `specs/002-knowledge-graph/research.md`. That is an
execution output, not a passing result supplied by this plan.

## Constitution Check

The organization's golden rules govern through
[the repository rule map](../../docs/standards/engineering.md) and ADR-0017.
Recheck this table after G25 and before release.

| Rule | Plan disposition and proof obligation |
| --- | --- |
| Simplicity, YAGNI, rule of three | Rules-first pilot; reuse S1's runner; one loader and one engine. G27's typed-edge API serves S2/S3; deployment-modes D07 adds the backend-choice `GraphStore`, not a second engine in S2. |
| One authority, least privilege | SQLite claims; per-hop filters before limits and fresh kernel rechecks of every delivered result. G02/G11/G13/G15 test refusals. |
| Test first, never weaken a gate | Every task starts with a failing contract/test; three-platform gates remain mandatory. G25 cannot waive native code coverage. |
| Pinned inputs, ADR-0020 | Exact lbug/features/tree and model/profile digests; named dependency exceptions with removal conditions, licences and vet evidence. |
| Truthful/private evidence | Public synthetic tests; private vendor receipts; all failed attempts retained; queued CI and a valid quote are not qualification or semantic truth. |
| Source architecture | No kernel-to-canonicalization dependency; no file over 500 counted code lines, import cycles or executable child `mod.rs` files. |

No gate exception is approved. A measured DEP-001 duplicate is a named,
expiring library exception under ADR-0020, not permission to relax the rule.

## Project Structure

### Documentation (this feature)

```text
specs/002-knowledge-graph/
├── spec.md
├── plan.md
└── tasks.md
```

G25 adds `research.md` with measurements. G24 adds `acceptance.md` with release
evidence. Data model, contracts and operating sequence live in this plan until
a real implementation needs a separate file; no empty scaffolding is created.

### Source code (repository root, added only when needed)

```text
crates/maestro-kernel/
├── migrations/         # NNNN assigned at integration, in landing order
└── src/
    ├── facts/          # authoritative claims, supports, builds, scoped reads
    ├── evidence/       # graph /2 contract beside the unchanged /1 contract
    ├── gateway/        # extractor role and constrained call
    └── model/          # existing registry, extended rather than replaced
crates/maestro-knowledge/src/
├── graph/
│   ├── rules.rs        # strict data-only rule pack
│   ├── verify.rs       # block/window ownership and quote location
│   ├── build/          # leased extraction with frozen inputs
│   ├── projection/     # port.rs API, writer/schema, one batch loader
│   ├── query.rs        # bounded graph operation orchestration
│   ├── cypher.rs       # parameterized templates and native cancellation
│   ├── resolve.rs      # sourced exact names and reviewed aliases
│   ├── extract/        # offline model windows and retained rejections
│   └── consumer.rs     # foreground cursor-driven projection effects
├── eval/graph/         # labels, complete-proof scoring and comparisons
└── search/evidence/    # whole proofs added to existing assembly
crates/maestro/src/
├── cli/graph/          # graph build/read/rebuild commands
├── knowledge/          # shared operations and output bounds
└── mcp/                # four graph tools over the shared operations
```

`mod.rs` files contain declarations/re-exports only when they have children.
Tests live beside their source, with process/rebuild contracts under the
existing crate integration-test layout. Exact files are assigned per task.

**Structure decision**: keep all three S1 ownership boundaries. The private
collection owns its rule pack, prompt, proof labels and receipts. The probe workflow lives in maestro-core, not the reusable workflow repository.
If toolchain support needs a reusable workflow change, its release and core
re-pin are a supervisor-owned external step with its wait recorded. No worker
edits another lane's clone.

## Design

### A0 Frozen pilot contract and private-scope proposal

G01 freezes one public synthetic table, not a production extractor:
`tests/fixtures/synthetic/graph/defaults.md` and `defaults.json`. The Markdown
is a separate graph development fixture, not an addition to S1's retrieval
corpus or a held-out acceptance item. Its SHA-256 is
`8cfbf93dbaa5dc25bf9c3a6d88f1b698a19c7c79c6bb832320977e95220766a3`.
The JSON rule/oracle SHA-256 is
`3443fe932c03e9042e08514c33b4034fc18cf302b2e296cdfa44531adcfefbf9`.
Git preserves LF bytes on all three platforms; a later fixture change needs
an explicit reviewed re-freeze, never a silent oracle update.

The JSON envelope separates `rule` and test-only `expected`. G03's production
parser consumes only the standalone `rule` object, never this envelope or its
oracle. The test harness passes that object; the private rule file has that
same standalone shape, with no `expected` key.

- `rule` is closed data only: schema `maestro-graph-table-rule/1`, `id`
  `synthetic-defaults/1`, the source digest above as `source_sha256`, exact
  heading path `Lantern controller` / `Parameters`, ordered columns
  `Parameter`, `Type`, `Default`, subject kind `Parameter` and predicate
  `DEFAULTS_TO`. Role bindings are explicit: `subject_column: Parameter`,
  `type_column: Type`, `lexeme_column: Default`; each must name a distinct
  declared header. No scripts, expressions or inferred column roles.
- Each row supplies its literal type explicitly. The four expected defaults
  are `label` → text `café`, `enabled` → boolean `true`, `retries` → integer `3`
  and `ratio` → decimal `0.50`. Keep each lexeme unchanged; no guessed units,
  floating-point rewrite or literal/Document/Section projection nodes.
- `expected` is a test oracle, never an input granting claim authority. Each
  entry pins a complete original row (newline included), its half-open UTF-8
  span and quote digest. The fixture check resolves the actual canonical table
  and row blocks, not a second Markdown parser. G03 must retain those revision/
  block references; G02 independently verifies authority before admission.

`graph_fixture` checks both file digests against constants and this A0 freeze,
source/quote digests, canonical row ownership, nonempty UTF-8 spans, explicit
column bindings, all default values/types and complete row accounting. Closed
rule checks reject unknown/executable/oracle and duplicate keys. It also checks
the requirement table against every task's Acceptance line exactly, including
missing, extra and duplicate ownership rows. G03 adds the production closed-rule
parser and extraction; G04 proves CLI neighbors.

**Temporary import metadata:** G03 and G04 build their import fixture in a fresh
temp directory, without changing S1's collection or corpus manifest. Copy the
frozen Markdown byte-for-byte to `corpus/graph/defaults.md`. Bind
`synthetic_graph_root` to the temp directory and write `collection.json` with:

```json
{
  "schema": "maestro-collection/1",
  "id": "synthetic-graph",
  "title": "Synthetic graph defaults",
  "visibility": "public",
  "profiles": {
    "extraction": "technical-html/1",
    "chunking": "structural-500-700/1",
    "embedding": "embed:winner",
    "sparse": "bm25-en-fr/1"
  },
  "quality": {"ledger": "quality/ledger.jsonl"},
  "sources": [{
    "id": "defaults", "kind": "import", "sync": "manual",
    "manifest": {"binding": "synthetic_graph_root", "path": "corpus/maestro-corpus.jsonl"}
  }],
  "evals": {"suite": "evals"}
}
```

Write `corpus/maestro-corpus.jsonl` as one `maestro-corpus/1` line with fixed
`path: graph/defaults.md`, `sha256` equal to the frozen Markdown digest above,
`bytes: 245`, `source_ref: corpus-path:graph/defaults.md`,
`title: Lantern controller parameters`, `version: 1.0`, `set: graph` and
`source_kind: reference` (all except `bytes` are JSON strings). These constants
belong to G03/G04's test helpers, not a new checked-in corpus. Version-selection
cases start from `1.0`; any deliberately different test revision must have its
own explicit metadata. This imports only development data, not acceptance data.

**Private proposal, not permission:** collection `ctm`, one official
parameter-reference document at one owner-selected release in **9.0.22+**,
with one parameter/default table. The proposed extraction window is that
complete table (header and all body rows) with its nearest heading as context;
no other document category, version or surrounding prose window is included.
Actual source identity, exact release, row count, byte windows and digests must
be frozen privately before reading. No private material was inspected by G01.

The supervisor-approved public reference is the **planned, unverified** binding
`PRIVATE/graph/receipts/pilot-inputs.json`. No S1 receipt applies. G05 and G07
remain blocked until the owner confirms their private scope; a pilot receipt
alone does not authorize a broader acceptance corpus. Keep receipt bodies,
rules, quotes and review results private. G05 reviews every accepted relation
and retains failures on a named backup's isolated scratch restore. This fixture
freeze proves neither private pilot success (SC-S2-001) nor M2.

### A1 Qualification before adoption

G25 is a four-hour investigation timebox, not a promise that queued CI ends
within four hours. It returns `pass`, `fail` or `blocked`, with versions,
commands and CI URLs. The supervisor set this six-row adoption bar before the
verdict; a miss fails/blocks, never waives a gate:

| Check | Pass bar |
| --- | --- |
| Bundled source | The pinned crate bundles native source; after fetching locked Cargo inputs, native build and runtime need no network or first-use download. |
| TLS linkage | No system OpenSSL dependency introduced by lbug. Record actual native links. |
| Build and cache cost | Added clean CI build time is at most 15 minutes over the same baseline. An unrelated Rust/document change must not rebuild liblbug. Measure clean/warm time, binary delta, added time per mutation shard and coverage run with the real cache. Mutation shards must fit their 30-minute deadline; local peak memory stays within 8 GiB with `CARGO_BUILD_JOBS=3`. No invented binary-size or warm-time ceiling. |
| Platforms and behavior | Linux evidence now; Windows/macOS native CI evidence or a supervisor-approved dated plan permits implementation. The mandatory behavior matrix below must actually pass on all three OSs before M2. Both local cross-Clippy targets need a supported gate-preserving recipe. A plan is not a passed test. |
| Licence | Record Rust/native licences and notices and pass the organization licence policy; unresolved obligations block adoption. |
| Dependency cost | Minimum features, metadata and feature tree measured; at most one forced duplicate from lbug, with a named DEP-001 exception, forcing library, removal condition and vet evidence. |

On each native OS, prove G27's Cypher types/bound parameters and rollback,
the single parameterized-batch loader, bounded paths filtered at every hop,
native cancellation, a writer beside separate CLI and MCP reader processes,
second-writer refusal, and kill/reopen. Test same-file locks and the intended
mode: a writer owns unpublished files, readers open immutable published files
and retain old pins. No unsafe wrapper, bypassed lock or coordinating daemon.

Put the probe in `.github/workflows/lbug-qualification.yml` in maestro-core
and its tests under `crates/maestro-knowledge/tests/it/`, not extra linked test
binaries. Keep native all-feature coverage. If the reusable toolchain needs a
change, the supervisor releases it and re-pins core; record that wait, not a
waiver. Publish the six-row result to the supervisor's G25 report.

The G25 checkpoint gates G11, G21, G22 and G26–G30 (G29 is removed), not the
SQLite pilot or model/evaluation groundwork. Linux plus the approved dated
platform plan permits implementation only; actual native and CI cost evidence
still gates M2. Other missing evidence is blocked.

A qualification failure triggers an S2 re-plan ruling, with new driver and
operations estimates if Neo4j is chosen. Separately, approved deployment-modes
work adds a later user-selected external Neo4j adapter. Neither is a runtime
fallback; G25 failure is not a prerequisite for that separate planned adapter.

### A2 Claims and source verification

Store immutable claims, not model conclusions. The rule/extractor submits a
candidate through knowledge verification, then the kernel's scoped write path.
Knowledge reads canonical blocks to verify revision/block/window ownership and
locate the claimed quote in original source bytes. The kernel independently
checks the authorized revision, eligibility, original digest, UTF-8 boundaries
and exact slice/quote digest. It does not import canonicalization types.

A quote without a unique authorized location is rejected; when explicit span
and block identify one occurrence, another identical quote elsewhere is not a
reason to change its location. Normalized/rendered text cannot be substituted
for original bytes. The quote verifier never decides whether a relation is
true: semantic validation and review state stay separate from byte validity
and model confidence.

| Authoritative record | Required content and invariant |
| --- | --- |
| Entity | Collection/application ID, kind, normalized name and exact source spelling; matching all four identity components resolves reversibly to one entity across documents. |
| Claim | Application ID, typed subject/predicate/object, conditions/environment, version and world-valid bounds, record time, extractor/profile and review state. `DEFAULTS_TO` has a literal object, not another entity. |
| Support | Claim ID, revision/block ID, original half-open byte span, quote digest; nonempty verified supports before acceptance. |
| Mention/alias/review | Source support, proposed/resolved identity, decision and supersession history; ambiguity is retained. |
| Claim set/build | Frozen input/profile digest, ordered claim membership, lease/checkpoint/budgets and rejection receipts. |
| Graph attachment | Collection and kernel generation, verified claim-set/profile digest, attached once; later input needs another generation. |
| Projection receipt | Attachment identity, schema/import profile, application-ID/count/content digests and close/reopen verification; readiness is kernel-controlled. |

A matching normalized name alone is insufficient: different source spellings
that normalize alike stay distinct/ambiguous for review; the same name in two
kinds is never automatically merged. Decisions are reversible and source-backed.
A `DEFAULTS_TO` object is a typed literal (text, boolean, integer or decimal)
with its exact source lexeme. No floating-point rewrite of a decimal or guessed
unit/type is allowed. Literals are claim properties, not projection nodes;
Document/Section nodes are not projected either. Claims keep revision/block
references, and literal-valued defaults do not create entity-to-entity paths.
G03 and G10 test matching/colliding names, literal objects and this node boundary.

Version ranges are half-open where ordered bounds are known. Unknown validity
is explicit, never replaced by capture time. Conditions are typed data from the
source/profile, not executable expressions. A query that cannot establish a
condition retains that uncertainty instead of claiming unconditional truth.
Contradictions, withdrawals and supersession remain recorded; new generations
change membership, never rewrite the evidence of an old pinned generation.

### A3 Build, publication and recovery

Reuse S1 jobs, leases, resource ownership and journal checkpoints. Freeze the
source set, rule/prompt/card digests, window bounds and work/token budgets at
submission. Work outside transactions; atomically record only verified results
and progress. An expired worker cannot attach results after takeover.

The pilot attaches verified claims for scoped SQLite one-hop reads. The full
projection uses one G25-qualified loader: bounded parameterized batches from a
frozen kernel snapshot into fresh unpublished files. G28 owns this loader and
its checkpoint/restart checks; G29 is removed. Bound values handle Unicode and
quotes without CSV escaping or a new library. No live engine-file copy.
Every newly attached generation gets a complete build; there is no separate
incremental loader. A second loader is outside S2. Any later proposal requires
a supervisor-set 100,000-edge build-time ceiling registered before measurement,
and an observed miss, before a new implementation ruling.

Flush, close and reopen with the qualified mode, compare schema/indexes,
application IDs, counts and content/profile digests, then publish readiness in
a short kernel transaction with the expected lease/generation. New generation
publication cannot change a request's existing pin. Cleanup drains readers and
removes only owned disposable files, preserving retained generations and other
collections. No engine handle is opened from a caller-supplied path.

Foreground consumers read S1 events after commit and schedule the same full
build for each newly attached generation. Deduplicate durable build effects and
ack only after their receipt is durable. Lost files after cursor advancement
are recovered from the complete frozen snapshot, not only the event tail.
Telemetry remains local, redacted and separate from authoritative progress.

### Cross-slice projection seam

G27 owns the public typed-edge API at
`crates/maestro-knowledge/src/graph/projection/port.rs`. Other slices project
and read their own frozen, kernel-authoritative typed edges through operations
on collection/generation/application IDs and explicit edge families. Scope and
eligibility constraints stay inside reads. No raw Cypher, engine IDs or lbug
types escape. All lbug calls stay in `G/projection/` and `G/cypher.rs`.

S3 C27a depends on G27 for catalog dependency edges. Those edges name catalog
records, not documentary source spans; they never become evidence-span claims
or enter a knowledge proof. A graph projection grants neither claim admission
nor catalog write authority. Keep the family discriminator through write,
read, verification and readiness checks to prevent cross-family confusion.

The supervisor accepts this API boundary as deployment-modes D07's seam
agreement. That task later wraps it in `GraphStore` for lbug/none and backend
choice; S2 implements neither that trait nor the Neo4j adapter. Graph `none`
is disabled with zero calls, including opens/probes, distinct from a selected
but unavailable graph. The deployment mode where the entire knowledge core is
not installed is a separate concern, not a new SQLite retrieval fallback.

### A4 Reads and complete proofs

G04's pilot is a scoped indexed one-hop kernel read. G11 removes that read as a
runtime traversal alternative; SQLite retains claims, export and evidence
checking, not recursive path search.

At admission, capture the published collection generation, immutable graph
attachment, principal, scopes, version/conditions and absolute deadline. Cypher
uses bound values and fixed predicate/type choices, applying all admission
filters at each hop **before** shortest-path selection, ordering and limits.
Enumerate only bounded admissible candidates; never select an unfiltered
shortest path then discard its hidden middle. Bound native expansion as well
as returned rows, and cancel the native query when its deadline expires.

Before delivery the kernel checks every returned entity, claim and support,
current permissions and source eligibility. A projection receipt or earlier
scope snapshot does not replace this check. Unknown and forbidden requests
share error shapes and disclose no counts, aliases or hidden intermediates.

R4 resolves exact identifiers/names and reviewed aliases from the question
only. FR/EN deterministic linking precedes any model. Simple lookup keeps S1
routes; relationship/path questions add R4. Each candidate retains its whole
claim path and support IDs through one-based RRF (K = 60), stable ties and
reranking. A graph echo of its seed is explanatory, not corroboration. No
transitive relation is inferred merely because two edges form a path.

Evidence assembly remaps original support spans to the pinned chunk set,
unions safe overlaps and protects all necessary links/conditions/versions.
Rank, evidence budget and final 64 KiB response policy operate on whole proof
groups. Charge graph proof metadata as well as passages; keep exact versus
estimated token accounting explicit. If no complete group fits, disclose a
sanitized known gap and refuse a conclusion requiring it. Nothing reports a
missing relation as corpus-wide absence.

### A5 Wire and application contracts

| Contract | Change |
| --- | --- |
| `maestro-evidence/1` | Keep non-graph semantics/round trips. Unknown graph fields remain rejected; no silent extension of its closed decoder. |
| `maestro-evidence/2` | Versioned graph evidence: pinned collection/generation/claim set, claims, ordered paths, support references into held passages, coverage, gaps and retained S1 provenance. Reject dangling/duplicate references or mixed generations. |
| `EvidenceInput` | Carry graph proof groups and attachment identity beside ranked candidates; never reconstruct a chain from final passage rankings. |
| CLI graph operations | `knowledge graph build`, `neighbors`, `path`, `resolve`, `trace`, `rebuild`; shared application operations, versioned JSON, existing 0/1/2 exit conventions. |
| MCP | `knowledge_graph_neighbors`, `knowledge_graph_path`, `knowledge_entity_resolve`, `knowledge_evidence_trace`; bounded arguments, explicit version/generation, no query-language tool. |
| Graph evaluation | Only `eval graph check --manifest` is new. The existing `eval ladder --manifest` runs/comparisons gain graph scores and route rungs; there is no second runner. |
| Model extraction | Distinct Extractor role and closed-schema call; 1,024-token maximum, pinned card/sampling, `Room::Free`; invalid partial JSON produces rejection, not partial accepted output. |

The four graph reads accept collection plus their entity/name/claim selectors;
local depth is bounded to two and path length to four. Output includes ambiguity,
coverage and source evidence. All transports share the same grants, deadline,
whole-proof truncation and final recheck; MCP annotations are not authorization.
G12 implements the versioned contract, G13 its budgeted assembly and G15 its
transport policy, before G16 relies on it for answers.

### A6 Models and evaluation

G17 adds Extractor to cards, evaluations and selections with a forward migration
that preserves S1 history, foreign keys and real-evaluation guards. G18 adds the
constrained gateway call; Answerer chat does not become an arbitrary-role call.
G19 chooses bounded offline windows only from pilot/synthetic development
failures, never held-out acceptance labels or failures. Retain rejections and
reuse the rule verifier. Source text is data; injection cannot change scopes,
bounds or predicates.

Compare deterministic rules with rules-plus-Qwen3-4B over three runs. Use one
construction scorer alongside the existing ladder's retrieval/answer scoring.
Precision review by a model other than the extractor, with owner rulings on
every flagged claim, is pending owner confirmation. Cover every accepted
held-out claim and use the 95% point-estimate floor; keep semantic precision/
recall separate from byte validity, tokens, throughput and VRAM. Only real
eligible evidence can select a card. A conditional Qwen3-8B trial is a new
download after measured recall failure at fixed precision and a new estimate;
never substitute the 27B router entry or enlarge the answerer.

G06 extends the existing evaluator and CLI ladder with graph labels/checks and
three separate score layers. Each frozen question records required semantic
links, conditions/version, original source anchors, acceptable alternative
complete proofs and an unanswerable reason when appropriate. Complete-proof
recall counts a question only when one full allowed proof is retrieved; partial
chains and duplicate alternate copies earn no extra complete-proof credit.
Construction precision judges relation semantics, not just matched text.

G07 provisionally drafts 100 FR/EN questions, twenty per type, pending owner
confirmation. All 100 are held-out acceptance, with 80 answerable independent
families and twenty unanswerable ones. Equivalent versions/alternate copies
stay within one item; duplicates cannot inflate the effective sample size.
Development uses only pilot/synthetic cases, not a slice of those 100. G08's
different-model review checks every chain; the owner rules on flagged wording/
answerability. Freeze suite/corpus/profile/family digests before acceptance.

As a supervisor execution choice, G08 freezes the S1 default at-most-4B
answerer card by digest as it stands at freeze, including its reasoning,
sampling and output settings; G23 uses exactly that card. Thinking off is S1's
current default, not an owner ban on a separately measured thinking candidate.
Do not change the frozen answerer to obtain graph gain.

Use the existing `eval ladder --manifest` runner for all three G23 rungs:
**Qdrant-only / LadybugDB-only / pairing** on the same binary and frozen
scratch inputs, without duplicate embeddings. Pairing's comparator is the
same-run Qdrant-only rung; G08 is a separate historical drift check. Alternate
run order, keep every attempt and apply the spec's exact 2,000-resample,
seed-0 paired percentile rule: 50th ordered delta strictly positive and observed
gain at least 0.05. Four wins and no losses among 80 prove the five-point
boundary in G06's synthetic check; three wins fail. No pooling three repeats as
240 independent questions.

Every run must separately pass relation precision, exactness, refusal on all
twenty unanswerables, and non-decreasing retrieval/supported-answer point
estimates versus its same-run Qdrant-only rung. Warm private-graph p95 uses
`retrieval.route.graph` and each graph tool's server time, plus separate
end-to-end search/ask limits; report cold and unavailable cohorts apart. Retain
per-type/stage results and compare `ctm-retrieval` on the same-run rungs.
All three runs must pass; missing, failed or inconclusive evidence blocks M2.
Disable losing fusion without changing the thresholds.

### A7 Local operations and equality proof

Package the G25-qualified runtime with explicit owned directories and assets.
`setup` previews changes; read-only health diagnoses missing runtime/files,
locks, permissions and corruption. Record installation, open/reopen, binary,
RSS and disk measurements. Offline rebuild/removal instructions preserve the
kernel and drain readers; a graph server or first-use network fetch is not a
repair strategy.

G30 freezes kernel state and a query matrix including hidden hops, versions,
contradictions, multiple collections and retained generations. Save ordered
neighbors, paths, claims, evidence and coverage; close all handles and drain
readers; delete only owned disposable LadybugDB files; rebuild from the kernel
without models, Qdrant claims or graph backup; compare exactly, excluding only
timing and transport IDs. Repeat the same loader after interrupted batches.
G22 repeats after SQLite/artifact backup/restore, with interrupted and corrupt
states and separate writer/CLI/MCP processes on all three operating systems.

## Migration landing order

Each migration takes the **next free number when the supervisor integrates
it**. S2, S3 and deployment modes coordinate at that landing; no fixed number
is reserved and no kernel applies these migrations in gapped order. `NNNN`
below names that integration-assigned number, not an executable filename.

| File in `crates/maestro-kernel/migrations/` | Owner |
| --- | --- |
| `NNNN_graph_claims.sql` | G02: claims, endpoints, supports and frozen membership. |
| `NNNN_graph_builds.sql` | G09: build/checkpoint and once-only attachment guards. |
| `NNNN_graph_resolution.sql` | G10: sourced resolution/review and supersession. |
| `NNNN_extractor_role.sql` | G17: all three role constraints and preserved registry guards. |
| `NNNN_graph_projection.sql` | G27: projection receipts/readiness. |

Register each in `src/store/migration.rs`; after assigning its number and
rebasing, rerun upgrade/rollback and compatibility checks against the integrated
head. Never renumber an already integrated migration or rewrite S1 history.

## Validation

### Scratch isolation before every private run

G05, G08, G20 and G23 restore a **named, digest-checked S1 backup** into a new
scratch kernel; they never open the owner's live kernel with an S2 binary.
`Database::open` applies forward migrations, and the old `maestro-s1` then
refuses that file with `UnknownMigration`. A restore is not a casual undo.

Before invoking any S2 command, the private scratch receipt records the backup
ID/digest, binary commit, absolute data/config directories, private Qdrant
instance/storage/ports and its owned collection/alias names. Check resolved
paths/endpoints against the live bindings and refuse overlap or missing values.
Use a separate Qdrant instance: projection names are scoped by its endpoint;
no live alias is moved. Preserve authoritative collection IDs in the restored
kernel, and record the scratch endpoint/name pairs explicitly.

Bind `XDG_DATA_HOME` and `XDG_CONFIG_HOME` to the receipt's scratch roots and
`MAESTRO_QDRANT_URL` to its private endpoint for **each** command, with required
nonempty absolute path bindings and no default fallback. Keep copied grants and
artifact bindings inside that restore. No automatic setup/doctor call may
connect to or migrate the live installation. G22 runs a retained S1 binary
against an S2 scratch kernel: it must refuse with `UnknownMigration` and leave
it unchanged. The drill's recovery receipt/diagnostic names the next action:
update the binary or restore the named pre-upgrade backup. Current kernel/CLI
errors gain that guidance with tests; do not claim the historical executable
prints new text. Never attempt backward SQL migration.

Only at G24, after S2 lands and `maestro-s1`/clients are updated, does the owner
authorize a live upgrade after a fresh named backup. Lanes do not upgrade it.

### Acceptance sequence

1. **Pilot and qualification in parallel:** the pilot uses integrated S1 and
   SQLite; engine tasks wait for G25's reviewed adoption bar, not M1 release.
2. **Pilot:** synthetic import → rule build → scoped neighbors, then the
   approved private scratch subset with every claim independently checked.
3. **Proof safety:** hidden-hop, revocation, version/condition, generation,
   fan-out, cancel, rechunking and whole-proof budget/wire checks pass.
4. **M2 quality:** G08 drift receipt, construction score and the same-run
   three-rung comparison meet all gate rules in all three runs. G23 need not
   wait for G22's recovery drill; both still gate release.
5. **Operations/release:** G30 equality, restore, real three-OS CI, Pi/Claude
   Code smoke, coverage and zero-miss/zero-timeout mutations. G24 finalizes the
   ADR against G25 and maps integrated evidence; a platform plan cannot pass M2.

## Delivery and estimate

The [29 tasks](tasks.md) keep stable IDs, with **G25 first** and G29 removed.
Each is at most four hours including targeted gates. Total: **108 lane-hours**;
the independent SQLite pilot is **18 hours**, while G25 runs beside it. G07 and
G08 each grow to four hours for the larger suite; dropping G29 saves four.
Six lanes at eight hours/day give a 2.7–2.9-day arithmetic floor with the
20–28-hour native CI/review reserve (128–136 total), not a delivery promise.
External reviews, owner confirmations and CI waits keep the planning estimate
**9–12 working days**, pilot **3–4**. Dependency-only critical path: **52 hours**:
G01 → G02 → G03 → G04 → G05 → G10 → G27 → G28 → G11 → G13 → G14/G15 → G16
→ G22 → G24. G14/G15 tie; G23 starts after G08/G16/G20, in parallel with G22.
Both must finish before G24, which waits for every task.

Split an overrun into independently testable tasks before dispatch, without
waiving acceptance. At most six lanes; GPU and projection writers are serialized
where their resources conflict. The supervisor owns dispatch/review/integration;
lanes never start subagents. Shared-file collisions are not parallel work.

## Complexity Tracking

None approved. The pilot's SQL read is deliberately temporary, not a fallback
architecture. New files appear with working behavior; existing S1 helpers and
contracts are extended rather than cloned.

## Risks and deferrals

| Risk | Early evidence or boundary |
| --- | --- |
| Native build cost or unsupported process mode | G25 fails/blocks before production adoption; no lock bypass or skipped gate. |
| False claims or incomplete proofs | G05 semantic review; G11/G13/G15 hidden-hop and budget tests; G20 precision. |
| Biased graph gain | Independent full review, all acceptance families held out, same-run paired intervals; inconclusive fails. |
| GPU/review contention | Free-room admission, bounded offline work, retained failures and explicit waits. |
| Unrelated scope entering S2 | Qdrant Edge, release-delta work, heading cleanup, general calibration, privacy scanner and source-rule polish remain separate. |

S1's graph-specific duplicate/support handling is covered in G10/G13 and graph
budgets in G13/G23. This does not implement general release-delta deduplication
or republish cleaned headings. Catalog, acquisition, UI and advanced graph
methods remain outside S2. Qdrant Server is unchanged while Edge is evaluated
on its own evidence.
