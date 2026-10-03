# Feature Specification: Knowledge graph

**Feature Branch**: `docs/s2-spec`; implementation integrates on
`feat/s2-integration`.

**Created**: 2026-09-28

**Status**: Owner-approved direction, revised after supervisor review;
implementation and qualification pending. The owner approved the fusion design,
**200 answerable graph families plus 20 unanswerable** and the three fork
patch categories on 2026-09-30. The precision-review protocol still needs
owner confirmation; approval is not qualification evidence.

**Input**: the S2 draft plan, including the owner's approval of every open
recommendation at 01:56 on 2026-09-28. Milestone M2: **Relationships answered**.

Architecture: [02](../../docs/architecture/02-retrieval-and-knowledge-graph.md),
[05](../../docs/architecture/05-platform-and-operations.md),
[06](../../docs/architecture/06-roadmap.md),
[08](../../docs/architecture/08-traceability.md).
[ADR-0021](../../docs/adr/0021-embedded-ladybug-graph-projection.md) amends the
Neo4j-first choice in ADR-0004. This specification governs S2 where older
architecture text still describes Neo4j, vector linking or global graph search.
The 2026-09-30 fusion amendment also governs descriptor linking and passage
transitions; ADR-0021/0002 amendments are proposed separately, not finalized here.
G01 reconciles the design references with embedded-first S2 and names the
deferred algorithms; design agreement is not qualification or delivery evidence.

Rules: the organization's golden rules come first, as
[the repository rule map](../../docs/standards/engineering.md) records them.
Implementation: [plan.md](plan.md). Execution: [tasks.md](tasks.md).

## Rules touched

The plan's ID-keyed Constitution Check assigns the holding test, gate or review;
these are obligations, not passing evidence. Security IDs also follow the
[security rule map](../../docs/standards/security.md).

| Rule IDs | S2 concern |
| --- | --- |
| C-001, C-006 | Requirement/rule ownership and scoped, expiring dependency exceptions. |
| FND-002, FND-003, P-001, P-002, P-004, P-005 | Rules-first scope, reuse and the minimal shared projection port. |
| P-011, P-012, P-013, P-014 | Closed typed boundaries, explicit unavailable states and scoped authority. |
| SEC-001, SEC-005 | Private local processing and explicit sensitive-approval receipts. |
| SEC-002, SEC-003 | Source instructions remain data; paths, scopes and quotes are checked. |
| SEC-008, SEC-009 | Retained failures, honest blocked states and evidence before acceptance. |
| ENF-001, ENF-002 | Bound paths and actual Linux/Windows/macOS evidence. |
| ENF-005, ENF-006, ENF-008 | Failing tests first, hooks and required CI gates; no weakened checks. |
| ENF-012, DEP-001 | Pinned engine/model/profile inputs and measured dependency cost. |
| TST-001, TST-003, COV-001, COV-002 | Deterministic tests, one integration binary and total/changed-line coverage. |
| HYG-003, ARC-001, ARC-002, ARC-005, SIZE-002 | No large committed fixtures; acyclic, small, correctly owned modules. |

## Clarifications

### Owner decisions, 2026-09-27 and 2026-09-28

- **D1 — Decided:** LadybugDB embedded first, with SQLite and artifacts as
  the authority; qualify the engine before adopting it. A qualification failure
  needs a re-plan ruling before substituting Neo4j in S2. Separately, the
  approved deployment-modes work adds a later user-selected external Neo4j
  adapter. Neither choice authorizes a runtime fallback.
- **D2 — Decided:** rules first, then Qwen3-4B as the first offline extractor
  candidate. The conditional larger candidate is **Qwen3-8B**, a new download,
  only under I1's approved development-data trigger below and a revised
  estimate; no held-out result triggers selection.
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

### Owner approvals, 2026-09-28 at 12:53

- **U2 — Approved:** acceptance scope is the whole published `ctm` generation,
  local processing only. The pilot is one **9.0.22** parameter table selected
  by a local deterministic script; input/window digests are frozen before its
  text is read for extraction/review. Bind separate pilot and acceptance
  receipts; no S1 receipt applies and scope approval does not invent a receipt.
- **I1 — Approved:** try Qwen3-8B only if 4B misses the precision floor (95%)
  or finds under 80% of gold claims on pilot and synthetic development data.
  Preregister the development gold set and trigger before runs. The recorded
  failed-4B result, exact assets/licences and revised estimate precede a trial;
  held-out construction or question results never select the extractor.

### Owner decisions, 2026-09-30

- **D6 — Decided:** adopt passage-first fusion: retain S1, question-only R4,
  scoped deterministic claim-first multilingual linking and a separate bounded
  passage-transition development route (FR-S2-025). Select routes on development
  only; no new model, library or graph engine is approved.
- **D7 — Decided:** freeze 200 independent answerable graph families (50 each
  for relationships, dependencies, version differences and multi-hop), plus 20
  unanswerable families. G06 records the power table before G07/G08 freeze.
  All three arms answer both the 220-question graph and unchanged 100-question
  golden suites in all three repeats: **2,880 mandatory requests**.
- **D8 — Decided:** the fork patches are approved: E01/E02 rooted filesystem
  operations, E03 external native cache and E03b source-default builds with
  automatic prebuilt downloads removed. Adopt only a freshly qualified combined
  immutable pin. This does not waive the true-cold build bar or feature gates.

### Approved manifest v4 cross-slice obligations

The approved S3 `30b702b` handoff (`specs/003-catalog/tasks.md:3969–3974`)
is part of S2's existing requirement scope, not evidence of implementation.
Typed backend settings and frozen configuration/lock inputs cross the existing
ports; knowledge does not parse catalog TOML or gain another authority.
The [manifest v4 settings and lock handoff](plan.md#manifest-v4-settings-and-lock-handoff)
records S3 D13/D14's exact names, bounds and source lines:

- Keep canonical `graph.engine`; the registered adapter maps backend
  `type = "ladybug"` from `core/backends/graphdb/config.toml`. Retired engine
  resources refuse; old `lbug` values require explicit migration, not a new
  alias invented here. `none` makes zero graph calls; an uncompiled selection
  refuses before native calls, and a missing selected graph is unavailable.
- C46 uses core backend defaults plus `settings/defaults.toml` as one lowest
  S1 registry slot, one producer per key. Validate whole files, inactive tables,
  masked invalid defaults and locked attempts. The only named tuning keys are
  `graphdb.buffer_pool_size`, `graphdb.max_db_size`, `graphdb.max_num_threads`;
  D14 fixes their bounds/defaults. Root handles, reader/writer modes and
  checkpoint-on-close remain locked; no manifest path or invented native setter.
- C47a supplies frozen admitted defaults and a complete non-resource lock,
  preserving the checked config closure in bundles. Changed config/lock cannot
  replay; E07a/E08b/G28 consume the existing typed seam, not catalog TOML.
- C48 publishes actual qualified backend declarations only after G25/E07a
  evidence and approved owners. **OA1** remains the owner-identities/protections
  input (S3 plan:1932). Unspecified migration/encoded-value details belong to
  C46 (plan:1668–1670), and handoff wire shape to C47a (tasks:2592–2597);
  these are implementation inputs, not new owner decisions or approval IDs.

This G01 reconciliation neither changes decided D3 nor replaces integrated
S1 settings, search/ask, backup, scopes or generation inputs. Static fixture
checks are not evidence that any downstream v4 consumer or release is complete.

| Existing owner | Approved addition | Required inputs |
| --- | --- | --- |
| G01 | +1 h to reconcile the manifest v4 graph contract. | C44. |
| G26 | +1 h for registry-backed graph settings in setup/health. | C46, G25, G01. |
| G27 E07a | +2 h for the frozen settings/lock handoff into native activation. | C47a, G01, combined fork, G25, released E04. |
| E08b | +1 h to carry settings/locks through reader, writer and publication. | E08a and E06, unchanged; these paths describe scope, not new edges. |
| G28 | +1 h to bind those inputs to the existing loader/rebuild. | C47a and G26/G27/G35 graph prerequisites. |
| G22 | +1 h for qualified backend declarations in the native/release drill. | C48 and existing release inputs. |

S2 MUST preserve **qualification → C48 → G22 → C49a**, never C48 ↔ G22.
C49a also requires **E11 and G28**; qualified pins come from actual execution
receipts, never invented metadata. C49a remains S3-owned, not a new S2 task.
The increment is **7 h** (4 G + 3 E), retaining **37 G tasks and 14 E slices**:
**149 h** bounded G + **43–89 h** E = **192–238 h** full-plan effort, not remaining
work or elapsed time. Task files, tests and acceptance ownership are retained.

## Needs owner action

| Action | Trigger / blocked work | Recommendation |
| --- | --- | --- |
| Confirm the precision reviewer | Before G05 pilot review and G08/G20/G23 acceptance review; pending owner confirmation | A different local model family from the router catalog checks every accepted claim/chain, including rule-made claims. Pin its card and record claim/chain ID, disposition and reason; owner resolves flags on a local review page. |
| Record the approved pilot receipt | Before G05 private reads; G05 gates private acceptance, not public engine construction | Bind `PRIVATE/graph/receipts/pilot-inputs.json` to U2's one deterministically selected 9.0.22 table, script/window/input digests and 12:53 approval evidence. No additional scope decision is pending. |
| Record the approved acceptance receipt | Before G07/G08 freeze or any acceptance-source read | Bind `PRIVATE/graph/receipts/acceptance-inputs.json` to the approved whole published `ctm` generation, document/version inventory, digests and bounded window policy, local-only. Pilot permission alone is insufficient. |
| Rule on flagged review changes | G08 wording/answerability; G20/G23 flagged claims | Resolve flags before freeze or acceptance; no automatic model approval. |
| Upgrade the live installation | G24, after S2 lands | Owner takes a named backup, updates `maestro-s1` and its clients, then authorizes the live-kernel migration. All earlier runs use scratch restores. |
| Approve an additional client | Only if Copilot smoke is requested | Pi and Claude Code are the current acceptance clients. Install/use Copilot only after approval; it does not block their S2 acceptance. |

Every sensitive approval receipt records its scope, target, expiry and approval
evidence (SEC-005), including pilot/acceptance inputs, additional-client use and
the live upgrade. Missing, expired or out-of-scope approval blocks that action.

For G05, G07, G08, G20, G23 and G38, a lane runs Maestro commands with only aggregate
metrics, IDs and digests in their visible output; standard error carries only
fixed codes, IDs and digests, never raw parser/model/router error text. No
private text enters a hosted model's context. Extraction and review use local models through the
router; the independent reviewer is a different local model family from its
catalog. G32 owns the local review/capture runner, G33 the owner's local review
page and decision import, and G34 the local drafting runner. G05's full neighbor
JSON is written directly to PRIVATE; the lane sees only counts and digests.
Only the owner reads flagged text on the local review page.

## Open questions

The independent local reviewer protocol remains **pending owner confirmation**.
The suite size is approved by D7. U2's scopes and I1's trigger are approved; their actual private
receipts still must be recorded and verified before use. No approval lowers
D3's thresholds. A conditional Qwen3-8B trial still needs the development-only
failed-4B result, exact asset/licence receipt and revised estimate.

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

The rule-only pilot reads neighbors only through qualified LadybugDB and G27's
projection port, using G28's single loader. SQLite holds claim authority and
evidence checks, never graph queries. G04 waits for G25/G28; G11 extends the
same engine with bounded paths. This pilot is not M2. G01 freezes the public
[synthetic source](../../tests/fixtures/synthetic/graph/defaults.md) and
[rule/oracle fixture](../../tests/fixtures/synthetic/graph/defaults.json).
Plan A0 specifies this single table contract and the separately approved
private scopes/receipt obligations; synthetic evidence never substitutes for
an actual private run.

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
   is allowed inside the same 20 s request deadline, followed by refusal if
   still invalid. Scores are uncalibrated, not truth probabilities.
5. **Given** a French paraphrase without an exact seed, **When** linking runs,
   **Then** only admitted descriptors compete before top-k; similarity neither
   merges identities nor proves a claim, and unresolved questions remain NIL.
6. **Given** source-linked passage hits, **When** the passage route expands,
   **Then** bounded authorized new passages may enter fusion without echo votes;
   a transition is never delivered as a documentary relationship proof.

### User Story 4 - Measure graph gain before shipping it (Priority: P1)

As a maintainer, I compare passage retrieval, graph retrieval and their pairing
on frozen questions, and ship graph fusion only when it improves evidence.

**Why this priority**: a graph can add plausible but wrong links, costs and
retrieval regressions. A working import is not relevance evidence.

**Independent Test**: run the private `ctm-graph` comparison with the same
answerer, context budget, sampling and input snapshot for every variant. Rerun
`ctm-retrieval` unchanged and report all attempts and failures.

**Acceptance Scenarios**:

1. **Given** the approved 220 FR/EN questions, fifty each for relationships,
   dependencies, version differences and multi-hop plus twenty unanswerable, **When**
   frozen after review, **Then** a different local model family
   has reviewed every chain and the owner has ruled on flagged wording or
   answerability on a local review page.
2. **Given** all 220 graph acceptance questions are held out, **When** rules, windows
   or prompts are tuned, **Then** only pilot and synthetic development cases
   may guide changes. Each acceptance question has one independent family;
   equivalent versions and alternate proofs do not multiply the score.
3. **Given** rules and rules-plus-4B extraction, **When** compared over three
   runs, **Then** precision, recall, quote validity, tokens, throughput and
   memory are retained, including rejected candidates and failed attempts.
4. **Given** no significant proof gain or a retrieval/answer/refusal regression, **When** M2
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

1. **Given** the qualified build, **When** a writer fills an unpublished
   projection build while separate CLI and MCP processes read, **Then** old pinned
   readers stay stable, a second writer is refused and publication is atomic.
2. **Given** a kill, disk-full error or uncertain native commit, **When** work
   resumes, **Then** partial data stays invisible and durable batches are not
   counted twice.
3. **Given** the kernel and artifacts only, **When** graph files are rebuilt,
   **Then** ordered graph/transition results and canonical descriptor contents,
   IDs, pointers and scope payloads are identical, except timings and transport
   IDs. Graph-file rebuild, including transitions, needs no model; only
   descriptor re-embedding may use the pinned existing embedding profile.
   Neither rebuild uses Qdrant claim authority or a live-file copy.
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

- **FR-S2-001**: G25 MUST precede engine adoption, including G04's pilot reads.
  Apply plan A1's six-row pass bar and native behavior/cost checks. Linux
  evidence plus a supervisor-approved dated Windows/macOS CI plan can permit
  implementation only with working gate-preserving cross-target Clippy recipes.
  M2 requires actual three-OS builds/tests, cross-target Clippy, cancellation,
  independent processes and recovery evidence. Requalify the combined
  E01/E02/E03/E03b pin, including source-default/no-download external builds,
  rooted native filesystem operations and true-cold versus cache-hit timings.
  Default builds remain engine-free; required feature coverage/mutation and
  M2 release packaging with `--features engine` cover the shipped engine.
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
  no literal, Document or Section nodes are projected as claim entities.
  FR-S2-025 permits passage application-ID nodes only in a separate retrieval
  transition family, never as claim endpoints or documentary proof links.
  Rule packs execute no scripts. Rejected candidates and their reasons remain inspectable.
- **FR-S2-005**: Builds MUST be bounded, leased, resumable and idempotent. A
  generation freezes its claim set and extraction profile, attaches verified
  claims once and pins them at admission. No model or engine I/O occurs inside
  SQLite transactions; partial builds are not queryable.
- **FR-S2-006**: Absent an unresolved source-backed namespace collision, the
  same normalized name and exact source spelling within one kind and collection
  MUST resolve reversibly to one entity across documents. Distinct spellings that normalize alike, or the same name in two
  kinds, remain distinct/ambiguous for review. Keep sourced aliases, reviewed
  decisions, supersession and version/condition boundaries; never overwrite
  contradictions or unknown validity. Audit same-name unrelated documentary
  namespaces within one collection before freeze: an unresolved collision
  blocks automatic resolution and is retained for review, not silently merged.
  Any identity-key change requires a separate ruling with synthetic cases.
  G35 landed before G10's reopened audit; that ordering exception does not
  satisfy the audit. G10's existing 2 h follow-up MUST recheck G35's descriptor
  linking and projection handoff against the audited identities and fix findings
  in the same follow-up, without a silent identity-key change.
- **FR-S2-007**: LadybugDB MUST be the only graph-read engine, including pilot
  neighbors and all paths, behind G27's projection port. SQLite serves claim
  authority and evidence checks only, never graph queries. The disposable
  in-process projection uses collection, generation and application IDs, never
  engine IDs. A literal-valued claim is a typed fact record on its subject,
  keyed by claim application ID with predicate, literal type/lexeme, generation
  and the claim's qualifiers/support references; `entity_facts` reads it through
  the port. It creates no literal node or edge and grants no semantic truth.
  Entity-to-entity claims alone create proof edges and count toward proof-path
  length. FR-S2-025 retrieval transitions are a distinct family with separate
  passage-hop limits, never interchangeable with claim or S3 catalog edges.
  Its file ownership and reader model MUST match G25; no unsafe lock bypass
  or daemon. Native opens/metadata/mutations MUST stay rooted in held safe
  directory capabilities or fail closed; preflight followed by path reopen is
  insufficient. No extension, COPY or alternate filesystem bypass is allowed.
- **FR-S2-008**: One G25-qualified parameterized-batch loader MUST fill a fresh
  unpublished projection build from the frozen kernel snapshot. Close/reopen,
  counts, IDs, digests, schema and indexes MUST pass verification before kernel
  readiness exposes it. Resuming that loader is not a second incremental engine.
  Retained generations and other collections remain untouched. Descriptor and
  passage-transition projections MUST have digest-bound builders and verified
  generation/scope readiness; neither is an alternate claim authority or a
  second graph loader.
- **FR-S2-009**: Cypher MUST apply scope, eligibility, generation, version and
  conditions at every hop before selection or limits. Local depth is at most
  two, path length four and evidence 50 items; expansion, execution time and
  cancellation are bounded. Shortest means shortest admissible path. Descriptor
  lookup MUST pre-filter authorized collection, pinned generation, requested
  version and eligibility inside Qdrant before top-k/seed caps; an unrepresentable
  admitted scope fails closed. Passage transitions filter each hop before caps.
- **FR-S2-010**: The kernel MUST recheck every result's entities, claims and
  supports against the pinned generation, current grants and eligibility
  before delivery. Descriptor candidates MUST also pass current-authority
  checks before traversal; pre-filtering and final rechecks are both mandatory.
  Transition passages and verified-mention identities receive the same checks.
  Hidden and unknown IDs have the same observable response.
- **FR-S2-011**: R4 MUST use question-only seeds and preserve supports through
  deterministic, one-based RRF and ties. It MUST not add echo votes or regress
  graph-disabled search. A configured graph `none` is disabled with zero calls,
  not unavailable. A selected but missing/stale/locked/rebuilding graph means
  R4 `unavailable`, not SQL or Neo4j fallback. Stale means its attachment's
  generation is not the published generation at request admission. R4 MUST NOT
  seed from dense/BM25 passage hits. Try exact names/identifiers/reviewed aliases
  first; whenever no exact seed exists, the development-selected linker matches
  the question directly to deterministic verified-span claim/triple descriptors
  ahead of entity-name-only descriptors, using the existing multilingual model.
  No LLM descriptor prose or online generative translation/NER. Keep separate
  pointers for disjoint spans; concatenated index text is not a contiguous quote.
  Similarity changes retrieval priority only, not identity, alias, claim or truth.
  Freeze caps and thresholds separately for EN→EN, FR→FR, FR→EN and EN→FR on
  development data with NIL cases in every direction; no per-product dictionary.
  Start development with at most five seeds, retaining competing senses and
  testing endpoint competition. Measure golden trigger frequency and lookup cost;
  leave semantic linking off if development evidence cannot justify it.
- **FR-S2-012**: Graph evidence MUST use `maestro-evidence/2`, carried through
  `EvidenceInput`; non-graph `/1` remains compatible and rejects unknown graph
  fields. Graph contracts carry generation, claims, paths, supports and coverage.
- **FR-S2-013**: Whole proofs MUST survive ranking, token accounting and wire
  bounds or be removed with explicit gaps/refusal. Rechunking remaps source
  spans to the pinned chunks; neither overlap nor version collapse removes a
  necessary link, condition or contradiction. Report candidate/pre-fusion/
  post-packing proof attrition and exact citation/support coverage separately;
  route recall never substitutes for delivered complete-proof scoring.
- **FR-S2-014**: CLI and MCP MUST share scoped operations for neighbors, path,
  entity resolve and evidence trace, with versioned JSON, deadlines, explicit
  ambiguity/coverage and bounded complete responses. No generic SQL/Cypher tool.
- **FR-S2-015**: Graph answers MUST use an at-most-4B local answerer, at most
  one retry and the S1 command guards. Acceptance runs pin G08's frozen card.
  Every conclusion needs complete cited proof; no inferred transitivity,
  invented links or absence claims. The one absolute **20 s end-to-end request
  deadline** includes admission/queue waits, retrieval, model loading, generation,
  validation, final response delivery and the retry; stages never reset it.
  Deadline exhaustion cancels remaining work and earns no support/refusal credit.
  Report unsupported conclusions separately from citation presence.
- **FR-S2-016**: The model registry and gateway MUST distinguish the extractor
  role, qualification and selection. Constrained extraction uses a closed JSON
  schema, card-bound sampling and `Room::Free`. Output is limited to 1,024
  tokens. Refuse cards for another role and malformed JSON: duplicate keys,
  unknown fields or incomplete output.
- **FR-S2-017**: Offline model extraction MUST use a bounded window policy
  tuned only on pilot/synthetic development failures, never held-out labels or
  failures, then apply it to the separately approved acceptance scope. Use the
  same quote checks as rules, resumable budgets and retained rejections. Model
  output is a candidate, not self-authorizing evidence.
- **FR-S2-018**: Evaluation MUST freeze input/profile/suite digests and reviewed
  proof labels. All acceptance questions stay held out from development. Use
  the existing ladder runner for passage-only, LadybugDB-only and pairing,
  without duplicated embeddings. Passage-only keeps S1's default Dense,
  Lexical, Identifier and Structured routes with graph `none`, unchanged
  weights and the same reranker. Freeze that definition before G07/G08.
  On development data compare A, A+R4, A+passage transitions and their combination
  immediately, without waiting for R4 failure. Freeze whether fused acceptance
  arm C includes transitions and semantic linking; graph-only B uses R4 proofs
  and reports any descriptor dependency, never hidden passage retrieval.
  Every acceptance arm answers both suites: (220 graph + 100 golden) × 3 arms ×
  3 repeats = **2,880 requests**, not a retrieval-only substitute. The golden
  suite remains 84 answerable entries and 16 unanswerable; translated entries
  are not assumed to be independent families.
  Score construction, retrieval and answers separately; compare with same-run
  passage-only and keep G08 as a drift check. Retain every attempt; rerun
  `ctm-retrieval` with its non-regression gate in every run. Local drafting,
  review and owner-page/decision commands MUST have owning tasks and private
  outputs; `eval graph check` remains inference-free.
- **FR-S2-019**: Rebuild MUST use SQLite/artifacts only and pass ordered result
  equality after deleting disposable graph/transition files and the descriptor
  collection and after authoritative backup/restore. Graph-file rebuild,
  including transitions, MUST need no model. Only descriptor re-embedding may
  use the pinned existing embedding profile. Reconstruct descriptors
  from verified authority/artifacts with the frozen builder/embedding profile;
  compare canonical text, IDs, source pointers and scope payloads, not approximate
  ANN ordering or storage bytes. No projection backup or LLM-written text is
  required to reconstruct authority.
- **FR-S2-020**: Foreground projection and local telemetry consumers MUST reuse
  S1 durable cursors, deduplicate effects and acknowledge only durable work.
  No background daemon or OTLP exporter is introduced.
- **FR-S2-021**: Vendor data MUST remain in the approved private collection.
  Public tests and CI use synthetic material only. Private commands MUST write
  raw results/reviews directly under the approved private root; both successful
  output and error paths expose only aggregates, fixed codes, IDs and digests.
  Existing privacy checks are not a complete vendor-content scanner.
- **FR-S2-022**: Setup and read-only health MUST cover the qualified embedded
  runtime, owned directories, relocation, locks, permissions and corruption.
  Measure install/open/reopen time, binary size, RSS and disk; document offline
  rebuild and reader-safe removal.
- **FR-S2-023**: G27 MUST expose a public typed-edge projection port at
  `crates/maestro-knowledge/src/graph/projection/port.rs`, using application IDs,
  pinned generations, scopes and authoritative edge-family records. FR-S2-025
  additionally permits a separate deterministic source-derived retrieval family;
  those records are never documentary claims or claim-path edges.
  Alongside entity-to-entity edges it MUST expose scoped `entity_facts` for
  literal-valued subject claim records; G04 neighbors returns these records and
  typed neighbors when present. Other slices use the port for their own typed
  edges. S3 catalog dependency
  edges remain distinct from knowledge claims and never become evidence-span
  claims. No raw Cypher or engine IDs cross this API; deployment-modes D07 later
  wraps it in `GraphStore` for backend choice.
- **FR-S2-024**: S2 MUST use the closed entity-kind and relation-type lists in
  [architecture 02 §8.2](../../docs/architecture/02-retrieval-and-knowledge-graph.md#82-graph-model).
  All listed entity kinds are eligible; extraction accepts the listed claim
  predicates except `ALIAS_OF`, which remains a reviewed identity record.
  G31 upgrades G02's literal-only authority to this closed vocabulary; G11,
  G19 and G27 wait for it. G18's schema, G19's refusals and plan A4's queries
  use the same list;
  no candidate extends it. Scope is documentation, not customer schedules:
  documented job types/events must fit the closed documentary vocabulary without
  inventing an instance-level `Job` kind or coercing event satisfaction into
  `DEPENDS_ON`. Missing vocabulary needs a narrow source-backed ADR before
  freeze. C27a catalog edges remain separate. M2's source-backed graph must
  cover these subsets for the corresponding answerable question classes,
  without inferred edges:

  | Question class | Required supported vocabulary |
  | --- | --- |
  | Dependencies | `DEPENDS_ON`, `REQUIRES` between listed entity kinds. |
  | Multi-hop | Complete entity-to-entity chains using `DEPENDS_ON`, `REQUIRES` or `PART_OF`; literal defaults never supply a hop. |
  | Relationships, including defaults | Listed entity relations; `DEFAULTS_TO` from `Parameter` to a typed literal supplies defaults. |
  | Version differences | Version-qualified claims, including `INTRODUCED_IN`, `DEPRECATED_IN`, `REPLACES` or `APPLIES_TO`; `Version` is an eligible entity kind. |

  G07 checks this coverage against approved source anchors before suite freeze;
  absent source support blocks the affected suite coverage, never licenses
  invented claims. Unanswerable items require no fabricated graph link.
- **FR-S2-025**: A separately switchable bounded passage-transition route MUST
  derive links deterministically from verified source structure, resolved source
  cross-references and shared verified entity mentions. Retain derivation/source
  provenance and stable passage application IDs in a rebuildable projection;
  generate shared-entity neighbors on demand, not a quadratic clique. Seed from
  admitted dense/lexical passage hits, filter authorization/eligibility/version
  before every cap, and recheck before delivery. Freeze seed/hop/per-entity
  fan-out/visited/time limits on development; high-degree hubs cannot exhaust
  all work. Cross-version expansion requires an explicit comparison request.
  Fuse newly discovered passages as a dependent passage route with parent/origin
  attribution and stable source deduplication; unchanged seed echoes earn no
  additional vote. Answer only from original passages under S1's support guards,
  never from transition edges. Relation-semantic review is not required for a
  retrieval hint, but source/identity verification is. Persisted graph reads
  use LadybugDB only; no SQLite traversal or extra engine. Run the development
  ablation before held-out freeze, measuring bridge recall, delivered proofs,
  lookup support/refusals, latency and indexing/review cost under equal budgets.

### Key Entities

- **Entity / mention / alias**: a typed application identity and its sourced
  names; a mention locates an occurrence, not an automatic identity merge.
- **Claim / support**: a qualified assertion and the exact source locations
  that support it. A contradictory claim is a separate record.
- **Extraction profile / build**: digest-bound rules, model card and windows,
  with budgets, leases, checkpoints and acceptance/rejection receipts.
- **Graph attachment / projection receipt**: the frozen claim set attached to
  a kernel generation, and evidence that its disposable files were verified.
- **Descriptor / passage transition**: deterministic disposable retrieval data
  keyed by source/application IDs and scoped generation/profile digests; neither
  descriptor similarity nor passage reachability is claim authority.
- **Proof group / coverage**: a complete chain and all required supports, plus
  what the bounded graph operation examined or could not establish.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-S2-001**: The pilot returns source-verifiable `DEFAULTS_TO` neighbors
  without a model/service; every private pilot claim is independently checked,
  with failures retained. Pilot success alone never closes M2.
- **SC-S2-002**: Pairing improves complete-proof recall by at least **five
  percentage points** over **same-run passage-only**, with the paired 95%
  interval strictly above zero. The approved suite has **220 held-out questions**:
  **200 answerable** (50 per answerable class), twenty unanswerable, one independent
  family per item. All 220 are acceptance-only; development uses pilot/synthetic
  cases. Ten new complete proofs among 200 with no loss meet the five-point
  boundary; nine do not. G06 records the power table before G07/G08 freeze;
  repeated runs do not enlarge the family denominator. Report each type and
  stage, and retain
  G08's S1 result as a separate drift check, not the M2 gain comparator.
- **SC-S2-003**: Relation precision is at least **95%** by point estimate;
  accepted evidence spans/quotes and answer command exactness are **100%**
  valid. The independent-model review protocol below is **pending owner
  confirmation**; unresolved flagged claims cannot be silently accepted.
- **SC-S2-004**: Supported-answer rate does not decrease under the frozen
  at-most-4B answerer versus same-run passage-only on the 200 answerable graph
  families; correct refusal on all twenty graph unanswerables is at least **80%**
  (16/20). On unchanged golden v2.2, supported-answer rate on **84 answerable
  entries** and correct-refusal rate on **all 16 unanswerable entries** MUST each
  equal or exceed same-run passage-only in every repeat. Errors/timeouts earn no
  support/refusal credit. All three arms answer both suites in all three repeats.
- **SC-S2-005**: On the reference workstation's private graph, warm p95 graph
  time is at most **500 ms**, p95 graph search is **under 2.5 s**, and p95 complete
  ask is **under 10 s**. Cold/loading and unavailable cohorts are reported apart.
  Synthetic 10,000/100,000-edge profiling is a separate explicit benchmark.
  Every request also has the single **20 s hard end-to-end deadline** in
  FR-S2-015; per-attempt generation timeouts cannot qualify this boundary.
- **SC-S2-006**: Delete/rebuild and authoritative backup/restore produce
  identical ordered neighbors, paths, claims, evidence and coverage, excluding
  only timings and transport IDs; retained pins and other collections survive.
  Include deterministic passage-transition results and descriptor text, IDs,
  separate source pointers, scope payloads and builder/embedding/profile digests.
  Use deterministic lookup fixtures, not byte-identical approximate ANN ordering.
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
Pairing must qualify in all three repeats. Passage-only (S1's default routes,
weights and reranker with graph `none`) and LadybugDB-only supply the comparator
and diagnostic; they need not each earn graph gain. Before any extraction run,
preregister the M2 graph selection rule: use the development-selected extractor
card's first complete run on the approved acceptance scope, with its frozen
rules/profile. G20 builds, verifies and freezes that graph by digest. G23 reuses
it for every rung/repeat; stochastic repeats never re-extract or choose a better
graph from held-out results.

| Gate | Population and pass decision |
| --- | --- |
| Complete-proof gain | Each of the 200 independent answerable families contributes one paired 0/1 result: one allowed proof counts only when every required anchor is in the delivered evidence bundle under the shared context budget, after the wire limit. Route, pre-fusion and pre-delivery recall are diagnostic only. Draw 2,000 paired resamples with replacement, seed 0, using S1's generator. Sort deltas; the two-sided 95% percentile interval uses the 50th and 1,950th ordered values (one-based). Pass only when the observed delta is at least 0.05 and the 50th value is strictly positive. G06 tests 10 wins/0 losses and 9 wins/0 losses at N=200, and retains the historical N=80 power examples as diagnostics only. Duplicate families are rejected, not counted as independent samples. |
| Relation precision | An independent different local model family from the router catalog checks every accepted held-out claim's semantics, including rule-made claims; the owner rules on flagged claims on a local review page. Pin the reviewer card independently of the extractor. The point estimate must be at least 0.95. No unresolved reviews or empty denominator passes. This reviewer protocol is pending owner confirmation. |
| Retrieval and supported answers | In every run, `ctm-graph` Recall@10, MRR@10 and supported-answer point estimates for pairing must each be at least the same-run passage-only value. G08 drift is reported separately; S1 tuning is not credited as graph gain. |
| `ctm-retrieval` non-regression | In every run, pairing-minus-same-run-passage-only point deltas for both Recall@10 and MRR@10 must be ≥0 on the unchanged suite. Golden supported-answer rate on 84 answerable entries and correct refusal on all 16 unanswerables must also each be ≥ same-run A. Errors/timeouts earn zero credit. Any regression blocks M2 and disables losing fusion (US4 AS4). Report the seeded paired 95% interval using the same 2,000-resample/seed-0 method; the interval is diagnostic only, not a gate. |
| Refusal and validity | At least 16 of all 20 unanswerable questions are correctly refused per run; errors/timeouts earn no refusal credit. Every accepted span, quote and command must pass exactness, not a sample. |
| Latency and repetitions | Warm runs use the frozen private graph with models loaded. Measure `retrieval.route.graph` and each graph tool's server time against 500 ms; end-to-end search/ask use their own limits. Use nearest-rank p95 over every predeclared warm attempt, retaining failures/timeouts as failed attempts. One 20 s hard request deadline includes queue/loading/retry time and cancels remaining work. All three stochastic runs must pass every applicable gate independently; do not average away a failing run or pool repeats as extra questions. Missing or inconclusive evidence blocks acceptance. |

## Out of Scope

No Neo4j dependency/service in S2; no SQL traversal fallback at M2. Later
user-selected Neo4j belongs behind deployment-modes D07's graph port, not a
forbidden future backend. Scoped deterministic descriptor candidate linking
(FR-S2-011) and dense/lexical-seeded passage transitions (FR-S2-025) are in scope;
identity merging by vector similarity and passage-hit seeding of R4 are not.
No graph-engine vector/FTS extension, duplicated vector store, community
summaries, PPR, Leiden or global graph analytics. Those require measured gain
and a later plan. Backend selection
belongs to deployment-modes D07, not S2's shared typed-edge API.

Qdrant Server deployment stays unchanged; descriptors extend its projection
contents only. The parallel Qdrant Edge evaluation is separate
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
  No S2 task waits for M1 release. Public claims/rules/model-role work can
  start alongside G25, but G04 pilot neighbors need G25/G27/G28 first; no
  temporary SQLite graph queries or postponed LadybugDB delivery.
- No S1 receipt applies to the pilot. U2 approves one deterministically
  selected 9.0.22 table and the whole published acceptance generation locally;
  G05/G07/G08 still need verified separate `pilot-inputs.json` and
  `acceptance-inputs.json` under `PRIVATE/graph/receipts/`, with frozen digests,
  scope/target/expiry and approval evidence.
  Private runs restore a named backup into an isolated scratch kernel, never
  the live kernel that `maestro-s1` uses. G01 reads no private material.
- `lbug` version, minimum features, native packaging and safe multi-process
  mode come from G25's measured verdict, not its crate name or approval alone.
- The 4B extractor is a candidate, not a chosen model; minimum sufficient
  size remains a measurement. A conditional 8B run is outside the estimate.
- One local user is deployed; tests exercise distinct principals and current
  revocations. Source truth and exact quotation are scored separately.
