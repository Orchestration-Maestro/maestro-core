# Feature Specification: Catalog

**Feature Branch**: `docs/s3-spec`, from `feat/s3-integration`

**Created**: 2026-09-28

**Status**: Draft specification of the owner-approved S3 direction; not delivery
or qualification evidence.

**Input**: Give the owner `maestro init` and a small Copilot/Pi catalog on WSL.
First prove cited knowledge answers and safe removal. M3, "Catalog installable",
also requires trusted releases, freshness, revocation, settings and measured
routing. The first checkpoint is not M3.

Architecture: [03](../../docs/architecture/03-agent-orchestration.md),
[06 S3](../../docs/architecture/06-roadmap.md#s3-catalog--m3),
[08](../../docs/architecture/08-traceability.md),
[ADR-0015](../../docs/adr/0015-bundle-freshness-and-revocation.md).
Rules: [engineering](../../docs/standards/engineering.md) and the organization's
golden rules come first. Implementation: [plan.md](plan.md), [tasks.md](tasks.md).

## Clarifications

### Owner decision, 2026-09-28 01:56

The owner accepted every open decision in the S3 plan draft at its
recommendation. These are decisions, not questions to reopen during execution.

- **D1 Scope — decided:** deliver the small content and owner-loop checkpoints
  first, explicitly as authoring convenience. They do not remove any M3 exit
  criterion. A narrower graph language needs an explicit, approved disposition
  in architecture 08; unsupported constructs are rejected, never ignored.
- **D2 Trust — decided:** use a pinned GitHub CLI (`gh`) verifier, not custom
  cryptography. Catalog and runtime have separate publisher identities. Refresh
  trust records at most five minutes apart during use; verified offline records
  expire within 24 hours. Prove the roots and rotation before adoption.
- **D3 Privacy — decided:** synthetic data is the default. Private use needs the
  exact client, provider, account and data-scope approval of the S1 T038 proof.
  A local stdio server does not make remote inference local.
- **D4 Libraries and hosts — decided:** "installed parsers plus approval for
  new libraries". Reuse installed parsers/adapters; adopt `tar`, a measured JSON
  Schema 2020-12 validator and `cedar-policy` 4.13 (ADR-0007). C09 measures minimum
  versions/features, duplicates and licences; the supervisor verifies ADR-0020
  evidence and named DEP-001 exceptions. This is not a second library-approval
  gate or permission for host downloads, account access or organization changes.
- **D5 Quality — decided:** a hybrid win is required only to enable hybrid
  routing. Keep top-3 accuracy at least 90 %, report top-1 and correct no-match
  separately, and include synthetic distractors so two workflows cannot make
  top-3 trivial. Record the change explicitly in architecture 06; do not
  silently reinterpret its older unconditional "beats the baseline" wording.

The exact host pin, measured dependency versions and publisher bindings are
execution inputs and qualification evidence, not unresolved product choices.
[Needs owner action](plan.md#needs-owner-action) lists the external steps.

### Owner decision, 2026-09-28 08:12

"Finish S1 in parallel and start S2 and S3." C00 waits only for the decided
D1–D5, not M1. Integrated T034/T035 and T038 live evidence gate C08 and C28;
the M1 release remains an M3 exit dependency, not an implementation-start gate.

### Execution rulings, 2026-09-28

The supervisor approved a pure move of ADR-0018's existing filesystem code and
tests into `maestro-filesystem` (C04a), and the S2 G27 public typed-edge port for
S3's separate catalog dependency projection (C27a). S3 supplies the edge schema
and adapters; these are not evidence-span claims. C00 records these boundaries
and the hook disposition below in architecture 03/06/08.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Start a project and ask a cited question (Priority: P1)

As the owner, I select the knowledge preset, preview its files, project the
Maestro profile into Copilot or Pi, and ask a question through the existing
knowledge tools without hand-copying framework configuration.

**Why this priority**: it proves useful content and host integration before
building distribution or routing infrastructure.

**Independent Test**: in an isolated home and fresh project, init → project →
`knowledge_search` → `knowledge_get` → `knowledge_ask` → remove. Use synthetic
content in public tests; keep any approved private proof private.

**Acceptance Scenarios**:

1. **Given** a reviewed source catalog, **When** I run
   `maestro init --catalog-dir "$REVIEWED_CATALOG" --preset knowledge-client`,
   **Then** I see every proposed file and collision, and nothing is written
   until `--apply`; the output says authoring convenience, not verified install.
2. **Given** base and Rust templates, **When** each composition is applied,
   **Then** generated JSON is strict, the project descriptor stays small,
   scripts are not run and copied workflows remain inactive.
3. **Given** a supported pinned host, **When** I project the profile,
   **Then** only its explicit tools and required resources are exposed; Pi loads
   the MCP tool provider as well as the tool names.
4. **Given** a published, permitted generation, **When** I search, get and ask,
   **Then** citations preserve generation, source span and digest; insufficient
   evidence produces a refusal; unavailable or uncalibrated states stay visible.
5. **Given** an existing projection, **When** I rerun or remove it,
   **Then** rerun changes nothing and removal preserves unrelated and edited
   files, including other MCP registrations.

---

### User Story 2 - Install and update a trustworthy catalog (Priority: P1)

As a laptop user, I install a released bundle without cloning the catalog or
building software, and I know an authentic but expired or revoked bundle cannot
be used merely because it is cached or named by an old lock.

**Why this priority**: this is the difference between the first convenience
checkpoint and the M3 release.

**Independent Test**: install an attested tag on a clean machine state, update
it, interrupt an update, revoke it, and retry offline and after restoring an
older backup. Every unsafe admission is refused without replacing a valid
install or losing user files.

**Acceptance Scenarios**:

1. **Given** a protected catalog release, **When** installed, **Then** exact bytes,
   source revision, expected repository, workflow and issuer are verified before
   the installation becomes current.
2. **Given** changed bytes, a wrong signer or unsupported runtime requirements,
   **When** installed, **Then** installation fails closed with no partial switch.
3. **Given** expired records, a replay below the version floor, or a revoked
   entry or bundle, **When** installed, updated, used by init, resolved, searched,
   routed, explained, analysed for impact or projected, **Then** the same admission
   check refuses it, caches included.
4. **Given** still-valid offline records, **When** resolving from the cache,
   **Then** the remaining validity window is shown; at expiry it is refused.
5. **Given** interrupted installation or an old backup, **When** recovered,
   **Then** durable ownership and pins remain consistent, and restored catalog
   use requires a fresh authenticated trust check rather than a restored floor.

---

### User Story 3 - Change preferences without widening permissions (Priority: P1)

As a developer, I change language, verbosity or an allowed model profile and
can see where each effective value came from, without disabling mandatory
checks, weakening permissions or changing another role's settings.

**Why this priority**: customization must not create an authorization bypass.

**Independent Test**: resolve the same project under each override layer;
check allowed neighbours and denied changes through the real resolver and
Cedar evaluator, with zero executor calls on denial.

**Acceptance Scenarios**:

1. **Given** a setting, **When** checked or loaded, **Then** it has exactly one
   class: free, bounded, additive or locked; unknown or duplicate classes fail.
2. **Given** conflicting preferences, **When** resolved, **Then** precedence is
   default → preset → project → user → command, with every source explained.
3. **Given** a wider permission, removed check or larger budget, **When** proposed
   as an override, **Then** permissions still intersect, checks accumulate and
   budgets narrow; locked changes are refused.
4. **Given** a Copilot native hook, **When** it checks an operation, **Then** the
   same Cedar rules allow or deny it; missing facts, unknown tools, opaque shell,
   evaluator errors and timeouts deny. An absent hook is reported as unprotected.
5. **Given** no S4 qualification or observed run, **When** explained, **Then** the
   value is unsupported or not observed, never an invented qualification receipt.

---

### User Story 4 - Find the smallest suitable workflow (Priority: P2)

As the Maestro user, I give an intent and receive a small, explained shortlist
of eligible workflows, with exact dependencies, or an honest no-match,
clarification, incompatibility or temporary-unavailability result. A real M3
install returns `incompatible` ("not qualified until S4") for executable
workflows; synthetic eligibility tests do not qualify a live role or model.

**Why this priority**: explicit selection already serves the first checkpoint;
measured routing comes after safe catalog consumption.

**Independent Test**: run a frozen set of at least 100 independently reviewed
public or synthetic intents against exact-ID/lexical routing, then compare
hybrid on the same held-out cases and frozen synthetic eligibility snapshot.
Check the real install's honest `incompatible` result separately.

**Acceptance Scenarios**:

1. **Given** a denied, revoked or unqualified resource, **When** any retrieval
   branch runs, **Then** that resource is excluded before its candidate limit,
   and cannot reappear through a cache or dependency closure.
2. **Given** a workflow with a mandatory reviewer of low similarity, **When** it
   is returned, **Then** the reviewer is present because the definition requires
   it, not because the reviewer happened to rank well.
3. **Given** an unavailable embedder or stale index, **When** routing,
   **Then** the still-valid authorized bundle supports exact-ID/local lexical
   fallback with an explicit degraded state; an expired bundle does not.
4. **Given** a skill, policy or contract change, **When** I request impact,
   **Then** exact transitive dependencies and their snapshot are returned under
   the caller's scopes, not a similarity guess.
5. **Given** a hybrid comparison without demonstrated gain, **When** choosing
   the shipped route, **Then** lexical routing remains enabled and the failed
   comparison remains in the report.

---

### User Story 5 - Publish a small, checked catalog (Priority: P2)

As a maintainer, I author only resources needed by the two v1 workflows,
check definitions without executing them, and hand a deterministic bundle to
an owner-controlled release workflow.

**Why this priority**: one source of definitions and evidence prevents an
unreviewed library of agents from becoming the platform.

**Independent Test**: reorder source files and timestamps and obtain identical
bundle bytes; mutate one definition and obtain a changed digest; reject each
invalid graph rule and policy fixture through the real checks.

**Acceptance Scenarios**:

1. **Given** a new resource, **When** checked, **Then** its owner, maturity,
   dependencies and architecture 08 rows resolve; an unused speculative role
   or a draft in an executable closure is refused.
2. **Given** `feature-delivery` and `ctm-question`, **When** compiled,
   **Then** all twelve architecture 03 §2.3 rules hold for supported constructs;
   the builder is a deterministic step and reviews remain independent.
3. **Given** skill scripts, hooks or templates, **When** compiled,
   **Then** they remain data; compilation never runs them.
4. **Given** an untrusted pull request, **When** CI checks it, **Then** it has no
   signing or production credentials; only the protected release path attests.

### Edge Cases

- A path escapes its root, repeats an archive entry, follows a link or changes
  between preview and write: refuse without overwriting user data.
- A crash occurs after any owned write: recover from recorded digests; an edit
  made after the crash is not mistaken for generated content.
- User content shadows an agent or MCP name: show the collision and stop;
  never rely on host search order to pick the right resource.
- An input is oversized, deeply nested, malformed, duplicated or contains an
  unknown key/type: bounded parsing returns a named error, not partial success.
- A clock rolls back, a trust refresh is interrupted, or revocation arrives
  during a consult: no stale admission or partly applied trust state is served.
- Host parser support, provider loading or a required model is absent: report
  unsupported or blocked; do not silently drop fields or fall back to a model.
- Revocation arrives after text was loaded into a native session: refuse new
  consultations, explain that existing text cannot be retracted, and require a
  restart/removal for that session. Do not claim runtime containment.
- A private-data denial test shares a kernel with an allowed principal: use a
  separate no-grant kernel; configuration reconciles the local principal's grants.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-S3-001**: Author the catalog from zero, without opening an earlier catalog
  before M3. Every resource MUST serve a named architecture 08 row and workflow.
- **FR-S3-002**: Parse Copilot-native Markdown/frontmatter, TOML and JSON as
  bounded, strict typed data. Reject duplicate/unknown keys, duplicate IDs,
  unsupported kinds, missing body sections, dangling references and dependency
  cycles. Prove `metadata:` support or use the approved sidecar alternative.
- **FR-S3-003**: Check owner and maturity evidence; a label alone MUST NOT qualify
  a resource. Drafts may be discoverable but never enter an executable closure.
- **FR-S3-004**: `maestro init` MUST inspect without scripts, compose base plus
  Rust, preview all files including dotfiles, require explicit apply, validate
  composed output and write only the small descriptor, lock and owned files.
- **FR-S3-005**: A shared owned-file writer MUST resist traversal, links, races
  and changed previews, stop on collision, recover interrupted operations and
  remove only unchanged owned content on all three supported operating systems.
- **FR-S3-006**: Project to Copilot and Pi with explicit tools, dependencies and
  provider loading, no model fallback, collision/shadow checks, preserved
  unrelated registrations and honest registered/observed/stale/failed states.
- **FR-S3-007**: The knowledge checkpoint MUST reuse S1 tools and T038 cases,
  preserving citations, refusals, scope denial and unavailable/uncalibrated
  markers. Codex and Claude Code receive MCP registration guidance, not agent
  projection. Private content and receipts MUST stay private.
- **FR-S3-008**: Compile deterministic sorted tar plus `bundle.json`, including
  entry digests, owners, maturity, source commit, exact workflow closures,
  policy digest, entry points, runtime range, features and tool contracts.
  Compilation MUST NOT execute catalog content.
- **FR-S3-009**: Revalidate the bundle schema, every byte digest and compatibility
  on load. Reject undeclared, duplicate, unsafe, oversized or truncated entries
  before any installation becomes current.
- **FR-S3-010**: Verify with pinned `gh`, fixed arguments and bounded time/output;
  bind exact bytes to the expected repository, workflow, issuer, source and
  digest. Missing, wrong, failing or hung verifiers MUST fail closed.
- **FR-S3-011**: Authenticate timestamp, revocation and version-floor records;
  refresh within five minutes during use, expire offline use within 24 hours,
  apply records atomically and resist clock rollback and replay. Consult them
  before every install, update, init, resolve, search, route, explain, impact
  and projection.
- **FR-S3-012**: Store scoped installs, components, closures, pins and trust
  state in the kernel with content-addressed artifacts. Installation and update
  MUST switch atomically; a failed update retains the prior valid install.
  Backup/restore MUST NOT restore authority to use old trust records.
- **FR-S3-013**: Publish from protected manifests CI using the released,
  checksum-pinned compiler, policy tests, bundle checksums and attestation.
  Separate catalog/runtime publisher identities, rotation and emergency
  revocation MUST be tested. The catalog publisher's scheduled trust workflow
  MUST re-issue attested records every six hours independently of content tags;
  an independent hourly freshness check alerts the maintainer/backup on a missed
  run before expiry. Online refresh requires authenticated pinned `gh` (login or
  `GH_TOKEN`); credentials never enter catalog content. `maestro doctor` checks
  the verifier pin and authentication readiness.
- **FR-S3-014**: Every setting MUST have exactly one override class. Preferences
  use the five-layer precedence; permissions intersect, prohibitions and checks
  accumulate, budgets narrow, secrets remain references, and capability values
  stay local to their role or step.
- **FR-S3-015**: The project lock MUST pin bundle/component/runtime/host/model
  identities and supported execution profiles. Updates are explicit; the lock
  is not trust authority. Explain declared, effective and observed states with
  each value's class, source and requester; absent S4 evidence stays unsupported.
- **FR-S3-016**: Check real Cedar policies and JSON Schema contracts; ship
  default-deny, destructive-operation, protected-path, egress and MCP-allowlist
  rules. Every rule MUST have an allowed neighbour and a denied fixture;
  missing mandatory facts or evaluation errors deny with zero executor calls.
- **FR-S3-017**: Copilot `preToolUse` MUST call `maestro policy check --stdin`
  on normalized supported operations. Unknown tools, opaque shell and hook
  errors/timeouts deny. Model text MUST NOT supply identity or approval.
  This is convenience defence in depth, not the S4 broker or sandbox.
  Pi, Codex and Claude Code hook administration is deferred to S4 host-adapter
  qualification: their trusted event/identity adapters are not qualified in S3.
  Four-client MCP registration remains in S3.
- **FR-S3-018**: Declare `feature-delivery`, `ctm-question`, Maestro, planner,
  coder, tester and reviewer, builder as a step, and only their needed resources.
  Provide `fast`, `balanced` and `deep` profiles for both providers; only
  evidence-qualified role/profile combinations become eligible.
- **FR-S3-019**: Statically check all twelve architecture 03 §2.3 graph rules:
  closure/maturity, reachability, bounded cycles, typed conditions, exact router
  edges, reviewer independence, tool policy coverage, sandbox requirements,
  bounded maps/depth, budgets, state flow and outputs on every successful path.
  S3 MUST NOT execute a graph or invent S4 qualification.
- **FR-S3-020**: Freeze at least 100 independently reviewed public/synthetic
  intents before routing comparisons, including valid alternatives, no-match,
  clarification and adversarial cases. Record suite, synthetic eligibility
  snapshot and profile digests; never import synthetic qualification into a
  real install.
- **FR-S3-021**: Implement exact-ID/local lexical baseline first. Expose
  `catalog_route`, `catalog_resolve`, `catalog_search` in the existing CLI/MCP;
  return typed statuses, reasons, exact dependencies and snapshot identity.
- **FR-S3-022**: Filter maturity, platform, provider/model qualification, scopes
  and data classification before scoring and before every retrieval branch's
  limit. Cache keys MUST include visibility, snapshot, trust/policy freshness,
  runtime constraints and retrieval profile.
- **FR-S3-023**: Index discovery cards through S1 in a separate scoped catalog
  collection, one verified generation per bundle. Definitions remain authority;
  missing or stale indexes MUST NOT redefine dependencies or leak knowledge.
- **FR-S3-024**: Compare S1 dense/BM25/fusion with the baseline on held-out paired
  cases. Enable hybrid only for demonstrated gain; retain a valid offline
  exact-ID/lexical route and report degraded or unavailable components.
- **FR-S3-025**: C27a MUST project C12's exact scoped component edges through
  S2 G27's public typed-edge port after S2 G25 qualification, as a separate,
  rebuildable catalog projection. `catalog_impact` traverses that projection
  using application IDs, identifies the snapshot and survives a rebuild.
  Catalog edges never become evidence-span claims; an in-memory closure fallback
  is not implicitly approved.
- **FR-S3-026**: After M3 and access approval, compare the earlier catalog once;
  record every item's disposition, provenance and reason. Each recovery is a
  separate reviewed task of at most four hours, with attribution and tests.

### Key Entities

- **Resource**: stable ID, kind, version, owner, maturity evidence, content digest
  and exact references; never an authority grant.
- **Bundle**: immutable compiled entries and closures, compatibility contract and
  source identity, verified independently of authoring checks.
- **Install and trust snapshot**: scoped kernel records tying artifacts and pins
  to authenticated freshness, revocations and a monotonic version floor.
- **Project and ownership manifest**: descriptor and explicit lock plus the
  paths/digests Maestro wrote, including owned portions of shared host settings.
- **Discovery card and route result**: searchable description versus exact
  definitions; results bind eligible candidates to the consulted snapshot.
- **Policy/qualification evidence**: rule decisions and measured role/profile
  support; declarations, tests and observed execution are different evidence.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-S3-001**: The first checkpoint completes init → project → search/get/ask
  → remove on both pinned hosts, with exact citation checks, refusal and
  no-grant cases, idempotence and no loss of edited or unrelated files.
- **SC-S3-002**: A tagged, attested bundle installs without a repository checkout,
  Rust toolchain or Python. The owner supplies a disposable clean WSL user or
  container with only the released `maestro`, checksum-pinned `gh` and basic
  shell utilities; a script asserts `cargo`, `rustc` and `python3` are absent
  before installing. Reordered inputs and timestamps yield identical compiled
  bytes; a changed definition changes its digest.
- **SC-S3-003**: Tamper, wrong signer, incompatibility, expiry, revocation, replay,
  interruption and old-backup cases all fail closed. Offline use shows its
  remaining window and refuses at expiry; no cache bypasses admission.
- **SC-S3-004**: Base and base-plus-Rust composition tests cover every generated
  file; all collision, race, crash-recovery and owned-removal cases pass.
- **SC-S3-005**: Every setting has one class, every policy has passing allow/deny
  neighbours, and every graph validation rule has a passing and failing case.
  Denial tests reach the real control and observe zero executor calls.
- **SC-S3-006**: On the frozen 100+ intent suite and frozen synthetic eligibility
  snapshot, top-3 accuracy is at least 90 % on matchable cases. C23 records both
  digests and a nonempty matchable denominator. Report top-1, correct no-match,
  clarification, exact dependency completeness, unnecessary context and
  distractor results separately. Required closure completeness is 100 %. Hybrid
  ships only if its paired gain is demonstrated; otherwise the baseline is the
  shipped route. C28 also verifies a real M3 install returns `incompatible`
  ("not qualified until S4"), without claiming synthetic results qualify it.
- **SC-S3-007**: Catalog discovery and impact rebuilds preserve scoped results
  and snapshot identity; no catalog/knowledge scope leakage is observed.
- **SC-S3-008**: Final CI passes on Linux, macOS and Windows, with line coverage
  at least 90 % overall and 95 % on changed lines, zero missed mutants and zero
  mutation timeouts. Mutation and coverage run in CI only, not on the workstation.
- **SC-S3-009**: M3 evidence names every required traceability row and delivered
  portion, with no missing, duplicate or extra keys; private receipts stay
  private. Missing host, release, S2 or CI evidence blocks the corresponding exit.

## Out of Scope

- Workflow execution, durable engine, daemon, broker enforcement, sandbox,
  runtime acceptance and general provider/role qualification: S4. Native hooks
  and filesystem safety are not substitutes for those controls.
- A new Pi extension, provider fallback, a separate catalog service, remote MCP,
  GUI, team deployment or multi-user operations.
- Extra roles, languages, three-way generated-file merge and bulk import of an
  earlier catalog. Comparison is a separately gated post-M3 task.
- S1 cleanup, answer calibration, corpus deduplication, Qdrant Edge, unrelated
  gate changes and the post-M1 backlog. Reuse approved privacy checks when they
  land; do not fold their implementation into S3.

## Traceability

Architecture 08 statuses are design dispositions, not proof of delivery. C00
freezes the exact row-key inventory and S3 portions before implementation;
C28 adds integrated evidence. Preserve combined row keys verbatim rather than
inventing a count or splitting identifiers differently from their source.

| Exact 08 row key | S3 portion; remaining boundary | Tasks |
| --- | --- | --- |
| `owner.catalog` | Fresh content and later comparison | C02, C21, C21b, C29 |
| `owner.m001.manifest` | Catalog definitions; InnerSource operation stays S5 | C03, C21, C21b |
| `owner.m001.team` | Role declarations; execution stays S4 | C21b, C22a, C22b |
| `owner.m001.cli` | Project bootstrap | C04a, C04, C05 |
| `owner.m001.load` | Bootstrap/preferences; human approvals remain | C05, C17, C18 |
| `owner.m001.laptop` | Detached install/native projection | C06, C07, C16, C28 |
| `owner.m001.guardrails` | Policy and Copilot hook; broker stays S4 | C19, C20 |
| `owner.m024` | Separate catalog and runtime releases | C15, C28 |
| `owner.m028` | Workflow discovery and exact impact | C23, C24a, C24, C25, C26, C27a, C27 |
| `owner.m032` | Restrictive settings; governed enforcement stays S4 | C17, C18, C19 |
| `chat.M006 layers` | Definitions through verified install; enforcement stays S4 | C03, C10, C16 |
| `chat.M006 layout, M023 layout` | Required v1 content only; capability expansion stays S5 | C02, C21, C21b |
| `chat.M006 compiler` | Strict definitions and static graph checks | C03, C22a, C22b |
| `chat.M006 explain` | Every effective setting's provenance | C17, C18 |
| `chat.M006 lockfile` | Exact pins; live execution qualification stays S4 | C18 |
| `chat.M006 project file` | Small non-authoritative descriptor | C05 |
| `chat.M006 transparency` | Declared/effective/observed; external export stays S5 | C18 |
| `chat.M006 release` | Trusted catalog lifecycle | C13, C14, C15, C16, C16b |
| `chat.M019 maturity, M023 step 4` | Evidence checks; role qualification stays S4 | C03, C21, C21b, C24a |
| `chat.M019 descriptor` | Native format or fixed v1 sidecar fallback | C01, C03 |
| `chat.M019 ownership` | Required owners/CODEOWNERS; dual-approval operation stays S5 | C02, C21, C21b |
| `chat.M027 detached` | No checkout or toolchain on the laptop | C16, C28 |
| `chat.M027 validation split` | Author/load validation; governed enforcement stays S4 | C03, C11, C13 |
| `chat.M027 invariants` | No verification bypass or fake receipts; broker stays S4 | C14, C16b, C18 |
| `chat.M027 contract crate` | In-workspace schemas, no separate contract crate | C03, C10, C11 |
| `chat.M027 versions` | Runtime/bundle/tool compatibility | C10, C11, C18 |
| `chat.M027 pipelines` | Separate publishers, unprivileged PR checks | C09, C15 |
| `chat.M027 TUF` | Freshness, revocation, floors and offline expiry | C09, C14, C15, C16b |
| `chat.M036 classes, M039 policy` | Restrictive override classes | C03, C17 |
| `chat.M036 defaults` | Authored defaults; runtime application stays S4 | C17, C21, C21b |
| `chat.M006 roles, M023 step 12` | Required v1 roles only; later capabilities stay S5 | C21, C21b |
| `chat.M006 change workflow` | Declarative development workflow; execution stays S4 | C21b, C22a, C22b |
| `chat.M019 machine contracts` | Static schema validation; acceptance stays S4 | C19, C21, C21b, C22b |
| `chat.M039 superpowers` | Explicit stages and preserved approval obligations | C21b, C29 |
| `delivery.R08` | Independent review declarations; runtime contexts stay S4 | C21b, C22a |
| `chat.M036 models` | Profile declarations; provider qualification stays S4 | C21, C21b |
| `chat.M059 model profile` | Profile identities and pins; live evidence stays S4 | C18, C21, C21b |
| `chat.M006 broker` | Shared native evaluator only; authoritative broker stays S4 | C19, C20 |
| `chat.M006, M023 step 8` | Normalize supported native operations | C20 |
| `chat.M006 destructive` | Real policy fixtures by effect/scope | C19, C21 |
| `chat.M006 approvals` | No model-supplied approval; bound approval execution stays S4 | C19, C20 |
| `chat.M023 step 7` | Real Cedar and fail-closed facts/errors | C19 |
| `chat.M031 principle` | Definitions remain authority, evidence gates eligibility | C24a, C24 |
| `chat.M031 workflow first` | Exact declared closure after workflow selection | C24a, C24 |
| `chat.M031 API` | Typed local CLI/MCP routes and caller context | C24a, C24 |
| `chat.M031 cards` | One replaceable card per resource | C25 |
| `chat.M031 hybrid` | Pre-limit eligibility and measured conditional hybrid | C26 |
| `chat.M031 optimal` | Smallest qualified workflow; no automatic feedback rewrite | C24, C26 |
| `chat.M031 separation` | Catalog/knowledge collection and scope separation | C25 |
| `chat.M031 offline` | Valid exact-ID/local lexical fallback | C14, C24 |
| `chat.M031 publication` | Verified generation/snapshot identity | C25 |
| `chat.M031 security` | Local caller context/cache isolation; HTTP remains out of scope | C24a, C24, C26 |
| `chat.M031 delivery` | Frozen baseline before hybrid | C23, C24, C26 |
| `chat.M031 service` | Existing server, no separate catalog service | C24a, C24 |
| `chat.M019 bootstrap, M023 step 13` | Inspect/preview/apply/validate/ownership | C04, C05 |
| `chat.M019 overlays` | Base plus Rust composed output | C02, C05 |
| `chat.M006 native` | Convenience projection, drift and owned removal | C06, C07, C20 |
| `product.GD2, GD4, GD5` | Four MCP clients, local access and Copilot hook; other hooks stay S4 | C00, C06, C07, C08, C20, C28 |
| `chat.M048 real controls` | Real verification/Cedar and allow/deny neighbours | C09, C13, C19, C28 |
| `chat.M057 CI` | Zero relevant tests cannot pass; honest statuses | C15, C28 |
| `chat.M059 provenance` | Observed/unsupported/not-run kept distinct | C08, C18, C28 |
| `delivery.U02` | Catalog consistency; translation comparison after M3 | C03, C21, C21b, C29 |
| `delivery.U05` | Consumption, compiler, closure and overrides | C10, C11, C17, C22a, C22b |
| `delivery.U09` | Deterministic bootstrap and projection | C04a, C04, C05, C06, C07 |
| `delivery.U13` | Catalog routing; S1 retains retrieval/model qualification | C23, C24a, C24, C25, C26 |
| `delivery.U17` | Bundle trust/lifecycle; general InnerSource stays S5 | C09, C15, C16, C16b, C28 |
| `core storage` | Scoped kernel authority, rebuildable projections | C12, C25, C27a |
| `core verification` | Organization gates, not waived for S3 | C28 |
| `delivery.§1.5` | Exact S3 delivery evidence; other slices retain their portions | C00, C28 |

C00 obtains OA7 approval of the frozen key inventory and records D1/D5,
concurrent S1/S2/S3 starts, the S3/S4 compiler boundary, the `product.GD2, GD4,
GD5` hook disposition and C27a's S2 G27 seam in architecture 03/06/08. C09 owns
closure of 08 §17's "Publisher identities, trust roots, key rotation procedure"
row with D2 and OA4 evidence; without that evidence C28 remains blocked. These
are future edits, not a claim they have landed. Runtime enforcement, general
qualification and InnerSource keep their named later slice. No similarity or
unapproved closure fallback replaces C27a/C27.

## Assumptions

- S1's knowledge tools exist on the integration base. M1 completion does not
  gate S3 implementation; integrated T034/T035 and T038 live evidence gate C08
  and C28, and the M1 release gates the M3 exit.
- The reference proof runs on WSL; portable crate tests do not qualify native
  clients, hosted providers or containment on untested systems.
- The owner supplies the approved host binaries, accounts, public repository,
  publisher identities and release operations. Workers prepare reviewed files
  and synthetic tests only until those actions are complete.
- S4-only qualifications can be represented as unsupported in S3 declarations.
  Synthetic eligibility fixtures test routing logic, never qualify a live model.
- Repository and trust changes remain owner-controlled even though D1–D5 are
  decided. The outward steps are listed in [the plan](plan.md#needs-owner-action).
