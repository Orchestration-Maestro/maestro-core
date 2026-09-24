# 04 Intelligence backend

The intelligence backend is the platform's memory, code understanding and
governed knowledge: what earlier research called "unified project knowledge and
agent continuity". It **will be built**, on the kernel that S1 delivers, so that
it joins the platform without a second store, a second pipeline or a rewrite.
This document distils the provider analysis, defines the kernel building blocks
and sequences the capabilities (S7, phases I1–I4).

## 1. Scope and stance

- **One backend, one authority.** Memory, code, documents and facts live in the
  same kernel (SQLite + artifacts) with the same scopes, journal and projections
  as the knowledge pipeline. No per-capability database.
- **Outcomes, not engines.** The twelve researched providers describe outcomes
  worth having; Maestro reimplements the outcomes natively where it needs them.
  It does not embed the providers' engines or reuse their framework code.
- **Federation first, replacement on parity.** Until a native capability matches
  a provider on an evaluation, agents keep using that provider through the
  catalog's approved MCP servers. Each replacement is a measured switch, per
  capability.
- **Three responsibilities, one authority.** *Control and catalog* (what an
  actor may execute), *knowledge* (which accessible passages support a claim)
  and *continuity* (which authorized events, decisions, artifacts and unfinished
  work survive runs and context changes) share the kernel but never share
  authority implicitly.
- **Rust wherever feasible.** A necessary Python (or other) exception records the
  missing capability, the Rust alternatives considered, its exact inputs and
  outputs, dependency, model and licence profile, platforms and cost; it runs as
  a narrow, owned worker with bounded typed requests, and it never becomes the
  GUI, the authority or an orchestrator. Transitive native dependencies are
  audited; a profile is called "entirely Rust" only when it is.
- **Local access first.** The initial clients are Pi, Codex, Claude Code and
  GitHub Copilot CLI, over local transports only; one shared runtime serves them
  all, not one engine per client. Remote access needs a separate security and
  deployment decision.

## 2. The provider analysis, distilled

The earlier research inventoried **1,432 operational surfaces** (CLI commands,
HTTP routes, MCP tools, library entries) across twelve providers. None was
implemented or verified in Maestro; semantic review was pending on all of them.
The inventory's value is the list of outcomes, not its dispositions.

| Provider | Surfaces | Frozen disposition | What it contributes |
| --- | --- | --- | --- |
| Headroom | 275 | 275 open | Context compression with recoverable originals, budgeted context assembly |
| OpenViking | 192 | 192 open | Progressive, tiered context organization (overview → detail) over resources |
| Utopia | 172 | 153 include, 18 conditional, 1 open | Governed ontology, proposals and admission, bitemporal facts, rules, hybrid cited search, scoped identity, health |
| Graphify | 140 | 45 include, 11 conditional, 2 open, 82 exclude* | Evidence-linked knowledge graphs, communities, change/PR overlap |
| Semantica | 134 | 82 include, 30 conditional | Candidate extraction (entities, relations, events), provenance, conflicts |
| CodeGraphContext (CGC) | 132 | 70 include, 53 conditional, 9 open | Code graph: calls, imports, hierarchy, impact |
| MemPalace | 125 | 82 include, 36 conditional, 6 open | Agent memory: identity, diaries, temporal facts, coordination events, artifacts |
| Archify | 113 | 106 include, 7 open | Architecture views, evidence-linked visual workbench, exports |
| Codebase-Memory | 51 | 29 include, 18 conditional, 3 open | Code search, symbols, snippets, change impact |
| Docling | 43 | 31 include, 6 conditional, 1 open | High-fidelity document conversion, structure, tables, OCR |
| CodeGraph | 33 | 23 include, 10 conditional | Code relationships and navigation |
| Context Mode | 22 | 22 open | Near-data operations: run code where data is, return only the answer |

\*Later reviews re-bound 111 historical exclusions, leaving 3 real exclusions
(no-op, demonstration and idle-launcher surfaces) and moving the rest to include
or defer. **489 surfaces, every context-management provider, were never
decided**; this document decides them at the outcome level (§7).

The research also defined product capability families that this backend adopts
as its requirements catalogue:

| Family | Outcome | Building blocks | Phase |
| --- | --- | --- | --- |
| C01 / M01 / M02 / M06 | Trusted identity and persona per agent; bounded restore of context at session start | Scopes, journal, restore bundle | I1 |
| C02 / M03 / M04 / M05 / M08 / M10 | Capture every authorized session event, parent and child; checkpoint before compaction; idempotent receipts; recover child outcomes | Journal, artifacts, capture adapters | I1 (runs already in S4) |
| C03 / M07 / U01 / U10 | One shared store with independent scopes; explicit cross-project access | Scopes | S1 |
| C04 / C15 / M11 | Exact evidence (source, snippet, window); exports; full backup and restore | Artifacts, evidence model, backup | S1 (+ I1 restore drills) |
| C05 / U06 | Exact, lexical, vector and hybrid discovery with citations | Knowledge search | S1 |
| C06 / U09 | Ingest, refresh, reconcile, coverage, jobs, cancellation | Jobs, generations, frontier | S1, S6 |
| C07 / C08 / C09 | Code and knowledge-graph navigation, change impact, metrics | Fact store, graph projection, code collection | S2, I2 |
| C10 | Documents beyond Markdown with structure and provenance | Extraction layer | S6 |
| C11 / C12 / C13 / U02–U05 | Candidate extraction, governed admission, temporal facts, rules, provenance and conflicts | Fact store, review workflow | S2 (evidence claims), I3 (governance) |
| C14 | Durable events, handoffs and artifacts between agents | Journal, engine | S4 |
| C16 / U12 / U13 | Honest health, readiness, alerts, diagnostics | Telemetry, doctor | S1 → |
| U07 / U08 | Optional structured-data profile: confirmed ontology-to-table mappings; read-only queries over approved databases, enforced by both a query gate and a read-only database role | Fact store, capability registry | I3, on demand |
| U14 / U15 | Human review and conversation workflows; decision replay and scenario overlays; execution gating (the S4 broker); SSO, high availability and replication as demand-driven team profiles | Engine, workbench | I3–I4; team profiles deferred |
| V01–V10 | Visual workbench: graph, evidence, timelines, reviews, exports | All of the above | I4 |
| V11 / V12 | Visual chat inside the same continuity (capture, checkpoint, restore); token-efficient context with exact recovery and measured fidelity | Journal, context assembler | I1, I4 |

### 2.1 Decisions already taken with the owner

These choices were made during the provider analysis and bind the design.

| Topic | Owner decision | Where it lands |
| --- | --- | --- |
| Code analysis | Keep the complete block, SCIP and LSP included; Maestro may **propose and apply** structural transformations, but only after explicit approval, through the orchestrator, in isolation, with verification; passing checks never authorize integration | I2 (§5) |
| Source synchronization | Chosen per source: one-off import, manual refresh or automatic watch, explicitly activated and bounded | [01 §1](01-knowledge-pipeline.md#1-collections-sources-and-scopes) |
| Multimedia | Keep OCR, transcription and visual interpretation, as derived records with provenance and uncertainty | [01 §3](01-knowledge-pipeline.md#3-l2-extraction-and-normalization) |
| Retrieval scope | Current project plus explicitly shared resources; wider search only on request | [02 §1](02-retrieval-and-knowledge-graph.md#1-admission-and-scope) |
| Knowledge admission | Human validation (single or batch) or explicitly approved rules; confidence is not approval; hypotheses allowed when marked as such | I3 (§6) |
| Memory originals | No automatic expiration by default; capacity limits never delete silently | I1 (§4) |
| Context assembly | Adaptive L1 and recall budget under a configured ceiling; the exact L0 identity is never truncated; an optional local model-traffic relay for explicitly configured clients | I1, §7 |
| Visual work | Source-linked views follow new versions and keep manual positions and annotations; conflicts are shown; frozen snapshots remain | I4 (§8) |
| Shutdown | Graceful draining by default, plus a real stop-all and a forced termination with an honest record | §9 |
| Provider configuration | Add, list, show and remove provider configurations in one common registry; credential references only; adding never connects; removing never deletes knowledge | [05 §3](05-platform-and-operations.md#3-model-selection) |
| Graph advice | Graph context is offered, never required before a read or search | §7 |
| Hook administration | Install, remove and verifiable status for the four initial clients, previewed, touching only owned material | [03 §1.5](03-agent-orchestration.md#15-native-projection-convenience-mode) |
| Product form | A native Rust desktop application over one persistent runtime (ADR-0016) | I4 (§8) |

## 3. The kernel building blocks

Built in S1 (B8 in S2), each as a small interface over a deep implementation in
`maestro-kernel`. Every later capability composes them; none bypasses them.

| Block | Purpose | Interface (sketch) | Invariants |
| --- | --- | --- | --- |
| **B1 Scopes** | Who may see what: `workspace → collection → source`, later `project`, `repo`, `worktree`, `agent`, `run`, `session` | `scope(path) -> Scope`, `grant(principal, scope, rights)`, `visible(principal) -> ScopeSet` | Every read filters by scope; grants are explicit and journaled; unknown scope = no access |
| **B2 Journal** | Append-only event log: runs, jobs, captures, decisions, sessions | `record(event) -> EventId`, `events(filter)`, `cursor(consumer)` / `ack` | Never updated or deleted (SQLite triggers refuse it); ordered per stream; consumer cursors survive restarts; an outbox guarantees at-least-once delivery to projections |
| **B3 Artifacts** | Immutable content-addressed bytes | `put(bytes, media) -> Digest`, `get(digest) -> Bytes`, `pin`/`gc` | SHA-256 verified on read; atomic write (temp, fsync, rename); garbage collection only of unreferenced, unpinned artifacts |
| **B4 Jobs** | Long-running work with leases | `job(spec) -> Job`, `heartbeat`, `complete(outcome)` | One writer per lease; states `queued/running/succeeded/failed/cancelled`; resumable from the journal |
| **B5 Documents** | Collections, sources, documents, revisions, occurrences | pipeline operations of [01](01-knowledge-pipeline.md) | Revisions immutable; failed revisions inspectable, never eligible |
| **B6 Projections** | Generation-stamped derived indexes (Qdrant, Neo4j, caches) | `generation(collection, profiles)`, `publish`, `retire`, `rebuild` | Always rebuildable from B3/B5/B8; readers pinned to one generation |
| **B7 Evidence** | Spans, citations and evidence bundles | `resolve(section/chunk) -> Passage`, bundle schema `maestro-evidence/1` | Text always read from the authority; every passage carries digest, span and version |
| **B8 Facts** (S2) | Entities, aliases, relations and claims with provenance and validity | `assert(claim, evidence)`, `supersede`, `query(pattern, as_of)` | No claim without verified evidence; supersession keeps history; ambiguity goes to review, never auto-merged |
| **B9 Capabilities** | Registry of tools and their effects, exposed through MCP | `register(tool, schema, effects)`, MCP adapters | Every tool declares its effects and required scopes; the Cedar schema is generated from it |
| **B10 Model gateway** | Router client for generate, embed, rerank and tokenize, bound to model cards | `embed(profile, inputs)`, `rerank(profile, q, docs)`, `tokenize(profile, text)`, `generate(profile, request)` | Every call carries its model card; no implicit model or provider switch |
| **B11 Telemetry** | Traces, metrics, health | `span!` helpers, `health() -> Report` | Diagnostic loss is visible; the journal, not telemetry, is the audit |

**Kernel storage** (SQLite, WAL, `busy_timeout`, short transactions): tables for
scopes and grants, events (with per-stream sequence), artifacts index, jobs,
the document tables of [01 §11](01-knowledge-pipeline.md#11-kernel-records-used-by-the-pipeline),
generations, facts (S2) and catalog installs (S3). Schema changes are forward
migrations, versioned and tested on copies of real stores. Paths follow XDG:
`$XDG_DATA_HOME/maestro/{kernel.sqlite3, artifacts/sha256/..}`.

## 4. Phase I1 — memory and continuity

Goal: an agent resumes with the right context, and nothing an authorized session
produced is lost.

| Capability | Design |
| --- | --- |
| Capture | Adapters send authorized events to the daemon: a Pi extension (session, turns, tool calls, compaction), Copilot hooks (`sessionStart`, `postToolUse`, `sessionEnd`), and runs (already journaled since S4). Event classes are explicit; private reasoning is excluded unless separately authorized. |
| Session tree | Sessions, parent/child links (subagents, background tasks), with exact ranges of each child's own events. |
| Checkpoint barrier | Before compaction or context replacement, the host calls `memory checkpoint`; the call returns only when the selected cut is durable. A failed checkpoint is visible and blocks "safe to discard". |
| Restore bundle | On startup, resume and after compaction, **exactly one** bundle per context generation reaches the request builder before the next model call: **L0** owner-approved identity/persona (exact text and digest, reserved first, never truncated), **L1** project story, open work, last committed cut and child outcomes, **L2** recall for the current task, **L3** wider search on request, with explicit omissions. Budget: adaptive under a configured ceiling (4,000 tokens proposed); an impossible budget is reported, not squeezed. A retry delivers the same payload and digest; a persona update rebuilds the bundle explicitly. |
| Capture scope | Every parent and foreground or background child, including short, assistant-only, failed and cancelled runs, each with its own event ranges and an attributable outcome; the parent can retrieve each child's original outcome and artifacts, not only its own summary. Per-save receipts are idempotent under retries and response loss; a conflicting payload under the same key is an error. |
| Retention | Authorized originals (conversations, tool outputs, evidence attachments) never expire automatically; purging is an explicit decision; rebuildable caches and indexes follow their own lifecycle. |
| Diaries and facts | Per-agent diaries (authored); facts (B8) with validity and supersession: a corrected fact supersedes, never overwrites. |
| Cross-project recall | Only through an explicit scope grant; revocation stops future access and prevents replay into narrower contexts. |
| Evaluation | Recall tasks (answer from past sessions with the right scope), restore quality, zero cross-scope leaks, checkpoint durability under crash. |
| Transition | MemPalace stays in service until I1 passes the recall and leak suites; then capture switches first, recall second. |

## 5. Phase I2 — code intelligence

| Capability | Design |
| --- | --- |
| Code collection | Per repository, worktree and revision: tree-sitter 0.27 parses supported languages into symbols (definitions, references, imports); chunks follow symbol boundaries; the same kernel, generations and projections as documents. |
| Code graph | `CALLS`, `IMPORTS`, `IMPLEMENTS`, `CONTAINS`, `TESTS` edges (heuristic from tree-sitter first; precise references from SCIP indexes and LSP servers where available), projected to the graph store; every edge records its resolver, and a missing edge is never proof of absence or dead code. Advanced scoped queries, declared data-flow paths and cross-repository or cross-service relations keep their provenance. |
| Structure tools | AST-aware structural search and diff, repository-shape and language metrics, hypothetical "twin" scenarios kept apart from applied changes. |
| Transformations | Maestro prepares a structural change and its diff; applying it needs explicit approval and runs through the orchestrator in an isolated worktree with verification; integration is a separate action. |
| History | gix 0.87: churn, co-change, blame windows; linked to symbols. |
| Impact | Reverse dependencies + test mapping + co-change → "what to re-test and who to ask" for a change. Static impact, test relevance, executed coverage, community overlap and churn are reported separately. Authorized runtime traces can enrich it. |
| Tools | `code_search`, `code_symbol`, `code_references`, `code_impact` over MCP. |
| Transition | codebase-memory, CGC, CodeGraph and graphify stay until I2 matches them on a navigation and impact suite built from real questions about our own repositories. |

## 6. Phase I3 — governed semantic and temporal knowledge

| Capability | Design |
| --- | --- |
| Admission | Extracted claims start as proposals; they become accepted knowledge through human validation (one by one or in batches) or through explicitly approved admission rules; every decision is journaled with its actor and the previous state. A confidence score never admits anything; an extractor cannot approve its own proposals. Mechanical observations (a file, a symbol, source metadata) are not reviewed one by one but keep their method, revision and limits. |
| Meaning | Polarity, modality and attribution are preserved: "did", "did not", "may", "must not" and "plans to" never collapse; unresolved stays unresolved. |
| Hypotheses | Maestro may propose unestablished links, marked as hypotheses with their basis and uncertainty; they are never presented as facts and need the same admission to be promoted. |
| Ontology packs | Versioned entity and relation types per domain (catalog content), with unresolved types kept explicit. |
| Bitemporal facts | World time (when it is true) and record time (when we learned it); `as_of` queries never leak later knowledge into earlier answers. |
| Rules | Deterministic, versioned derivations with explanations and recomputation on change; no opaque inference. |
| Conflicts | Detected and kept, surfaced for review; never resolved silently. |

## 7. Context management (the undecided 489 surfaces)

Headroom, OpenViking and Context Mode all address the same problem: models have
small windows and large, noisy inputs. Their outcomes become one runtime
capability, built with S4 and extended in I1:

| Outcome | Maestro design |
| --- | --- |
| Recoverable compression | Tool outputs, logs and long documents enter the context as summaries with artifact handles; the full original is one tool call away; nothing is truncated silently |
| Budgeted assembly | The context assembler of [03 §3.4](03-agent-orchestration.md#34-context-assembly) allocates the window by priority and reports what it omitted |
| Tiered context | Overview → section → full text tiers over any resource (the L0/L1/L2 of memory restore uses the same mechanism) |
| Near-data operations | Queries and transforms run where the data is (kernel, sandbox) and return only the result |
| Model-traffic relay (optional) | An explicitly configured compatible client can route its model requests through Maestro for authorized context processing and forwarding to an approved provider, with the same authority, cancellation and cost controls; it never intercepts unrelated traffic and never replaces durable capture |
| Graph advice | Relevant graph context is offered alongside reads and searches; consulting it is never mandatory |
| Measured economy | Each optimization is compared with and without it on pinned fixtures, counting follow-up retrievals, retries and final task correctness; raw byte reduction is not a token saving, and a recoverable original does not excuse a misleading summary |

## 8. Phase I4 — workbench

A visual interface for people: graph exploration, evidence and citations,
timelines and supersession history, review queues, run lineage and exports. It
is a **native Rust desktop application** (owner-confirmed, ADR-0016); egui/eframe
is the proposed toolkit, confirmed when I4 starts. The workbench calls the same
application operations as the CLI and MCP, over versioned authenticated local
IPC with reconnection; it owns no data, and closing it never cancels
runtime-owned work. Linux, macOS and Windows are first-class targets, each
qualified before it is claimed.

| Aspect | Design |
| --- | --- |
| Scope | V01–V12: versioned graph projections and typed diagram authoring, drill-down from a master view to real code, APIs, documents and decisions, grounded chat bound to the displayed snapshot, impact and change proposals, comparisons and scenarios, stories and exports, continuity of visual chat, measured token economy |
| Views | Source-linked views follow new authorized versions and keep manual positions and annotations; missing elements and conflicts are shown; frozen snapshots remain available |
| Changes | Plan and preview first, explicit approval before any code modification, execution by the orchestrator in an isolated worktree with checks, integration as a separate approved action; a changed base invalidates the approval |
| Accessibility | Keyboard operable, visible focus, screen-reader status, non-colour errors and a table alternative to every graph; target WCAG 2.2 AA |
| Outcomes, not engines | The useful outcomes of the researched visual tools are reimplemented natively; their JavaScript renderers are not embedded |

## 9. Operating model

| Aspect | Design |
| --- | --- |
| Visible lifecycle | Per source or cut: authorized → original captured → derivation queued → derived → indexed → published, each with request ID, scope, generation, times, retries, last error class and achieved durability. Admission: candidate → accepted, rejected or contested → superseded, ended or archived. A failure can leave the original committed while derivatives fail |
| Recovery | Capture store unavailable: keep the host outbox, fail the checkpoint barrier visibly. Crash or lost response: the outcome is unknown until the receipt is looked up. Model unavailable: capture originals, allow exact and lexical reads, mark semantic coverage pending. Stale index: show stale and offer the exact source read. Broken citation: the answer or handoff is not promoted. Permission change: deny new reads and invalidate scoped caches |
| Retries | Bounded, backed off, classified (transient, permanent, permission, invalid input, unknown commit); poison work is inspectable, never dropped or retried forever |
| Shutdown | Graceful by default: stop admission, let accepted work reach a safe point, record state. **Stop-all** disables admission, watches, schedules and retries, cancels running work and stops every managed worker, agent and extension; forced termination is available with an honest record of interrupted work; nothing outside Maestro is touched; a watcher never undoes a stop-all, and restart needs an authorized start or resume |
| Deployment profiles | Local private workstation first; a team-hosted service is a separate profile with its own bootstrap, TLS, backups, keys, migration roles and rollback |

## 10. Quality targets and gates

Proposed targets, validated on a declared reference profile before evaluation
(not after failing it):

| ID | Requirement | Proposed target |
| --- | --- | --- |
| N01 | Bounded, responsive continuity | Restore budget 4,000 tokens with L0 first; p95 capture acknowledgement ≤ 250 ms for ≤ 256 KiB batches; p95 restore assembly ≤ 1 s |
| N02 | Performance and cost | p95 warm exact/lexical retrieval ≤ 1 s; bounded graph query ≤ 2 s; semantic routes reported separately; timeouts return partial results with a receipt |
| N03 | Security and privacy | Zero leaks in the isolation suites; least-privilege runtime roles; untrusted text never becomes instructions |
| N04 | Durability | RPO 0 for acknowledged raw capture; backup RPO ≤ 24 h and RTO ≤ 1 h; a restore drill each release |
| N05 | Honest diagnosis | Every terminal job and cut has a machine-readable reason and next action; no empty result on error |
| N06 | Accessibility | Core journeys keyboard-operable, WCAG 2.2 AA; CLI with plain and machine-readable output |
| N07 | Portability | Each platform exercised before it is claimed; derived paths, no machine-specific paths in exports |
| N08 | Interoperability | Versioned capability, effect, scope and output contracts, identical across CLI, MCP, API and GUI |
| N09 | Supply chain | SBOM, notices and redistribution review for code, binaries, parsers, models, tokenizers and datasets |
| N10 | Maintainability | A named owner per profile and runbook; failed evaluations and regressions stay published |

**Hard gates:** zero acknowledged-capture loss, zero wrong-persona restores, zero
unauthorized disclosure, zero accepted polarity inversions, zero historical
leakage of later knowledge, and a full manifest restore. **Quality gate:** no
worse than the strongest task-appropriate baseline by more than two points in
task success and citation correctness (paired 95 % intervals; inconclusive means
a larger set). **Usefulness gate:** at least a 20 % median reduction in user time
or model tokens and round trips on the primary journey, without raising p95
latency by more than 10 %. Evaluation sets: at least 100 source-finding
questions over five real repositories, 30 revision-pinned changes, 50 documents
across required formats, 100 adversarial semantic and temporal assertions with
30 rule scenarios, and a fault matrix run ten times per variant. Baselines are
the actual providers at pinned versions (plain search and git first), never a
feature list.

## 11. Transition from existing providers

1. Register each provider in `maestro-manifests/mcp/` with an approved tool
   allowlist and the policies that govern it; agents use them today.
2. For each capability, build the eval suite first, from real questions.
3. **Inventory without mutation, snapshot and prove readability**, then map
   scopes explicitly (unknown legacy scope goes to a restricted area), keeping
   native IDs beside new ones.
4. Import idempotently in stages: raw evidence and authored records first,
   facts, relations, decisions and artifacts next, derived indexes last;
   non-equivalent provider structures stay labelled as unconverted rather than
   flattened.
5. Ship the native capability behind the same tool names where sensible; run
   both side by side on the suite (shadow reads, one write authority).
6. Switch when the native capability matches or beats the provider; keep the
   provider read-only for an agreed retention period, with a tested rollback,
   and available for one release as a fallback that a person selects
   explicitly, never automatically.

## 12. Decisions still open

| ID | Decision | Status |
| --- | --- | --- |
| D01 | First primary journey | Proposed: scoped every-agent continuity plus exact project evidence (I1), then change review (I2) |
| D02 | Construction and storage | Decided: one coherent native core on the kernel (SQLite + artifacts) with Qdrant and Neo4j projections |
| D03 | Capture consent and private events | Proposed: all authorized user, assistant, tool and ancestry events; private reasoning excluded unless separately authorized; secret and sensitive-output policy to write |
| D04 | Identity ownership and budgets | Partly decided: exact L0 never truncated, adaptive budget under a ceiling; global versus per-project identity still open |
| D05 | Scope, retention, deletion | Partly decided: no automatic expiry of originals; erasure, backup expiry and key custody still open |
| D06 | Admission governance | Decided: human validation or explicitly approved rules; who edits ontologies and rules still open |
| D07 | Initial languages, formats, clients | Partly decided: Pi, Codex, Claude Code, Copilot CLI; languages and format profiles still open |
| D08 | Workload and benchmark targets | Open: approve or replace §10 before evaluation |
| D09 | Migration order and rollback | Proposed: §11 |
| D10 | Models, network, licensing | Covered by the bake-off and provider profiles ([05](05-platform-and-operations.md)); business terms open |
| D11 | SSO, high availability, replication | Deferred, demand-driven team profiles |

## 13. Explicitly not built

- Twelve embedded engines or one wrapper per provider command.
- A second authoritative database for memory, code or facts.
- Automatic admission of model-extracted facts as truth.
- Monitoring or ranking of people.
- A second orchestrator: the backend supplies knowledge and continuity; the
  runtime of [03](03-agent-orchestration.md) owns delegation and execution.
