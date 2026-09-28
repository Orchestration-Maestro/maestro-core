# Implementation Plan: Knowledge graph

**Branch**: `docs/s2-spec` | **Date**: 2026-09-28 | **Spec**: [spec.md](spec.md)

**Input**: the approved S2 draft and its 2026-09-28 01:56 owner decision.
Tasks: [tasks.md](tasks.md). D1–D5 remain decided; the revised 100-question
suite and local precision-review protocol remain pending owner confirmation.
U2's private scopes and I1's development-only 8B trigger were approved at 12:53.
This plan includes
the supervisor's review ruling and cross-slice projection seam.

## Summary

Build a small sourced graph before adding model extraction or graph fusion.
Run `lbug` qualification (G25) first in the task list, alongside public claim,
rule and model-role work. The rule-only pilot imports one approved
subset/version, extracts `DEFAULTS_TO` and returns scoped neighbors with exact
quotes through LadybugDB. G04 waits for G25/G27/G28's qualified port and single
loader; it needs no model/service and is not M2. No task waits for M1 release.

SQLite claims and artifacts remain authoritative; SQLite never answers graph
queries. LadybugDB is the only S2 neighbor/path engine from the pilot onward:
an in-process projection behind G27's port, rebuilt from frozen kernel claims.
G11 extends the same engine with bounded Cypher paths. A selected but missing
graph is unavailable, not a fallback; graph `none` is disabled without calls.
Reuse S1 jobs, evidence, evaluation, CLI and MCP. G27 exposes the shared application-ID
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
| `crates/maestro-kernel/src/store/migration.rs` | Forward-only migrations; G02 occupies `0012_graph_claims.sql`. G31 and every later S2/S3 migration take the next free number at landing, never a reserved or gapped number. |
| `crates/maestro-kernel/src/facts/{read,quote}.rs` | Landed G02 claim authority and original-byte quote checks; G04 reuses these, never `evidence/claim_support.rs` (an import cycle) or SQLite graph queries. |
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

No graph module, S2 migration or lbug dependency was present at that G01 head.
This amendment rebased onto `79b6502`: G02 (`a176f26`) has landed
`0012_graph_claims.sql` and `facts/{types,write,read,quote}.rs`, admitting literal
`DEFAULTS_TO` only with nonempty string kinds. G31 owns the closed/entity-valued
upgrade; the historical G02 task is not credited for it. G03 is integrated as
`79b6502`: `graph/rules.rs` already provides the `Extractor` port and `resolve`,
which G10 will move/extend rather than duplicate.

The separate G25 lane writes the supervisor's G25 report and publishes
sanitized evidence in `specs/002-knowledge-graph/research.md`. That is an
execution output, not a passing result supplied by this plan.

## Constitution Check

The organization's golden rules govern through
[the repository rule map](../../docs/standards/engineering.md) and ADR-0017.
G25's Check owns the post-qualification recheck and records blockers in
`research.md`; G24 owns the final recheck before M2 acceptance.
Rows name planned holders, not claimed passes; record evidence or blockers.

| Rule IDs | Holding test, gate or review |
| --- | --- |
| C-001 | G01 checks exact requirement ownership; G25 rechecks this table against its verdict and records blockers; G24 checks final integrated evidence. |
| C-006, DEP-001 | G25's measured dependency/licence/vet review records scope, rationale, forcing library and expiry/removal condition for each named exception (ADR-0020). No gate exception is granted. |
| FND-002, FND-003, P-001, P-002, P-004, P-005 | Supervisor review of G01/G27/G28: rules-first pilot, reused S1 runner, one engine/loader and a typed-edge API for S2/S3; no speculative backend framework. |
| P-011, P-013 | G03/G06/G12/G18 test closed decoders, duplicate/unknown keys and malformed or incomplete output. |
| P-012, P-014 | G02/G31's scoped typed writes and G11/G15's per-hop/current-grant refusal tests preserve one authority and least privilege. |
| SEC-001 | G06/G32/G33/G34 test sanitized success/error channels and direct private writes; G05/G07/G08/G20/G23 use these tools, exposing only aggregates, IDs and digests. Public fixtures are synthetic. |
| SEC-002 | G03 data-only rule refusals and G19 source-instruction injection tests cannot change authority, predicates or bounds. |
| SEC-003 | G26 rejects caller-supplied engine paths; G02/G11/G15 test quote, scope and final-delivery boundaries. |
| SEC-005 | G05/G07 reject missing, expired or incomplete scope/target/expiry/evidence receipts; G24 verifies owner live-upgrade authorization. |
| SEC-008, SEC-009 | G06/G20/G23 retain every attempt; G24 refuses missing/inconclusive evidence and distinguishes queued platform work from passes. |
| ENF-001 | `maestro-conventions` path checks and commit hooks; private paths use bindings, not committed machine names. |
| ENF-002 | G25 supplies working cross-Clippy recipes before engine implementation; G22/G24 require actual native Linux/Windows/macOS builds/tests. |
| ENF-005 | Each task's Red step records a failing test or rejected missing/invalid receipt before Green. |
| ENF-006, ENF-008 | Local targeted gates and commit hooks precede lane `--no-verify` pushes under the recorded integration workflow; required CI, independent review and the supervisor's integration check still gate landing. No gate is weakened. |
| ENF-012 | G01/G07/G08/G20 freeze fixture/source/profile/card/graph digests; G25 pins engine/features and locked Cargo inputs; G20 records extractor assets before use. |
| TST-001, TST-003 | Required CI test-policy checks; G06's deterministic scoring tests and G25/G30's modules in the existing `tests/it/main.rs`, not extra native-linked binaries. |
| COV-001, COV-002 | G24 verifies required CI's ≥90% total and ≥95% changed-line coverage, plus zero missed mutants/timeouts before merge. |
| HYG-003 | Hygiene hook rejects large committed fixtures; G11 generates the 100,000-edge profile at test time. |
| ARC-001, ARC-002, ARC-005, SIZE-002 | Architecture gate and review: acyclic imports, declaration-only child `mod.rs`, correct item ownership, ≤500 counted lines; G02 never imports canonicalization into the kernel. |

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

### A0 Frozen pilot contract and approved private scopes

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

**Private pilot scope, approved at 12:53 (U2):** collection `ctm`, one official
parameter table at exactly **9.0.22**, selected by a local deterministic script. The proposed extraction window is that
complete table (header and all body rows) with its nearest heading as context;
no other document category, version or surrounding prose window is included.
The local script selects from the eligible source/table metadata in stable ID
order and freezes its own digest, chosen source, row count, byte windows and
input digests privately before text is read for extraction/review. It prints
only counts, IDs and digests. No private material was inspected by G01.

The public reference is the binding `PRIVATE/graph/receipts/pilot-inputs.json`;
its body remains unverified here. No S1 receipt applies. The 12:53 approval
settles scope, not receipt existence. G05 waits for the receipt and G32/G33's
local review/page tools; its raw neighbor JSON and dispositions stay private.
This fixture freeze proves neither private pilot success (SC-S2-001) nor M2.

**Acceptance scope, approved at 12:53 (U2):** process the whole `ctm`
collection's published generation locally. Before G07 reads it, record separate
`PRIVATE/graph/receipts/acceptance-inputs.json` with the approved generation,
document/version inventory, source digests and bounded window policy. Tune that
policy only on pilot/synthetic development failures, then apply it across the
approved scope; do not select acceptance windows from held-out labels. Scope
approval is not a receipt. All sensitive approvals record scope, target, expiry and evidence
(SEC-005), including additional-client use and the live upgrade; absent,
expired, changed or out-of-scope approval refuses the corresponding action.

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
| Platforms and behavior | Linux evidence plus working gate-preserving recipes for both local cross-Clippy targets are required before implementation. Windows/macOS native CI evidence or a supervisor-approved dated plan is also required; the actual three-OS behavior matrix still gates M2. A missing cross-Clippy recipe is blocked, never a skip. A plan is not a passed test. |
| Licence | Record Rust/native licences and notices and pass the organization licence policy; unresolved obligations block adoption. |
| Dependency cost | Minimum features, metadata and feature tree measured; at most one forced duplicate from lbug, with a named DEP-001 exception, forcing library, removal condition and vet evidence. |

On each native OS, prove G27's Cypher types/bound parameters and rollback,
the single parameterized-batch loader, bounded paths filtered at every hop,
native cancellation, a writer beside separate CLI and MCP reader processes,
second-writer refusal, and kill/reopen. Test same-file locks and the intended
mode: a writer owns an unpublished projection build; readers open immutable
published files and retain old pins. No unsafe wrapper, bypassed lock or coordinating daemon.

Put the probe in `.github/workflows/lbug-qualification.yml` in maestro-core
and its tests under `crates/maestro-knowledge/tests/it/`, not extra linked test
binaries. Keep native all-feature coverage. If the reusable toolchain needs a
change, the supervisor releases it and re-pins core; record that wait, not a
skipped gate. Publish the six-row result to the supervisor's G25 report.

The G25 checkpoint gates G04's pilot reads, G11, G21, G22 and G26–G30 (G29 is
removed), not public claim/rule or model-role groundwork. Linux, both working
cross-Clippy recipes and the approved dated platform plan permit implementation
only; actual
native and CI cost evidence still gates M2. Other missing evidence is blocked.
G25's dependency/lock/vet/probe edits integrate only with a reviewed pass;
a failed or blocked G25 lands only `research.md` and its report, not adoption.

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
G03's integrated rule tests cover literal/node boundaries and cross-document
subject grouping/collisions. G10 preserves and extends `rules::resolve` with
sourced review, version and collection semantics. FR-S2-024 binds claim predicates and entity kinds
to architecture 02 §8.2's closed lists. `ALIAS_OF` is a reviewed identity record,
not an extracted claim. G31 adds typed subject/object kinds and entity-valued
predicates after G02/G03, before G11/G19/G27. Its next-free migration preserves
valid 0012 rows/digests/history; out-of-vocabulary legacy rows refuse the whole
upgrade unchanged, with an ID-only diagnostic, never an invented mapping.
G18/G19 share those choices; G07 requires source-backed coverage for the
dependency, multi-hop, default and version question subsets.

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

The pilot attaches verified claims and reads through G27's LadybugDB projection
port, never SQLite. Each literal-valued claim is a typed fact record on its
subject entity: claim application ID, predicate, literal type/lexeme, generation
and the authoritative claim's qualifiers/support references. It is not a
literal node or an edge, and “fact” names a projection record, not proven truth.
G27 exposes scoped `entity_facts` beside entity-edge reads. G28 loads both
representations and verifies separate counts/content digests; readiness covers
both, never an edge-only projection missing the pilot's defaults.

Pilot and full graph share one
G25-qualified loader: bounded parameterized batches from a frozen kernel
snapshot into an unpublished projection build. G28 owns this loader and its
checkpoint/restart checks; G29 is removed. Bound values handle Unicode and
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

The same port's scoped `entity_facts` exposes literal-valued knowledge claims
on a subject without creating literal nodes or edges. G04 combines this read
with the subject's entity-to-entity neighbors. Neither representation grants
claim admission; both are kernel-rechecked before delivery.

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

G04 reads the subject's scoped `entity_facts` plus typed neighbors when any
entity edges exist, only from LadybugDB through G27's port after G25/G28.
A literal-only pilot therefore returns its subject facts without an invented
object entity. G11 extends this engine with bounded Cypher paths; path length
counts only entity-to-entity claim hops, never literal facts. SQLite supplies
claim authority/export and evidence checks, never graph queries.

At admission, capture the published collection generation, immutable graph
attachment, principal, scopes, version/conditions and absolute deadline. Cypher
uses bound values and fixed predicate/type choices from architecture 02 §8.2
and FR-S2-024 (no `ALIAS_OF` claim), applying all admission
filters at each hop **before** shortest-path selection, ordering and limits.
Enumerate only bounded admissible candidates; never select an unfiltered
shortest path then discard its hidden middle. Bound native expansion as well
as returned rows, and cancel the native query when its deadline expires.

Before delivery the kernel checks every returned entity, claim and support,
current permissions and source eligibility. A projection receipt or earlier
scope snapshot does not replace this check. Unknown and forbidden requests
share error shapes and disclose no counts, aliases or hidden intermediates.

R4 resolves exact identifiers/names and reviewed aliases from the question
only, using deterministic FR/EN linking without a model. Simple lookup keeps S1
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
| Graph evaluation | G06 adds inference-free `eval graph check --manifest`; the existing `eval ladder --manifest` gains graph scores/rungs, not a second ladder. G32 adds local `eval graph review --manifest`, G33 `eval graph review-page --manifest` (and `--decisions` import), and G34 `eval graph draft --manifest`. Raw artifacts remain PRIVATE; stdout/stderr are sanitized. |
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
G19 tunes the bounded offline window policy only on pilot/synthetic development
failures, then applies it to the separately approved acceptance scope; held-out
labels/failures never guide tuning. G18's schema and G19's refusal tests use
architecture 02 §8.2's closed lists and FR-S2-024. Retain rejections and reuse
the rule verifier. Source text is data; injection cannot change scopes, bounds
or predicates.

Compare deterministic rules with rules-plus-Qwen3-4B over three runs. Use one
construction scorer alongside the existing ladder's retrieval/answer scoring.
Precision review by a different local model family from the router catalog,
with owner rulings on every flag on a local review page, is pending owner
confirmation. Cover every accepted held-out claim, including rule-made claims,
and use the 95% point-estimate floor; keep semantic precision/recall separate
from byte validity, tokens, throughput and VRAM. Select the extractor on real
eligible pilot development evidence, with synthetic development checks, before
held-out construction runs; never select or tune it on acceptance results.

**I1, approved at 12:53:** preregister the development gold set and try Qwen3-8B
only if 4B misses 95% semantic precision or finds under 80% of gold claims on
pilot/synthetic development data. Retain the failed-4B evidence and revise the
estimate before a trial; pin exact assets/licences/GPU costs before use.
Never substitute the 27B router entry or enlarge the answerer.

G06 extends the existing evaluator and CLI ladder with graph labels/checks and
three separate score layers. Each frozen question records required semantic
links, conditions/version, original source anchors, acceptable alternative
complete proofs and an unanswerable reason when appropriate. Complete-proof
recall counts a question only when every required anchor of one allowed proof
is in the delivered evidence bundle under the shared context budget, after wire
packing. Route/pre-fusion/pre-delivery recall is diagnostic only; partial chains
and duplicate alternate copies earn no extra complete-proof credit.
Construction precision judges relation semantics, not just matched text.

G07 uses G34's bounded local drafting runner for the provisional 100 FR/EN
questions, twenty per type, pending owner suite-size confirmation. All 100 are held-out acceptance, with 80 answerable independent
families and twenty unanswerable ones. Equivalent versions/alternate copies
stay within one item; duplicates cannot inflate the effective sample size.
Development uses only pilot/synthetic cases, not a slice of those 100. G08's
G32 independent different-local-family runner checks every chain; the owner
rules on flags through G33's local page and digest-bound decision import. Freeze suite/corpus/
profile/family digests and the passage-only route definition before acceptance.

As a supervisor execution choice, G08 freezes the S1 default at-most-4B
answerer card by digest as it stands at freeze, including its reasoning,
sampling and output settings; G23 uses exactly that card. Thinking off is S1's
current default, not an owner ban on a separately measured thinking candidate.
Do not change the frozen answerer to obtain graph gain.

Before any G20 extraction run, preregister the graph selection rule: the
extractor card is already selected on development evidence; its first complete
run on the approved acceptance scope, with frozen rules/profile, supplies M2's
claim set. G20's Check attaches that set in scratch, runs G28's loader, verifies
G27's readiness and freezes collection/generation, source/profile/card/claim-set
and projection-content digests plus the authoritative scratch backup receipt.
Retain all attempts; a failed review/gate blocks rather than selecting a later
better graph. G23 restores this G20 authority and verifies/rebuilds the same
projection, never an S1-only backup with no graph claims. Its repeats reuse
that graph and never re-extract; only retrieval/answer stochastic work repeats.

Use the existing `eval ladder --manifest` runner for all three G23 rungs:
**passage-only / LadybugDB-only / pairing** on the same binary and frozen
scratch inputs, without duplicate embeddings. Freeze passage-only in the
manifest before G07/G08: S1's default Dense, Lexical, Identifier and Structured
routes with graph `none`, unchanged weights and the same reranker as pairing.
Identifier/Structured still use kernel SQLite; do not discard them. Pairing's
comparator is that same-run rung; G08 is a historical drift check. Alternate
run order, keep every attempt and apply the spec's exact 2,000-resample,
seed-0 paired percentile rule: 50th ordered delta strictly positive and observed
gain at least 0.05. Four wins and no losses among 80 prove the five-point
boundary in G06's synthetic check; three wins fail. No pooling three repeats as
240 independent questions.

Every run must separately pass relation precision, exactness, ≥16/20 correct
refusals, and non-decreasing `ctm-graph` retrieval/supported-answer point
estimates versus its same-run passage-only rung. Warm private-graph p95 uses
`retrieval.route.graph` and each graph tool's server time, plus separate
end-to-end search/ask limits; report cold and unavailable cohorts apart. Retain
per-type/stage results. On unchanged `ctm-retrieval`, pairing-minus-same-run-
passage-only Recall@10 and MRR@10 point deltas must each be ≥0 in every run;
a negative delta blocks M2 and disables losing fusion. Report the same seeded
paired 95% interval as diagnostic only, never as a non-regression gate.
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
| `0012_graph_claims.sql` (landed) | G02: literal `DEFAULTS_TO` claims, supports and frozen membership; never renumber it. |
| `NNNN_graph_claim_vocabulary.sql` | G31: closed subject/object kinds and entity-valued predicates, preserving valid 0012 rows/digests and refusing invalid legacy rows unchanged. |
| `NNNN_graph_builds.sql` | G09: build/checkpoint and once-only attachment guards. |
| `NNNN_graph_resolution.sql` | G10: sourced resolution/review and supersession. |
| `NNNN_extractor_role.sql` | G17: all three role constraints and preserved registry guards. |
| `NNNN_graph_projection.sql` | G27: projection receipts/readiness. |

Register each in `src/store/migration.rs`; after assigning its number and
rebasing, rerun upgrade/rollback and compatibility checks against the integrated
head. Never renumber an already integrated migration or rewrite S1 history.

## Validation

### Scratch isolation before every private run

G05, G07, G08, G20 and G23 use new isolated scratch kernels; they never open
the owner's live kernel with an S2 binary. G05/G07/G08/G20 restore a **named,
digest-checked S1 backup**. G23 restores G20's digest-frozen authoritative
scratch backup containing the M2 claim set, then verifies or rebuilds its
projection without re-extraction. G06/G07 test that `eval graph check` cannot
open the default kernel, including missing or overlapping scratch bindings.
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

### Private executors and inference locality

For G05/G07/G08/G20/G23 the lane runs Maestro commands in that scratch
environment; visible output is limited to aggregate metrics, IDs and digests.
Private sources, prompts, labels, quotes and review pages never enter a hosted
model's context, including hosted review. G06 owns safe summary/error emission
for graph check and the private ladder path; G32/G33/G34 reuse its contract:
standard output and standard error contain only aggregates, fixed codes, item
IDs and digests, never raw parser/model/router errors or private names.
Prerequisite import/build/project/backup commands redirect both streams to
private receipt files before invocation; lanes inspect only sanitized summaries
and exit status, never those raw files. Extraction/drafting uses local models
through the router. The independent precision/chain reviewer is a different
local model family from its catalog, with a pinned card; this also covers
rule-made claims and every pilot claim. The owner disposes of flagged items
on a local review page. Private review records bind claim/chain IDs, reviewer
card digest, disposition, reason and the owner's flag ruling. Reviewer protocol
confirmation and the scope/target/expiry/evidence receipt checks precede use.

G32's `eval graph review --manifest` has a pilot-capture mode: read entity IDs
from the frozen private receipt internally, run G04's projection reads and
write full neighbor JSON directly to `PRIVATE/graph/receipts/pilot-results.json`.
The lane checks only aggregate counts and digests. The same runner reviews
claim/chain inputs locally and writes every disposition/failed attempt to
private JSONL through the existing gateway, with `Room::Free` and no hard-coded
reviewer. For model-made claims/chains, reviewer family differs from extractor/
drafter; rule-made claims use the separately pinned local reviewer.

G33 renders those receipts as a self-contained static local page, following
T027's owner-disposition pattern; it imports the owner's exported decision JSON
only when item/input digests match. It has no server, external assets, automatic
approval or new UI framework. G34's `eval graph draft --manifest` calls the
existing local gateway on approved source windows and writes unreviewed drafts
directly to PRIVATE. G06 then checks anchors without inference; G08 cannot
freeze until G32 review and G33 owner flags are complete. Synthetic fake-model
and malicious-text tests precede use of each tool. These tasks create the
private workflow tools; no lane fills the gap with an ad-hoc text-dumping script.

### Acceptance sequence

1. **Construction and qualification in parallel:** public claims/rules and
   model-role work use integrated S1; all native graph reads, including
   the pilot, wait for G25's reviewed adoption bar, not M1 release.
2. **Pilot:** synthetic import → verified claims → G27/G28 projection → scoped
   LadybugDB neighbors, then the approved private scratch subset with every
   claim independently checked. G05 is a private-acceptance checkpoint, not
   a prerequisite for G10's public identity implementation.
3. **Proof safety:** hidden-hop, revocation, version/condition, generation,
   fan-out, cancel, rechunking and whole-proof budget/wire checks pass.
4. **M2 quality:** G08 drift receipt, construction score and the same-run
   three-rung comparison meet all gate rules in all three runs. G23 need not
   wait for G22's recovery drill; both still gate release.
5. **Operations/release:** G30 equality, restore, real three-OS CI, Pi/Claude
   Code smoke, coverage and zero-miss/zero-timeout mutations. G24 finalizes the
   ADR against G25 and maps integrated evidence; a platform plan cannot pass M2.

## Delivery and estimate

The [33 tasks](tasks.md) keep stable IDs, with **G25 first**, G29 removed and
G31–G34 added. Each is at most four hours including targeted gates. Total:
**124 lane-hours**, up from 108; the four new tasks add 4 h each. The pilot plus
its prerequisites is **61 lane-hours**, up from 41, with a **42-hour** dependency
path (formerly 30): it now includes G06 and all four new tasks. G09/G31 precede
G10 (migration/resolver edits); G34 precedes G32 (shared manifest). These two
extra ownership dependencies add no critical-path time. G06 follows G03, not
G04. G06/G09/G31 can
run beside qualified packaging once their inputs exist; G04 starts after
G25/G28/G03 and G05 waits for G32/G33's actual review tools.

Six lanes at eight hours/day give a **3.0–3.2-day** aggregate-work quotient with
the 20–28-hour native CI/review reserve (**144–152 total**), not a delivery floor.
The dependency-only critical path is **59 hours**, up from 52, or **7.375
eight-hour working days** before external waits. The overall planning estimate
remains **9–12 working days**; the pilot alone needs at least **5.25 working
days**, plus approval/qualification waits. Critical path:
G01 → G02 → G03 → G09 → G10 → G27 → G28 → G04 → G32 → G33 → G05 → G07
→ G08 → G20 → G23 → G24. G22 can run beside private acceptance; both must finish
before G24, which waits for every task. The added review/page prerequisites,
not G06, now control the private acceptance path.

Split an overrun into independently testable tasks before dispatch, without
waiving acceptance. At most six lanes; GPU and projection writers are serialized
where their resources conflict. The supervisor owns dispatch/review/integration;
lanes never start subagents. `[P]` marks tasks eligible for another lane once
inputs exist, not permission to race file edits. G09/G31 precede G10 and G34
precedes G32 to serialize their shared files.
Other migration/CLI registration hunks are narrow supervisor-rebased exceptions;
all other shared-file collisions require serialized ownership.

## Complexity Tracking

None approved. The pilot and full graph use the same LadybugDB projection port
and loader; no SQLite graph-read path or fallback exists. New files appear with
working behavior; existing S1 helpers and contracts are extended, not cloned.

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
