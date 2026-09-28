# Feature Specification: Knowledge graph

**Feature Branch**: `docs/s2-spec`; implementation integrates on
`feat/s2-integration`.

**Created**: 2026-09-28

**Status**: Owner-approved direction, revised after supervisor review;
implementation and qualification pending. The 100-question suite and precision
review protocol below are **pending owner confirmation**.

**Input**: the S2 draft plan, including the owner's approval of every open
recommendation at 01:56 on 2026-09-28. Milestone M2: **Relationships answered**.

Architecture: [02](../../docs/architecture/02-retrieval-and-knowledge-graph.md),
[05](../../docs/architecture/05-platform-and-operations.md),
[06](../../docs/architecture/06-roadmap.md),
[08](../../docs/architecture/08-traceability.md).
[ADR-0021](../../docs/adr/0021-embedded-ladybug-graph-projection.md) amends the
Neo4j-first choice in ADR-0004. This specification governs S2 where the earlier
architecture still describes Neo4j, vector linking or global graph search; G01
updates those design references without claiming delivery.

Rules: the organization's golden rules come first, as
[the repository rule map](../../docs/standards/engineering.md) records them.
Implementation: [plan.md](plan.md). Execution: [tasks.md](tasks.md).

## Clarifications

### Owner decisions, 2026-09-27 and 2026-09-28

- **D1 — Decided:** LadybugDB embedded first, with SQLite and artifacts as
  the authority; qualify the engine before adopting it. A qualification failure
  needs a re-plan ruling before substituting Neo4j in S2. Separately, the
  approved deployment-modes work adds a later user-selected external Neo4j
  adapter. Neither choice authorizes a runtime fallback.
- **D2 — Decided:** rules first, then Qwen3-4B as the first offline extractor
  candidate. The conditional larger candidate is **Qwen3-8B**, a new download,
  only after recall failure at the same precision floor and a revised estimate.
  It is not the router's `qwen38` entry (Qwen3.8-27B). The answerer stays at or
  below 4B. A candidate approval is not selection; record exact assets,
  licences and GPU costs before use. Answerer card/reasoning settings are
  frozen in G08, not invented here as an owner restriction.
- **D3 — Decided:** preregister the quality gates in Success Criteria, rather
  than report-only measurement. Never lower them after a failed run.
- **D4 — Decided:** use the owner-approved official-source receipt, local
  inference and independent review. Vendor questions, labels, quotes, rule
  packs, prompts and reports stay private. Additional versions or egress need
  fresh permission. Approval does not invent a receipt or broaden its scope.
- **D5 — Decided:** `lbug` is approved under ADR-0020 with the fewest features
  and measured, named DEP-001 exceptions and removal conditions. Other new
  libraries still need approval. Approval is not a passing G25 verdict.

## Needs owner action

| Action | Trigger / blocked work | Recommendation |
| --- | --- | --- |
| Confirm the 100-question suite | Before G07/G08 freeze; pending owner confirmation | Twenty per type, all 100 held out, 80 answerable independent families; development uses pilot/synthetic cases only. |
| Confirm the precision reviewer | Before G20/G23 acceptance; pending owner confirmation | A model other than the extractor checks every accepted held-out claim; owner rules on flagged claims. |
| Confirm private source coverage | Before G05/G07 private reads | Confirm that the S1 owner-pinned receipt covers the pilot subset, version and windows; otherwise approve a narrower receipt. |
| Rule on flagged review changes | G08 wording/answerability; G20/G23 flagged claims | Resolve flags before freeze or acceptance; no automatic model approval. |
| Upgrade the live installation | G24, after S2 lands | Owner takes a named backup, updates `maestro-s1` and its clients, then authorizes the live-kernel migration. All earlier runs use scratch restores. |
| Approve an additional client | Only if Copilot smoke is requested | Pi and Claude Code are the current acceptance clients. Install/use Copilot only after approval; it does not block their S2 acceptance. |

## Open questions

The two provisional items are **pending owner confirmation**: the 100-question
acceptance suite and the independent-model precision review with owner rulings.
Their task gates stay blocked until confirmed; none lowers D3's thresholds.
Private receipt coverage and any flagged review decisions also need the actions
above. A future conditional Qwen3-8B trial needs a recorded failed-4B result,
exact asset/licence receipt and revised estimate before the new download.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Inspect one sourced relationship (Priority: P1) MVP

As a maintainer, I import an approved subset and inspect a parameter's default
without running a model or a graph service.

**Why this priority**: one table rule proves the complete source-to-claim path
before a larger extractor or traversal engine can hide mistakes.

**Independent Test**: import a synthetic parameter table, build `DEFAULTS_TO`
claims and run `maestro knowledge graph neighbors`. Check every returned
revision, original byte span and quote against the source. Repeat privately on
one approved subset/version; independently review every accepted claim.

**Acceptance Scenarios**:

1. **Given** an eligible parameter table, **When** its rule runs, **Then** a
   typed relation carries the source revision, block, span, quote digest,
   extraction profile and review state.
2. **Given** an invented, empty, normalized or ambiguously located quote,
   **When** a candidate is checked, **Then** it is rejected with its reason.
3. **Given** a caller without the source grant, **When** the caller guesses an
   entity ID, **Then** the result is indistinguishable from an unknown ID.
4. **Given** no graph relation for a name, **When** neighbors are requested,
   **Then** the response describes coverage, not corpus-wide absence.

The G01–G05 pilot may use indexed SQLite one-hop reads. It is not M2 and is
not a second traversal engine retained after G11.

### User Story 2 - Follow a permitted path (Priority: P1)

As a developer, I follow a dependency or version-specific relationship and can
inspect the evidence for every link, including contradictory claims.

**Why this priority**: relationships are useful only when hidden or
inapplicable intermediate nodes cannot change the answer.

**Independent Test**: in a synthetic graph with an inaccessible short path and
an accessible longer path, request a path and receive only the admissible path,
with every link proved from authorized kernel records.

**Acceptance Scenarios**:

1. **Given** a hidden or ineligible intermediate entity, claim or support,
   **When** paths are ranked, **Then** it is filtered at each hop before
   shortest-path selection and limits, not removed from the winning path later.
2. **Given** a grant revoked after admission, **When** a result is delivered,
   **Then** a fresh kernel check refuses it without revealing hidden metadata.
3. **Given** two conflicting defaults or unknown world-valid time, **When**
   queried, **Then** their conditions, versions and uncertainty remain visible;
   the latest record does not silently win.
4. **Given** a four-link path and a five-link path, **When** the maximum is
   four, **Then** only the first is eligible; cycles, expansion and time are
   bounded and ties use stable application IDs.

### User Story 3 - Ask with complete proofs (Priority: P2)

As a developer using the CLI, Pi or Claude Code, I ask a relationship question and
receive all the source evidence for its conclusion, or an honest refusal.

**Why this priority**: graph gain must survive fusion, reranking and transport,
not disappear when an intermediate passage scores poorly.

**Independent Test**: use the fake answerer on a two-link proof; force each
budget boundary and remove one support. The answer is complete and cited or
explicitly incomplete/refused, never a confident unsupported shortcut.

**Acceptance Scenarios**:

1. **Given** an FR or EN relationship question, **When** search adds R4,
   **Then** seeds come from the question, and a graph echo is not another vote.
2. **Given** an unavailable, stale, locked or rebuilding graph, **When**
   searching, **Then** R4 reports `unavailable` and passage routes continue;
   an answer requiring the missing graph proof is refused.
3. **Given** a proof that exceeds the evidence or 64 KiB wire limit, **When**
   results are packed, **Then** whole proofs are kept or dropped with known
   gaps; no dangling link or citation is delivered as complete.
4. **Given** an invented command, inferred transitivity or unresolved
   contradiction in a generated answer, **When** validated, **Then** one retry
   is allowed, followed by refusal if still invalid. Scores are uncalibrated,
   not truth probabilities.

### User Story 4 - Measure graph gain before shipping it (Priority: P1)

As a maintainer, I compare passage retrieval, graph retrieval and their pairing
on frozen questions, and ship graph fusion only when it improves evidence.

**Why this priority**: a graph can add plausible but wrong links, costs and
retrieval regressions. A working import is not relevance evidence.

**Independent Test**: run the private `ctm-graph` comparison with the same
answerer, context budget, sampling and input snapshot for every variant. Rerun
`ctm-retrieval` unchanged and report all attempts and failures.

**Acceptance Scenarios**:

1. **Given** the provisional 100 FR/EN questions, twenty each for relationships,
   dependencies, version differences, multi-hop and unanswerable cases, **When**
   frozen after owner confirmation, **Then** a different model has reviewed
   every chain and the owner has ruled on flagged wording or answerability.
2. **Given** all 100 acceptance questions are held out, **When** rules, windows
   or prompts are tuned, **Then** only pilot and synthetic development cases
   may guide changes. Each acceptance question has one independent family;
   equivalent versions and alternate proofs do not multiply the score.
3. **Given** rules and rules-plus-4B extraction, **When** compared over three
   runs, **Then** precision, recall, quote validity, tokens, throughput and
   memory are retained, including rejected candidates and failed attempts.
4. **Given** no significant proof gain or a retrieval regression, **When** M2
   is judged, **Then** losing fusion is disabled and unmet exits block M2.

### User Story 5 - Operate and rebuild without a graph server (Priority: P1)

As a maintainer, I install the embedded runtime, check it without changing data,
and rebuild disposable graph files from the kernel while old readers stay safe.

**Why this priority**: embedding is useful only if the CLI, MCP and writer can
coexist safely and recovery does not depend on a model or a graph backup.

**Independent Test**: freeze kernel state and queries, close all graph handles,
delete only disposable graph files, rebuild and compare ordered neighbors,
paths, claims, evidence and coverage. Repeat after authoritative backup/restore.

**Acceptance Scenarios**:

1. **Given** the qualified build, **When** a writer builds an unpublished
   generation while separate CLI and MCP processes read, **Then** old pinned
   readers stay stable, a second writer is refused and publication is atomic.
2. **Given** a kill, disk-full error or uncertain native commit, **When** work
   resumes, **Then** partial data stays invisible and durable batches are not
   counted twice.
3. **Given** the kernel and artifacts only, **When** graph files are rebuilt,
   **Then** ordered semantic results are identical, except timings and
   transport IDs. No extraction model, Qdrant claim source or live-file copy
   is needed.
4. **Given** a relocated install, **When** setup or doctor runs offline,
   **Then** it needs no graph daemon, port, Docker, JVM or first-use download;
   corruption, lock and permission failures name a safe next action.

### Edge Cases

- A quote crosses a UTF-8 boundary or belongs to another block/window: reject
  it, even if similar normalized text occurs elsewhere.
- Different source spellings collide after normalization: keep the ambiguity
  for review. Matching spelling, normalized name, kind and collection resolves
  reversibly to one entity across documents; similarity alone never merges.
- A frozen generation receives new extraction input: build a new generation;
  do not attach a changed claim set to an existing pin.
- A graph file disappears after the consumer cursor advances: rebuild from the
  authoritative snapshot; replaying only later events is insufficient.
- G25 lacks required evidence: fail/block. A supervisor-approved dated
  Windows/macOS CI plan may permit implementation under plan A1, but queued
  CI is never reported as a passed native test or as M2 qualification.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-S2-001**: G25 MUST precede engine adoption, not the SQLite pilot.
  Apply plan A1's six-row pass bar and native behavior/cost checks. Linux
  evidence plus a supervisor-approved dated Windows/macOS CI plan can permit
  implementation; M2 requires actual three-OS builds/tests, cross-target
  Clippy, cancellation, independent processes and recovery evidence.
- **FR-S2-002**: The kernel MUST own immutable entities, qualified claims,
  supports, aliases, mentions and review records. Claims carry typed endpoints
  and predicates, conditions/environment, version/world validity (unknown is
  allowed), record time, extractor/profile identity and review state.
- **FR-S2-003**: Every accepted claim MUST have verified nonempty evidence:
  revision, block, original half-open UTF-8 byte span and quote digest.
  Knowledge verifies block/window ownership; the kernel verifies authorized
  revision, digest and exact bytes without depending on canonicalization.
  Quote validity is not semantic truth; confidence never grants admission.
- **FR-S2-004**: Construction MUST reuse canonical block references and one
  strict, digest-bound, data-only `DEFAULTS_TO` table rule. Its object is a
  typed literal, never an invented entity. Literals stay in claim properties;
  no literal, Document or Section nodes are projected in S2. Rule packs execute
  no scripts. Rejected candidates and their reasons remain inspectable.
- **FR-S2-005**: Builds MUST be bounded, leased, resumable and idempotent. A
  generation freezes its claim set and extraction profile, attaches verified
  claims once and pins them at admission. No model or engine I/O occurs inside
  SQLite transactions; partial builds are not queryable.
- **FR-S2-006**: The same normalized name and exact source spelling, within
  one kind and collection, MUST resolve reversibly to one entity across
  documents. Distinct spellings that normalize alike, or the same name in two
  kinds, remain distinct/ambiguous for review. Keep sourced aliases, reviewed
  decisions, supersession and version/condition boundaries; never overwrite
  contradictions or unknown validity.
- **FR-S2-007**: LadybugDB MUST be one disposable in-process projection, using
  collection, generation and application IDs, never engine IDs. Its file
  ownership and reader model MUST match G25; no unsafe lock bypass or daemon.
- **FR-S2-008**: One G25-qualified parameterized-batch loader MUST fill a fresh
  unpublished build from the frozen snapshot. Close/reopen, counts, IDs,
  digests, schema and indexes MUST pass verification before kernel readiness
  exposes it. Resuming that loader is not a second incremental engine.
  Retained generations and other collections remain untouched.
- **FR-S2-009**: Cypher MUST apply scope, eligibility, generation, version and
  conditions at every hop before selection or limits. Local depth is at most
  two, path length four and evidence 50 items; expansion, execution time and
  cancellation are bounded. Shortest means shortest admissible path.
- **FR-S2-010**: The kernel MUST recheck every result's entities, claims and
  supports against the pinned generation, current grants and eligibility
  before delivery. Hidden and unknown IDs have the same observable response.
- **FR-S2-011**: R4 MUST use question-only seeds and preserve supports through
  deterministic, one-based RRF and ties. It MUST not add echo votes or regress
  graph-disabled search. A configured graph `none` is disabled with zero calls,
  not unavailable. A selected but missing/stale/locked/rebuilding graph means
  R4 `unavailable`, not SQL or Neo4j fallback.
- **FR-S2-012**: Graph evidence MUST use `maestro-evidence/2`, carried through
  `EvidenceInput`; non-graph `/1` remains compatible and rejects unknown graph
  fields. Graph contracts carry generation, claims, paths, supports and coverage.
- **FR-S2-013**: Whole proofs MUST survive ranking, token accounting and wire
  bounds or be removed with explicit gaps/refusal. Rechunking remaps source
  spans to the pinned chunks; neither overlap nor version collapse removes a
  necessary link, condition or contradiction.
- **FR-S2-014**: CLI and MCP MUST share scoped operations for neighbors, path,
  entity resolve and evidence trace, with versioned JSON, deadlines, explicit
  ambiguity/coverage and bounded complete responses. No generic SQL/Cypher tool.
- **FR-S2-015**: Graph answers MUST use the frozen at-most-4B local answerer
  card, one retry and the S1 command guards. Every conclusion needs complete
  cited proof; no inferred transitivity, invented links or absence claims.
- **FR-S2-016**: The model registry and gateway MUST distinguish the extractor
  role, qualification and selection. Constrained extraction uses a closed JSON
  schema, card-bound sampling and `Room::Free`. Output is limited to 1,024
  tokens. Refuse cards for another role and malformed JSON: duplicate keys,
  unknown fields or incomplete output.
- **FR-S2-017**: Offline model extraction MUST use bounded source windows
  chosen from pilot/synthetic development failures only, never held-out labels
  or failures. Use the same quote checks as rules, resumable budgets and
  retained rejections. Model output is a candidate, not self-authorizing evidence.
- **FR-S2-018**: Evaluation MUST freeze input/profile/suite digests and reviewed
  proof labels. All acceptance questions stay held out from development. Use
  the existing ladder runner for Qdrant-only, LadybugDB-only and pairing,
  without duplicated embeddings. Score construction, retrieval and answers
  separately; compare with same-run Qdrant-only and keep G08 as a drift check.
  Retain every attempt and rerun `ctm-retrieval`.
- **FR-S2-019**: Rebuild MUST use SQLite/artifacts only and pass ordered result
  equality after deleting disposable graph files and after backup/restore.
- **FR-S2-020**: Foreground projection and local telemetry consumers MUST reuse
  S1 durable cursors, deduplicate effects and acknowledge only durable work.
  No background daemon or OTLP exporter is introduced.
- **FR-S2-021**: Vendor data MUST remain in the approved private collection.
  Public tests and CI use synthetic material only. Existing privacy checks
  are not assumed to be a complete vendor-content scanner.
- **FR-S2-022**: Setup and read-only health MUST cover the qualified embedded
  runtime, owned directories, relocation, locks, permissions and corruption.
  Measure install/open/reopen time, binary size, RSS and disk; document offline
  rebuild and reader-safe removal.
- **FR-S2-023**: G27 MUST expose a public typed-edge projection port at
  `crates/maestro-knowledge/src/graph/projection/port.rs`, using application IDs,
  pinned generations, scopes and authoritative edge-family records only.
  Other slices use it to write/read their own typed edges. S3 catalog dependency
  edges remain distinct from knowledge claims and never become evidence-span
  claims. No raw Cypher or engine IDs cross this API; D07 later wraps it in
  `GraphStore` for backend choice.

### Key Entities

- **Entity / mention / alias**: a typed application identity and its sourced
  names; a mention locates an occurrence, not an automatic identity merge.
- **Claim / support**: a qualified assertion and the exact source locations
  that support it. A contradictory claim is a separate record.
- **Extraction profile / build**: digest-bound rules, model card and windows,
  with budgets, leases, checkpoints and acceptance/rejection receipts.
- **Graph attachment / projection receipt**: the frozen claim set attached to
  a kernel generation, and evidence that its disposable files were verified.
- **Proof group / coverage**: a complete chain and all required supports, plus
  what the bounded graph operation examined or could not establish.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-S2-001**: The pilot returns source-verifiable `DEFAULTS_TO` neighbors
  without a model/service; every private pilot claim is independently checked,
  with failures retained. Pilot success alone never closes M2.
- **SC-S2-002**: Pairing improves complete-proof recall by at least **five
  percentage points** over **same-run Qdrant-only**, with the paired 95%
  interval strictly above zero. The provisional suite has **100 held-out
  questions**, twenty per type: **80 answerable**, twenty unanswerable, one
  independent family per item. All 100 are acceptance-only; development uses
  pilot/synthetic cases. Four new complete proofs among 80, with no loss,
  give the smallest gain that meets both gates: five points. This suite size
  is **pending owner confirmation**. Report each type and stage, and retain
  G08's S1 result as a separate drift check, not the M2 gain comparator.
- **SC-S2-003**: Relation precision is at least **95%** by point estimate;
  accepted evidence spans/quotes and answer command exactness are **100%**
  valid. The independent-model review protocol below is **pending owner
  confirmation**; unresolved flagged claims cannot be silently accepted.
- **SC-S2-004**: Supported-answer rate does not decrease under the frozen
  at-most-4B answerer; correct refusal on all twenty unanswerable questions is
  at least **80%** (at least 16/20), in every run.
- **SC-S2-005**: On the reference workstation's private graph, warm p95 graph
  time is at most **500 ms**, graph search is **under 2.5 s**, and complete ask
  is **under 10 s**. Cold/loading and unavailable cohorts are reported apart.
  Synthetic 10,000/100,000-edge profiling is a separate explicit benchmark.
- **SC-S2-006**: Delete/rebuild and authoritative backup/restore produce
  identical ordered neighbors, paths, claims, evidence and coverage, excluding
  only timings and transport IDs; retained pins and other collections survive.
- **SC-S2-007**: G25 meets plan A1's pass bar and native CI actually passes on
  Linux, Windows and macOS, including separate writer/CLI/MCP processes.
  Coverage is at least **95% changed-line / 90% total**; before merge, stable
  CI mutation runs have **zero missed mutants and zero timeouts**, within the
  **30-minute shard deadline**. Local builds stay under the **8 GiB** cap with
  three Cargo jobs. A platform plan is not final M2 evidence; no gate is waived.
- **SC-S2-008**: All four graph tools work through CLI and MCP, with **Pi and
  Claude Code** smoke receipts; permission, revocation and complete-proof
  64 KiB boundary tests pass. Copilot follows only after owner approval.

### Gate decision rules

Freeze these rules before G07/G08; never change them after a failed run.
Pairing must qualify in all three repeats. The Qdrant-only and LadybugDB-only
rungs supply the comparator and diagnostic; they need not each earn graph gain.

| Gate | Population and pass decision |
| --- | --- |
| Complete-proof gain | Each of the 80 independent answerable families contributes one paired 0/1 complete-proof result. Draw 2,000 paired resamples with replacement, seed 0, using S1's generator; keep answerable/unanswerable strata separate. Sort deltas; the two-sided 95% percentile interval uses the 50th and 1,950th ordered values (one-based). Pass only when the observed delta is at least 0.05 and the 50th value is strictly positive. G06 tests 4 wins/0 losses and 3 wins/0 losses. Duplicate families are rejected, not counted as independent samples. |
| Relation precision | A model other than the extractor checks every accepted held-out claim's semantics; the owner rules on flagged claims. The point estimate must be at least 0.95. No unresolved reviews or empty denominator passes. This reviewer protocol is pending owner confirmation. |
| Retrieval and supported answers | In every run, Recall@10, MRR@10 and supported-answer point estimates for pairing must each be at least the same-run Qdrant-only value. G08 drift is reported separately; S1 tuning is not credited as graph gain. |
| Refusal and validity | At least 16 of all 20 unanswerable questions are correctly refused per run; errors/timeouts earn no refusal credit. Every accepted span, quote and command must pass exactness, not a sample. |
| Latency and repetitions | Warm runs use the frozen private graph with models loaded. Measure `retrieval.route.graph` and each graph tool's server time against 500 ms; end-to-end search/ask use their own limits. Use nearest-rank p95 over every predeclared warm attempt, retaining failures/timeouts as failed attempts. All three stochastic runs must pass every applicable gate independently; do not average away a failing run or pool repeats as extra questions. Missing or inconclusive evidence blocks acceptance. |

## Out of Scope

No Neo4j dependency/service in S2; no SQL traversal fallback at M2. Later
user-selected Neo4j is deployment-modes work behind D07's graph port, not a
forbidden future backend. No vector/fuzzy entity linking, dense-seeded
expansion, community summaries, PPR, Leiden, global graph analytics or duplicated
vector store. These require measured gain and a later plan. Backend selection
belongs to D07, not S2's shared typed-edge API.

Qdrant Server stays unchanged. The parallel Qdrant Edge evaluation is separate
and does not qualify it for S2. Catalog semantics belong to S3; S2 provides only
the shared G27 port. No S6 acquisition, S7 UI, workflow daemon, HTTP API or
OTLP exporter. Release-delta deduplication, heading cleanup, general answer
calibration, privacy-scanner work and unrelated S1 polish keep
their own scope; source headings remain pinned until their coordinated cleanup.

## Traceability

The requirement-to-task map is in [tasks.md](tasks.md#requirement-coverage).
Design dispositions in architecture 08 are not delivery evidence. G01 maps the
S2 portions and named deferrals; G24 records integrated commits, tests and
receipts for every FR-S2 and SC-S2 item before declaring M2.

## Assumptions

- The integrated S1 APIs are the starting point; S1 finishes in parallel.
  G01–G05 wait for neither M1 release nor G25. Only engine tasks need G25.
- The owner confirms whether the S1 receipt covers the subset, version and
  windows. Private runs restore a named backup into an isolated scratch kernel,
  never the live kernel that `maestro-s1` uses. No private material was read
  to write this spec.
- `lbug` version, minimum features, native packaging and safe multi-process
  mode come from G25's measured verdict, not its crate name or approval alone.
- The 4B extractor is a candidate, not a chosen model; minimum sufficient
  size remains a measurement. A conditional 8B run is outside the estimate.
- One local user is deployed; tests exercise distinct principals and current
  revocations. Source truth and exact quotation are scored separately.
