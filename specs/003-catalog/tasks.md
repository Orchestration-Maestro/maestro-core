# Catalog Implementation Tasks

> **For agentic workers:** use superpowers:executing-plans for the one task in
> your brief. A lane starts no subagents and never integrates its own work.

**Goal:** a small useful Copilot/Pi catalog first, then M3 "Catalog installable",
without turning native convenience into a claim of governed execution.

**Architecture:** a catalog crate over the S1 kernel/knowledge pipeline and
one pure extraction of the existing ADR-0018 filesystem code into
`maestro-filesystem`. Thin adapters use the existing CLI/MCP server. Content
lives in the owner-created manifests repository; S4 owns graph execution.

**Tech Stack:** Rust 1.98.1, MSRV 1.98, existing workspace parsers/storage/MCP,
pinned `gh`, measured minimum tar/JSON Schema/Cedar dependencies.

**Input and prerequisites:** [spec.md](spec.md), [plan.md](plan.md) and the
owner-approved D1–D5 decisions. The 08:12 decision starts S3 beside unfinished
S1. Only C08/C28 require integrated T034/T035 and T038 live evidence; M1 release
is an M3 exit dependency. The 11:25–12:05 owner amendments add workspace
preferences, bounded conversational language tags, client instructions, verified
startup updates, user-approved path trust and small replaceable ports. The evening
amendment adds descriptor-only kinds, owner-approved model cards and S1 settings
reuse; round 2 incorporates the 21:02 review ruling, explicit C16h registration,
the owner's 20:48 registry-generated every-setting editor and 20:50 later glossary/
source-class kinds. The final manifest v4 amendment (2026-09-30) updates C30–C43 and adds
C44–C96b: mandatory standards, root languages, delegated ownership, three
backends, typed content/lifecycle/checkpoints and bounded gap kits under D13–D15. Research, data model and contracts are in
the plan.

**Format:** `Cnn [USn] title (hours)`, with red/green/check steps. Each task's
**Phase** and **After** fields are authoritative; physical workstream order is
not execution order. Phase 1 means M3; Phase 2 starts immediately after M3.
**143 S3 tasks / 478–490 h** comprise **108 Phase 1 tasks / 357–369 h** and
**35 Phase 2 tasks / 121 h**. Three S6 tasks add 6–10 h; transferred C20 adds
4 h in S4, not S3. All 147 headings total 488–504 h across those boundaries.
The amendment is **150 h Phase 1 + 121 h Phase 2 = 271 h**, including the
retained S2 7 h, C02's original +1 h v4 adjustment and +1 h validation fix.
The fix raises the approved Phase 1 149 h by exactly 1 h for C02's Rust
profile/reference, MCP base and refusal cases; it replaces, not adds to, the
approved 27 h engine amendment.
[Accounting](#critical-paths-and-effort) reconciles every row and critical path.
Completed work retains historical estimates, not new remaining effort.

The approved inputs are `manifest-design-final.md` §§8.3–8.6 and
`manifest-gap-analysis.md` §§3, 6–7 (2026-09-30). C44 records their minimal
option; no intermediate layout, full-option duplicate or zero-hour recovery.
C20's live hook moves to S4. G12 context limits are Phase 2 (6 h), per the
supervisor's correction; G07–G11 are the Phase 1 12 h guides/guard bundle.

## Global Constraints

- Test first: write the named failing case, run it and retain its failure;
  implement the least code/content that passes, then run the named checks.
  Spikes and documentation tasks start with a falsifiable probe/check, not an
  implementation claim. Missing host/access evidence is blocked, not passed.
- One fresh clone, one task, one signed commit per repository. Branch from the
  current `origin/feat/s3-integration`; push only the lane branch with
  `--no-verify` after checks. No lane PR to main, self-integration, deployment,
  repository creation, settings change, tag or release publication.
- Read the current lane rules. Every Cargo command uses
  `~/.local/bin/capped` and `CARGO_BUILD_JOBS=3`; use focused tests while editing,
  then workspace tests once before pushing Rust changes. Run org-configured
  Clippy on Linux, Windows and macOS, formatting, guide/docs/links,
  architecture/duplication/licences and vet when dependencies change.
- Mutation and coverage run **only in CI**. Do not start local mutation or full
  local CI runs. Main merge needs ≥90 % overall and ≥95 % changed-line coverage,
  zero missed mutants and zero mutation timeouts, plus three-OS tests/Clippy.
  Supervisor review and integration checks still gate landings.
- Public files/tests/logs contain no private vendor material, personal path or
  secret. Keep private receipts at the owner-approved private location. Read only the owner-approved legacy inputs for the named recovery tasks;
  C29 audits their provenance before M3. No private collection access follows. No task may expand its stated budget by
  weakening scope, validation or tests; split a discovered overrun explicitly.
- C05a/C05b/C05d/C17/C18 extend the integrated S1 settings registry, strict
  preference parser/resolver, config commands and change journal. Their files
  below are catalog adapters/tests, not a second implementation. Preserve the
  landed S3 authority/discovery/MCP rules; a conflicting S1 API needs a ruling.
  The supervisor synchronizes required landed S1 commits into S3 before C17 and
  records the integration head; the `fd39783` base still carries S1 `dac543c`.
  **Supervisor ruling, 21:02: S1 registry names are canonical**; catalog-only
  descriptors extend S1, never duplicate its keys or use uncommitted APIs.

### Paths and checks

`CORE` is maestro-core. `MAN` means the separate, owner-created
maestro-manifests checkout; a task marked MAN commits there only. All other
paths are relative to CORE. A MAN, RW (rust-workflows) or ORG (organization
.github repository) qualifier applies until the next repository qualifier.
Brace lists name exact files, not directory-wide permission to refactor. Tests live beside source or in the existing `it`
process suite. New modules update only their necessary `mod.rs`/`lib.rs` lines;
process test modules register in `crates/maestro/tests/it/main.rs`.

Commands below assume the pinned toolbelt is on PATH and `CARGO_BUILD_JOBS=3`
is exported. `$MAESTRO_BIN` is the tested/released compiler selected for the
stage; `$MANIFESTS` is the reviewed source checkout, and `$HOST_HOME` is a
disposable approved test home. A passing filtered test command must execute
at least one relevant test. For live tests, an ignored/skipped probe is not a
pass. Every task ends with the applicable checks above and a signed commit.

Shared files are serialized by the supervisor: `Cargo.toml`, `Cargo.lock`,
`maestro-quality.toml`, `supply-chain/audits.toml`,
`crates/maestro/src/cli/{args.rs,run.rs,mod.rs}`, module registrations, MCP
dispatch, migration registration, `docs/how-to/{catalog.md,knowledge-mcp.md}`,
`crates/maestro-catalog/src/hosts/copilot.rs`, MAN `README.md` and the generated
Copilot guide. Serialize C26's retrieval edits with S2 G12/G14 (including R4
if present), and MCP dispatch with all slices. Do not create a new command/module
before its first working behaviour.

## Workstream 1: First useful content, preferences and owner loop [US1, US3, US5]

### C00 Contract and traceability [US1, US2, US3, US4, US5] (2 h)

**Phase:** P1/M3

**After:** D1–D5 already decided; no M1 start gate.
**Files:**
`crates/maestro-conventions/tests/catalog_traceability/mod.rs` and its
`inventory_checks.rs` and `field_regressions.rs` modules,
`crates/maestro-conventions/{Cargo.toml,tests/policies.rs}` (supervisor-approved
existing `serde_json` test use and one integration-test binary),
`specs/003-catalog/{spec.md,plan.md,tasks.md,traceability.json}`,
`docs/architecture/{03-agent-orchestration.md,06-roadmap.md,08-traceability.md}`,
`docs/adr/0010-spec-kit-and-executable-gates.md` (approved scoped amendment).
The supervisor serializes `Cargo.lock` and `.github/copilot-instructions.md`
for the existing dependency edge and generated-guide registration.
**Requirements:** FR-S3-001, FR-S3-019, FR-S3-024, SC-S3-009.

- [x] **Step 1: Red.** Derive S3 candidates from 08 source tables; fail if a
  candidate is neither included nor explicitly excluded with a reason. Reject
  missing, duplicate or extra exact keys in the JSON and spec, missing/different
  portions or tasks, and lost/unnamed remaining slices; prove each mutation
  fails its own check. Task IDs must match actual C-number task headings.
- [x] **Step 2: Green.** Freeze exact keys/S3 portions, approved by the owner,
  2026-09-28 (C00 inventory approval). In 03/06/08 record D1's authoring boundary, D5's synthetic
  conditional hybrid result, the 08:12 parallel start and S3 static/S4 runtime split. For
  `product.GD2, GD4, GD5`, C00 originally kept four-client MCP and Copilot
  `preToolUse` in S3, deferring other hooks. **Superseded by C44:** four-client
  MCP stays S3; all live hooks, including C20, move to S4 qualification.
  Name C27a's catalog schema/
  adapters and S2 G27's public port. Coordinate next-free migrations above all
  landed/reserved numbers across main, S1/S2/S3 and deployment-modes, never a
  fixed/gapped block.
- [x] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-conventions catalog_traceability`,
  `rumdl check specs/003-catalog`, and offline links for the edited Markdown.

**Acceptance:** every exact row has a disposition, no false delivered status,
no open D1–D5 choice, and architecture agrees on routing, hook scope, impact
ownership and checkpoints. The frozen inventory is approved by the owner,
2026-09-28 (C00 inventory approval). External operations are not run.

**C00 integrated evidence:** `746df13`; supervisor-confirmed integration check:
2,099 tests and Clippy on three targets. The analyze-fixes lane re-runs C00's
conventions gates; completed checkboxes are ticked by supervisor authorization.

**C00 evidence boundary:** [traceability.json](traceability.json) and the
conventions check contain 85 included exact rows with planned portions and
named remainders, plus six explicit S5-only exclusions. This inventory is
approved by the owner, 2026-09-28 (C00 inventory approval); fixture checks are not M3 delivery
evidence. Architecture 03/06/08 records the decided contract.

### C01 Real host format probe [US1, US5] (3 h)

**Phase:** P1/M3

**After:** C00; OA2 approved on 2026-09-28 for already-installed tools in isolated
temporary homes only. No installs/upgrades, real-configuration or enterprise
policy changes; anything broader needs fresh approval.
**Files:** `crates/maestro/tests/it/catalog_host_probe/`,
`tests/fixtures/catalog/hosts/{metadata.agent.md,sidecar.agent.md,sidecar.maestro.toml,pi.md}`,
`specs/003-catalog/research/hosts.md`.
**Requirements:** FR-S3-002, FR-S3-006, SC-S3-001.

- [ ] **Step 1: Red.** Build a synthetic profile probe requiring metadata
  preservation, exact tool exposure, a real MCP call and a detected same-name
  shadow. Run it with the provider absent to demonstrate the failure boundary.
- [ ] **Step 2: Green.** Test both Copilot metadata/sidecar shapes on the pinned
  real parser in isolated temporary homes. Record exact installed versions as
  pins for Copilot CLI, Pi, Claude Code, Codex and adapters used, plus host
  digests, lookup/reload and Pi's explicit tool/provider/
  skill mapping. Integrated [C01 evidence](research/hosts.md) (`0be954b`)
  confirms agent sidecars. The unknown-key skill control also loads silently:
  skill `metadata:` rests on the Agent Skills specification, not parser silence.
  A host warning on skill metadata reopens the ADR-0005 sidecar decision.
  Record in-session reload as not run until measured; re-probe changed pins.
- [ ] **Step 3: Check.** Run the explicit live `catalog_host_probe` cases:

  ```sh
  ~/.local/bin/capped cargo test -p maestro --test it catalog_host_probe -- \
    --ignored --nocapture
  ```

  Keep synthetic receipts in the research note, with unsupported/not-run distinct.

**Acceptance:** an evidence-backed shape per resource kind and both host mappings;
missing format evidence blocks C03 too, not independent C04a/C09 work.
No new extension and no implicit model fallback.

### C03 Strict source checker and kind registry [US1, US5] (6 h)

**Phase:** P1/M3

**After:** C00, C01's integrated [host-format evidence](research/hosts.md)
(`0be954b`, ADR-0005): confirmed agent sidecars and specification-backed skill
metadata, not support inferred from the silent unknown-key control.
**Files:** `crates/maestro-catalog/{Cargo.toml,src/lib.rs}`,
`crates/maestro-catalog/src/{limits.rs,limits/tests.rs}`,
`crates/maestro-catalog/src/source/{mod.rs,types.rs,parse.rs,check.rs,registry.rs,descriptor.rs,load.rs,metadata.rs}`,
`crates/maestro-catalog/src/source/kinds/{mod.rs,builtin.rs,settings.rs}`,
`crates/maestro-catalog/src/source/tests/{mod.rs,registry.rs,parse.rs}`,
`crates/maestro/src/cli/catalog/{mod.rs,check.rs}`,
`crates/maestro/tests/it/catalog_check.rs`,
`tests/fixtures/catalog/source/{valid.agent.md,valid.maestro.toml,invalid.agent.md,preset.toml}`,
`tests/fixtures/catalog/source/valid-skill/SKILL.md` in C01's decided format;
workspace/CLI registration files from the shared list.
**Requirements:** FR-S3-001, FR-S3-002, FR-S3-003, FR-S3-014, FR-S3-037,
SC-S3-005, SC-S3-013.

- [ ] **Step 1: Red.** Add passing-neighbour and refusal fixtures for duplicate
  IDs/keys, unknown keys/types, oversized/deep input, missing body sections,
  dangling references, dependency cycles and placeholder/authored/retired
  closure members, with a declared reviewed/named-owner neighbour per D7.
  Reject missing/empty owner, missing or unknown 08 row and a resource unused by
  any workflow. Test source byte/depth/resource boundaries at small injected
  `Limits` values and each one past, before unbounded parser allocation; assert
  every D2 production constant once in `limits/tests.rs`, not with giant inputs.
  Reject unknown, unclassified and doubly classified setting keys. Test a
  mismatched agent stem/name and ambiguous sidecar pairing against a valid pair.
  Inject a synthetic glossary descriptor with a finite float and nested table:
  check valid data, unknown/missing fields, dangling references and absent
  descriptor without a parser/checker branch. Keep it for C10/C11/C12/C16. Add a
  model-card-like descriptor with a version field, nested identity and f64
  sampling fields; a named test hook must receive the complete subtree. Non-finite
  TOML floats and unknown hook names refuse. Register deserialized descriptors
  and retain the built-ins' semantic refusals, not only a data round-trip.
- [ ] **Step 2: Green.** Agent `name:` must equal its file stem (before
  `.agent.md`), so `<stem>.maestro.toml` pairs with exactly one agent. Skills
  use specification-backed `metadata:`; a host warning reopens ADR-0005's
  sidecar decision rather than silently dropping a field. Agent sidecars follow
  the integrated probe. Add the first checker/CLI using installed parsers,
  one shared immutable `Limits` value and precise path/key diagnostics.
  Use serde-able descriptor constants behind a loader seam, one per kind;
  validate descriptors and expose each resource's full file set. A built-in
  `Registration` pairs a descriptor with an existing hook named in the descriptor;
  resolve against the fixed table, refusing unknown names. Add Eq-safe finite
  floats and a structured-table field type passed whole in serde form to
  `KindRules`. Generic metadata/reference checks consume descriptors; isolate
  special-rule hooks and reject undescribed directories. Expose known settings
  through a port like `KnownRows`; C17 replaces its temporary list with S1 keys.
  No file-loaded descriptors, JSON source format or production model-card kind
  in C03; C03a adds the descriptor/consumer without changing generic machinery.
  Validate declared stage/owner; do not invent completed-review evidence from
  a label. OA1/C15 enforce protected-branch CODEOWNERS review. Unimplemented graph/policy
  features are unsupported, never silently accepted as fully checked.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog source::tests` and
  `~/.local/bin/capped cargo nextest run -p maestro catalog_check`.

**Acceptance:** valid sources round-trip; every invalid neighbour fails;
checking executes no template/script. Fixed agent sections are Purpose,
Responsibilities, Inputs, Working sequence, Outputs and Boundaries.

### C03a Model-card declaration and kernel adapter [US1, US5] (4 h)

**Phase:** P1/M3

**After:** C03; existing S1 v2 cards/scoped registry integrated. S2 G17 is required
only for extractor acceptance; unsupported query_expander waits for S1's named
post-M1 role follow-up, never an S3 alias or role extension. No MAN prerequisite.
**Files:** `crates/maestro-catalog/src/source/kinds/{mod.rs,builtin.rs,model_card.rs}`,
`crates/maestro-catalog/src/model_cards/{mod.rs,declaration.rs,register.rs,tests.rs}`,
`tests/fixtures/catalog/model-cards/{valid.toml,invalid.toml}`,
`crates/maestro-kernel/src/gateway/card.rs` and
`crates/maestro-kernel/src/gateway/tests/{v2.rs,v2_golden.rs}` (pure constructor
extraction only, explicitly an S1 kernel edit); necessary module registration,
no kernel schema/role migration or public artifact-digest query.
**Requirements:** FR-S3-001, FR-S3-002, FR-S3-038, FR-S3-039, SC-S3-014.

- [ ] **Step 1: Red.** Use public synthetic v2 identities to assert declaration
  round-trip and the existing kernel canonical digest, including sampling/output/
  template and measured/unavailable qualification fields. Refuse unknown/duplicate
  keys, invalid identity, secrets/paths, missing local evidence and unauthorized
  collection registration. The unsupported-role adapter test must reach the
  kernel's own unknown-role refusal, not a hard-coded planned-role table.
  Same-card registration is a no-op; identity mutations produce different
  fingerprints. Assert zero selection calls; do not duplicate S1's synthetic
  evaluation-cannot-select guard test.
- [ ] **Step 2: Green.** Add the `model-card` descriptor and thin D12 adapter over
  `CardIdentity`, not copied field types or validation. Declare `version` as a
  model-card field, not a common metadata key. Share a pure
  `ModelCard::from_identity(&CardIdentity) -> Result<ModelCard, CardError>` with
  `record_v2` if needed. Check has no artifact writes. Explicit registration uses
  `Database::record_model_card`/`NewModelCard` with all identity-referenced evidence
  already local. Rely on the transactional pin refusal for absent artifacts and
  existing artifact-store integrity, not a new `artifact_digests()` public query.
  Never fabricate evaluation/selection records, import evidence, fetch, load or
  configure a model. Each machine qualifies its own backend/runtime/hardware-bound
  card. Preserve D12's latest-registration and earlier-card re-registration caveats.
  Keep this internal until C16h supplies admitted installed input; no unsigned CLI shortcut.
- [ ] **Step 3: Check.** Run capped nextest `model_cards::tests` in maestro-catalog
  and `gateway::tests` plus `model::tests` in maestro-kernel. Retain unchanged v1
  reads and v2 golden identities; assert zero selection calls and no edits to
  generic checker/compiler/reader/installer logic. Unsupported roles use the
  kernel refusal; extractor acceptance follows S2 G17, included before M3 with
  S2, while query_expander remains unsupported until S1's named follow-up.

**Acceptance:** one descriptor plus the small existing-kernel validation/consumer
adapter; no second model registry, bespoke installer, bake-off runner or fake
qualification. No fabricated evaluation/selection record; explicit registration
can affect a later same-entry ask under S1's existing lookup, never rewrite history.

### C02 Minimal v4 catalog seed [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C00, C03, C39, C51a, C52b; OA1 approved identities/protection.
**Files:** MAN package.toml; core/package.toml; core/agents/maestro.{agent.md,maestro.toml}; skills/knowledge-evidence/SKILL.md; core/instructions/knowledge.{instructions.md,maestro.toml}; core/backends/mcp/config.toml; languages/rust/{package.toml,profiles/quality/default.toml,instructions,bootstrap}; standards/<domain>/package.toml; capabilities/orchestration/application-workflow/package.toml; presets/{knowledge-client,rust-service}.toml; bootstrap/repository.toml and bootstrap/repository/files/; .github/CODEOWNERS; README.md; docs/standards/.
**Requirements:** FR-S3-001, FR-S3-002, FR-S3-003, FR-S3-007, FR-S3-040, FR-S3-041, FR-S3-042, FR-S3-046, FR-S3-047, FR-S3-050, FR-S3-051, FR-S3-056, SC-S3-004, SC-S3-021.
**Named tests:** `seed_missing_knowledge_skill_refuses`, `seed_unknown_setting_refuses`, `minimal_common_rust_starters_accept`, `seed_missing_rust_profile_refuses`, `seed_missing_mcp_config_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; remove the Rust profile/reference or MCP config independently and
  require a refusal. Run the focused fixture/check command and retain failures.
- [ ] **Step 2: Green.** Write approved area records, knowledge resources, existing
  two presets and minimal standard/common/Rust starter fixtures at final paths.
  Include the conforming `quality-profile:rust/default` and its required reference
  in `languages/rust/package.toml`, alongside instruction/starter references.
  Bind the seeded `standard:quality` baseline exactly; retain every gate category,
  applicability, thresholds, evidence/failure rules and manager/tool choices.
  Missing runtime bindings stay unresolved, never reported as passing gates.
  Include a minimal checked MCP base for the existing S1 knowledge server in
  `core/backends/mcp/config.toml`, bound through `package:core` and the presets;
  no `mcp:` resource/edge, new server or graph/vector qualification dependency.
  Use exact qualified IDs and generated ownership/index/schema outputs.
  C76a extends this same Rust profile; C48 reuses this MCP base when publishing
  qualified backend declarations. Standards imports join through C82 before
  publication; never invent approved rules/owners. C21b/C70/C72 add remaining
  framework content. No package-new, generic eval runner, extension or full
  historical starter dependency.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Public synthetic seed is checked, useful and inert, with complete
Rust profile/instruction/starter references and the MCP configuration C21 reuses.
Strict recipes JSON and missing prerequisite diagnostics remain; no private text,
model selection, hook launch or qualification claim. C02 is 2 h historical seed +
1 h v4 adjustment + 1 h validation fix for the minimal Rust profile/reference,
MCP base and their refusal cases = 4 h, charged once. C76a/C48 retain their hours.

### C02a Reviewed model cards in the manifest [US1, US5] (2 h)

**Phase:** P1/M3

**After:** C02, C03a; OA1 and owner-supplied approved public card identities/evidence
references. Extractor content waits for S2 G17; do not invent query_expander data.
**Files (MAN):** `core/llm/models/<role>/<name>.toml` for the owner-approved workflow
dependencies only, `core/package.toml`, `presets/knowledge-client.toml`,
`.github/CODEOWNERS`, `README.md`. Use qualified model-card IDs and area-derived ownership;
canonical kernel identity bytes remain unchanged.
**Requirements:** FR-S3-001, FR-S3-003, FR-S3-038, FR-S3-039.

- [ ] **Step 1: Red.** Check a missing/dangling card reference and an authored
  card in the preset closure with C03a; retain refusals before adding reviewed
  content. Compare each approved identity's canonical digest with the digest of
  its owner-supplied canonical card JSON; no kernel export command is assumed.
- [ ] **Step 2: Green.** Author only required cards with owners, versions,
  reviewed maturity and 08/workflow references. Preserve all v2 identity fields;
  attach only approved public qualification references, never private reports,
  machine paths or credentials. Each bake-off winner arrives as an owner-approved
  manifest diff, not an edited runtime default. Keep M059 agent profiles separate.
- [ ] **Step 3: Check.** Run `"$MAESTRO_BIN" catalog check --catalog-dir "$MANIFESTS"`;
  compare card fingerprints and record owner review of each winner change. Missing
  approved identity/evidence blocks content rather than creating placeholders.

**Acceptance:** reproducible versioned declarations, no model downloads or kernel
selection effects. C08 can still use existing S1 cards without waiting for this
content; C28 includes it before M3. C15/C21 do not wait for owner-approved winners.

### C04a Share the existing ADR-0018 filesystem [US1] (2 h)

**Phase:** P1/M3

**After:** C00.
**Files:** `crates/maestro-filesystem/{Cargo.toml,src/lib.rs}`,
`crates/maestro-filesystem/src/{root.rs,unix.rs,windows.rs}`;
move from `crates/maestro-canonicalization/src/filesystem/{mod.rs,root.rs,unix.rs,windows.rs}`;
`crates/maestro-canonicalization/{Cargo.toml,src/lib.rs,src/store.rs}`,
`crates/maestro-canonicalization/src/tokenizer/artifacts.rs`,
`.cargo/mutants.toml` (17 pure path renames), `maestro-quality.toml` (delete
only the two filesystem ARC-005 exceptions that become stale at the crate root);
workspace manifest/lock and guide registration from the shared list.
**Requirements:** FR-S3-005, SC-S3-004.

- [ ] **Step 1: Red.** Run the existing filesystem/store/tokenizer tests as a
  baseline; change consumer imports to the intended shared crate and capture
  the missing-crate failure before moving implementation. Keep existing
  assertions and fixture bytes unchanged.
- [ ] **Step 2: Green.** Move the module and its tests to `maestro-filesystem`,
  adapting only imports, visibility and workspace wiring. Pure-rename the 17
  exclusions at `.cargo/mutants.toml:10, 12-25, 29, 32`; leave :62 (the kernel's
  `filesystem.rs`) unchanged. Preserve mutant identities/lines, reasons and
  scope; verify each renamed target exists. Delete both ARC-005 exceptions for
  `Directory` and `open_nofollow`: the new `src/lib.rs` crate root does not trigger
  the `mod.rs`-only rule, so renamed exceptions would be stale. Add no exclusion
  or exception. Move existing target-specific dependencies, not new libraries.
  Preserve store/tokenizer
  integration regressions and all ADR-0018 behavior; no catalog logic here.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-filesystem` and
  `~/.local/bin/capped cargo nextest run -p maestro-canonicalization`;
  compare test assertions/fixtures, the 17 path-only exclusions and both deleted
  ARC-005 exceptions. Show conventions exclusion checks, `rust-gate architecture`
  (no stale exception) and three-target Clippy passing.

**Acceptance:** one shared implementation, unchanged security behavior and
regression assertions; canonicalization no longer owns a private copy.

### C04 Shared owned-file operations [US1] (4 h)

**Phase:** P1/M3

**After:** C03, C04a.
**Files:** `crates/maestro-catalog/src/files/{mod.rs,plan.rs,apply.rs,remove.rs,recovery.rs}`,
`crates/maestro-catalog/src/files/tests/{mod.rs,crashes.rs,races.rs,removal.rs}`.
**Requirements:** FR-S3-005, SC-S3-004.

- [ ] **Step 1: Red.** Inject failure before/after each write and ownership
  commit; test traversal, links, ancestor swaps, racing creates, stale previews,
  hard-linked targets and user edits before recovery/removal.
- [ ] **Step 2: Green.** Build digest-bound plans, removal and crash recovery
  on C04a's held-handle operations; never duplicate platform safety code.
  Journal progress locally, publish ownership last and refuse changed files.
  Shared JSON entry semantics belong to C06, not this task.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog files::tests`;
  run the same cases on the three CI platforms and cross-target Clippy locally.

**Acceptance:** rerun is idempotent; every crash is recoverable or explicitly
refused with user bytes intact; no path escapes its root and removal never
claims another writer's file or entry.

### C17 Restrictive settings resolution [US3] (4 h)

**Phase:** P1/M3

**After:** C03; supervisor S1-to-S3 sync of the landed S1 settings
(`feat/s1-settings`) registry/resolver and required kernel commits, with the
integration head recorded before dispatch. A branch or uncommitted tree is not
this prerequisite; preserve landed S3 behavior under the 21:02 naming ruling.
**Files:** `crates/maestro-catalog/src/settings/{mod.rs,classes.rs,resolve.rs,tests.rs}`
(catalog descriptor/constraint adapters over S1, not another registry/resolver),
`crates/maestro-catalog/src/source/kinds/settings.rs` and C03's existing
known-settings port/call site; S1's landed descriptor module for missing 03 §1.6
keys (supervisor binds its exact path at dispatch; this is an S1-owned edit),
`tests/fixtures/catalog/settings/{classes.toml,overrides.toml}`.
**Requirements:** FR-S3-014, FR-S3-027, FR-S3-033, SC-S3-005.

- [ ] **Step 1: Red.** Test unknown/unclassified/doubly classified keys, all
  four precedence layers, explicit flags versus parser defaults, locked mutation,
  permission widening, dropped checks, larger budgets, secret literals and
  role-to-role leakage, each with a neighbour. Cover language/tone as free,
  updates as bounded and documented model_profile/routing_candidates overrides.
  Workspace auto over user propose stays propose; only user preferences can
  enable auto. Off dominates propose/auto; flags cannot raise the user/workspace
  ceiling. Budget-like keys take the minimum across every layer. Trust lives
  only in kernel authority, never preferences. Add a synthetic S1 descriptor:
  preset checking must recognize it and class validation must require exactly
  one class. Fail a stale key list or both overlapping spellings being accepted.
- [ ] **Step 2: Green.** Replace `KNOWN_SETTINGS` with S1 registry keys through
  C03's known-settings port, like `KnownRows`. Under **supervisor ruling, 21:02:
  S1 registry names are canonical**, use `ask.output_tokens`, not a second
  `max_output_tokens` key; add only missing 03 §1.6 catalog descriptors to S1.
  Extend those descriptors with checked catalog classes; reuse S1 resolution/
  provenance and journal, not new storage
  or parsing: explicit flags > workspace file > user-level config > built-in defaults.
  Apply update-consent ceilings and budget minima before free-value precedence,
  explaining ignored widenings. Presets are init seeds only; permissions
  intersect, checks accumulate and values stay role-local. Reject unsupported
  profile effort. The small resolver consumes typed layers, not storage or UI.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog settings::tests`.

**Acceptance:** every effective value has one class/source/requester; ordering
search results cannot change configuration or grant a permission.

### C05 Preview and apply project bootstrap [US1] (3 h)

**Phase:** P1/M3

**After:** C03, C04; CORE fixtures, not the owner-created seed.
**Files:**
`crates/maestro-catalog/src/bootstrap/{mod.rs,inspect.rs,compose.rs,project.rs,tests.rs}`,
`crates/maestro/src/cli/init.rs`, `crates/maestro/tests/it/catalog_init.rs`,
`tests/fixtures/catalog/bootstrap/presets/{knowledge-client.toml,rust-service.toml}`,
`tests/fixtures/catalog/bootstrap/bootstrap/{core.toml,rust.toml}`,
`tests/fixtures/catalog/bootstrap/bootstrap/core/.github/copilot-instructions.md`,
`tests/fixtures/catalog/bootstrap/bootstrap/rust/.maestro/recipes.json`,
`tests/fixtures/catalog/bootstrap/core/package.toml`,
`tests/fixtures/catalog/bootstrap/languages/rust/package.toml`;
these are target fixture paths, migrated in C36/C37, not a second charge to C05;
CLI registration files from the shared list.
**Requirements:** FR-S3-004, FR-S3-005, SC-S3-004.

- [ ] **Step 1: Red.** Test core and core-plus-Rust composed outputs, every
  dotfile, strict generated JSON, collisions, changed preview, rerun and
  interrupted apply. Plant a repository script that records any invocation.
- [ ] **Step 2: Green.** Inspect without running scripts; resolve explicit
  presets, preview by default, apply only with `--apply`, write the small
  descriptor and digest-bound authoring lock through C04. Keep recipes/workflows
  inert and name missing prerequisites rather than invoking installers.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog bootstrap::tests` and
  `~/.local/bin/capped cargo nextest run -p maestro catalog_init`.

**Acceptance:** both complete compositions validate, preview has zero writes,
no script marker exists, and authoring output cannot be mistaken for a verified
install. Descriptor fields cannot override authentication, policy or hooks.

### C05a Strict workspace and user preferences [US1, US3] (3 h)

**Phase:** P1/M3

**Status:** Closed; integrated `a5e0e96`, `fa1f65e`, `2843b5f`.
Historical acceptance below is retained. C44 and named v4 follow-ups carry
new behavior; no implementation step is reopened or charged again.

**After:** C05, C17, C37; S1 settings shared schema/parser integrated. D6's complete
BCP 47 subset needs no dependency or OA9.
**Files:** `crates/maestro-catalog/src/settings/preferences.rs` (init-plan adapter
consuming S1 descriptors/parser; no `preference_files.rs` parser copy),
`crates/maestro-catalog/src/settings/tests/preferences.rs`,
`crates/maestro/src/cli/init.rs`, `crates/maestro/tests/it/catalog_preferences.rs`,
`tests/fixtures/catalog/settings/{workspace.toml,user-preferences.toml}`;
necessary module/CLI registrations only.
**Requirements:** FR-S3-027, FR-S3-033, SC-S3-010.

- [x] **Step 1: Red.** Test full/partial files, absent language, canonical
  en/fr/es/ja, zh-Hant-TW, es-419 and sr-Latn; reject variants, extensions,
  private-use, grandfathered forms, malformed tags and non-ASCII input. Test all
  tones/update modes, duplicate/unknown keys, types, versions, exact/one-past
  file-size/depth bounds at small injected `Limits` values and forbidden
  access/identity/hooks/secrets. Both preference files reject trust, paths and
  receipts; workspace auto is an ignored widening. Preview/cancel create no
  files and changed config bytes survive rerun.
- [x] **Step 2: Green.** Extend S1's strict schema with catalog settings and
  adapt its file/parser API behind `WorkspacePreferences` for init planning.
  Both files hold preferences only. Reuse S1's exact bounded tag parser without
  a new library; pass only canonical tags onward.
  Add language, tone, narrowing update choices and documented overrides to
  init's side-effect-free draft; absence of language remains distinct from en.
  Do not enable config persistence before C05h/C05j supply the real trust guard;
  this task produces the validated C04 write plan, not an approval stub. Keep
  user authority config untouched; the draft contains no machine-local paths.
- [x] **Step 3: Check.** Run capped nextest filters `settings::tests::preferences`
  in maestro-catalog and `catalog_preferences` in maestro; parse the generated
  TOML and compare scripted and draft-plan bytes.

**Acceptance:** one strict schema/port and file adapter, no raw option escape
hatch or authority field. Init plans selected preferences at its own
root; C05j owns guarded persistence, never into an ancestor/user authority file.
New storage adapters do
not change configuration consumers.

### C05b Every-session discovery and precedence [US1, US3] (3 h)

**Phase:** P1/M3

**Status:** Closed; integrated `9a0852b`, `ab7e6b1`.
Historical acceptance below is retained. C44 and named v4 follow-ups carry
new behavior; no implementation step is reopened or charged again.

**After:** C05a, C17; S1 settings discovery/resolver integrated. Read existing
kernel journal records, not config claims.
**Files:** `crates/maestro-catalog/src/settings/{discovery.rs,resolve.rs}`
(S1 discovery/trust integration adapters only),
`crates/maestro-filesystem/src/{unix.rs,windows.rs}` (held-handle owner/write
metadata only, keeping ADR-0018's module layout after C04a),
`crates/maestro-catalog/src/settings/tests/discovery.rs`,
`crates/maestro/src/cli/{args.rs,run.rs}`, `crates/maestro/src/mcp/run.rs`,
`crates/maestro/tests/it/catalog_session_preferences.rs`.
**Requirements:** FR-S3-014, FR-S3-028, SC-S3-010.

- [x] **Step 1: Red.** Test home-bounded and explicitly trusted external-root
  discovery, nested configs, missing config, links and ancestor swaps. Plant
  valid and invalid foreign-owned/other-writable files, unreadable candidates,
  a /tmp parent and a /mnt/c-style parent, with safe user-owned neighbours.
  Skips warn, never parse or block startup; malformed selected safe files refuse.
  Outside home without journal trust reads no config. Cover four-layer conflicts,
  absent language, masked invalid keys, explicit/default flags, ignored ancestors,
  workspace auto over user propose, user off and budget-only narrowing. No grant
  reconciliation runs; explicit MCP --workspace uses these same checks.
- [x] **Step 2: Green.** Reuse S1's session discovery/resolver through C05a's port
  and C17, including provenance; add S3 trust integration and preserve its CLI/MCP
  startup wiring before effects. Reuse ADR-0018 reads with held-handle uid/mode or owner SID/DACL checks
  on directory and file; never select mount/drive roots. MCP without --workspace
  uses only user preferences, never cwd/roots. Preserve platform config-home
  resolution; snapshots do not watch files and tool arguments cannot replace them.
- [x] **Step 3: Check.** Run capped nextest filters `settings::tests::discovery`
  and `catalog_session_preferences`; restart the process after an edit and
  assert that only the new session sees the changed preference. Run planted-file
  and owner/write-check cases on all three CI hosts, not Unix-only mocks.

**Acceptance:** free values use explicit flags > workspace file > user-level
config > built-in defaults; update consent/budgets only narrow. No hidden preset
layer, ancestor merge or policy widening. CLI/MCP
consume the same port; replacing file storage does not change their resolution.

### C05h User-approved workspace trust records [US1, US3] (3 h)

**Phase:** P1/M3

**After:** C05a, C05b; existing kernel journal/scopes and C04 filesystem seam.
Trust administration itself never depends on successful workspace discovery.
**Files:** `crates/maestro-catalog/src/policy/workspace/{mod.rs,port.rs,approval.rs}`,
`crates/maestro-catalog/src/policy/workspace/tests/{mod.rs,approval.rs}`,
`crates/maestro/src/cli/trust.rs`, `crates/maestro/src/cli/init.rs`,
`crates/maestro/tests/it/catalog_workspace_trust.rs`.
**Requirements:** FR-S3-035, FR-S3-036, SC-S3-012.

- [ ] **Step 1: Red.** Test default-no terminal confirmation and fresh-home CI:
  `trust add DIR --confirm-path DIR` then scripted init writes its files. Missing
  non-terminal confirmation exits 2 with the exact command; mismatched canonical
  path, --yes, --json, environment, catalog and MCP text cannot approve. Refuse
  filesystem/drive/mount roots, HOME itself and internal directories. Adding or
  removing trust preserves edited preference bytes; list reads only kernel state.
  Decline cannot apply a template/projection; copied configs never grant trust.
- [ ] **Step 2: Green.** Store answers, canonical paths and receipts only in
  user-local kernel authority keyed by canonical path. Implement explicit
  `maestro trust add/list/remove`, never an MCP tool; no discovered config is
  read or rewritten by these commands. Add uses D11's terminal or exact
  --confirm-path contract; remove revokes subsequent controlled access. Implement
  the separately confirmed preferences-only decline write with internal metadata,
  without broad HOME grants, elevation or a new authorization database.
- [ ] **Step 3: Check.** Run capped nextest filters `policy::workspace::tests::approval`
  and `catalog_workspace_trust`; assert journal provenance and unchanged files
  after refused additions and successful trust changes beside an edited config.

**Acceptance:** `WorkspaceTrust` resolves journal-backed authority, not editable
text or process identity inferred from a repeated path. The default adapter is replaceable behind that port without changing
callers; every adapter must preserve the mandatory deny floor. Only a real user
approval can add a folder, and config-only decline records grant no other write.

### C05i Canonical path policy and secret deny data [US1, US3] (4 h)

**Phase:** P1/M3

**After:** C05h, C04a.
**Files:** `crates/maestro-catalog/src/policy/workspace/{paths.rs,deny.rs}`,
`crates/maestro-catalog/src/policy/workspace/tests/paths.rs`,
`crates/maestro-catalog/resources/secret-paths.json`.
**Requirements:** FR-S3-036, SC-S3-012.

- [ ] **Step 1: Red.** Test inside/outside reads and writes, nested trust roots,
  lookalike prefixes, `..`, symlink/reparse escapes, link swaps and new-file
  parents. Deny secret reads even inside trust: SSH/GPG, cloud/CLI credentials,
  password stores/keyrings and `.env`/`.env.*`. Exercise home/XDG/Windows path
  mappings and a known secret path's allowed non-secret neighbour.
- [ ] **Step 2: Green.** Implement the default `WorkspaceTrust` decision adapter
  over ADR-0018 held handles/canonical ancestry, not string prefixes or a second
  path implementation. Load the immutable built-in deny rules as strict data;
  workspace/catalog inputs cannot remove or replace them. Keep kernel-internal
  storage inaccessible to agents/tools even if a broader folder is trusted.
- [ ] **Step 3: Check.** Run capped nextest filter `policy::workspace::tests::paths`
  and three-target Clippy; run the real filesystem cases on all three CI hosts.

**Acceptance:** outside writes, secret reads and escapes fail before effects;
ordinary outside reads and non-secret trusted reads/writes remain eligible
under other controls. Adding a secret location changes checked data, not callers;
no adapter may weaken the mandatory policy floor.

### C05j Own-file-operation trust enforcement [US1, US2, US3] (4 h)

**Phase:** P1/M3

**After:** C05i, C04, C05.
**Files:** `crates/maestro-catalog/src/files/{plan.rs,apply.rs,remove.rs,recovery.rs}`,
`crates/maestro-catalog/src/files/tests/workspace_trust.rs`,
`crates/maestro/src/cli/init.rs`,
`crates/maestro/tests/it/catalog_trusted_files.rs`.
**Requirements:** FR-S3-005, FR-S3-027, FR-S3-035, FR-S3-036,
SC-S3-010, SC-S3-012.

- [ ] **Step 1: Red.** Prove trusted --apply writes expected config bytes at the
  displayed root, nested init leaves the ancestor's `.maestro/` byte-identical,
  and identical rerun writes nothing. Across en/fr/es/ja × all three tones,
  every generated workspace/host deliverable is byte-identical except config
  language/tone values. Check kernel/internal ownership metadata structurally
  for the exact corresponding config digest; the test's doc comment must name
  this boundary and forbid other deliverable exceptions. Receipts/logs normalize
  only timestamps and IDs. Drive apply/remove/recovery through outside-write, secret-read and swapped-path
  denials with zero writer calls and intact bytes. Test the narrow declined-trust
  config write and kernel-internal operations without granting agents those paths.
- [ ] **Step 2: Green.** Gate owned-file effects through `WorkspaceTrust` at the
  held-handle write/read boundary. Supply trusted local actor/operation facts,
  not model text. C06/C07/C16/C16d consume this same port for external host
  targets, asking once to trust an exact managed directory at first installation.
  Kernel-owned XDG config/data/state writes keep kernel rules, not agent grants.
- [ ] **Step 3: Check.** Run capped nextest filters `files::tests::workspace_trust`
  and `catalog_trusted_files`, then the existing race/crash/removal suite.

**Acceptance:** one controlled file-effect gate serves init, projection and
later install/update/rollback; adapters do not bypass path policy. No trust
preference, catalog or update widens roots. S3 does not claim containment of arbitrary
external host tools; transferred C20 and the other named S4 host obligations
supply live containment, not this S3 task.

### C05c Interface message port and English logs [US1, US3] (4 h)

**Phase:** P1/M3

**After:** C05b.
**Files:** `crates/maestro/src/presentation/{mod.rs,messages.rs,tests.rs}`,
`crates/maestro/src/cli/{output.rs,run.rs,init.rs}`,
`crates/maestro/tests/it/catalog_presentation.rs`.
**Requirements:** FR-S3-029, SC-S3-010.

- [ ] **Step 1: Red.** Exercise en/fr/es plus another accepted subset tag across three
  tones; deterministic messages have one wording per language, invariant under
  tone. Assert one English-interface note for other tags, no snapshot language
  change, English logs, stable JSON/status/command fields and complete warnings.
  --help and clap reference stay English; no translation network calls occur.
- [ ] **Step 2: Green.** Add deterministic built-in interface translations and
  fallback through the existing human output boundary, with data interpolation
  separate from message selection. Wire init's new prompts and the common output
  boundary only; C05l owns existing-message migration. Keep logs/receipts, code
  and documentation English and invariant under tone. No model call to render UI.
- [ ] **Step 3: Check.** Run capped nextest filter `catalog_presentation` plus
  the existing CLI contract suite; compare structured outputs byte-for-byte
  and inspect the no-color/plain snapshots.

**Acceptance:** presentation is one small module consuming validated preferences,
not a configuration or authorization engine. Future clients consume its typed
messages without changing language resolution or machine contracts.

### C05l Existing user-facing message migration [US1, US3] (4 h)

**Phase:** P1/M3

**After:** C05c.
**Files:** existing human-message/error call sites under `crates/maestro/src/cli/`,
`crates/maestro/src/presentation/{messages.rs,tests.rs}`,
`crates/maestro/tests/it/catalog_presentation.rs`;
no command/parser or machine-contract refactoring.
**Requirements:** FR-S3-029, SC-S3-010.

- [ ] **Step 1: Red.** Inventory existing user-facing messages/errors at the
  human output boundary. Add en/fr/es snapshots covering each message key,
  three-tone invariance and English help/log/JSON neighbours; fail a missing key.
- [ ] **Step 2: Green.** Route that inventory through C05c's typed message port.
  Keep one wording per language and interpolate data separately; no translated
  command names, help/reference text, machine values or operational logs.
- [ ] **Step 3: Check.** Run capped nextest `catalog_presentation` and the CLI
  contract suite; compare the inventory to migrated call sites and retain zero
  untranslated user-facing message keys, with help/reference exclusions named.

**Acceptance:** interactive messages/errors use the shared translations without
changing output semantics or touching clap's English documentation. The inventory
bounds this migration separately from the message port and the later renderer.

### C05d Ask language, tone and artifact boundary [US1, US3] (4 h)

**Phase:** P1/M3

**After:** C05b, C05c; S1 settings preference/answer integration and existing
answer ports integrated, not live calibration approval. Extend those inputs;
do not build another language/tone setting or redo already-landed S1 wiring.
**Files:** `crates/maestro-knowledge/src/answer/{types.rs,prompt.rs,generate.rs}`,
`crates/maestro-knowledge/src/answer/tests/{prompts.rs,prompt_text.rs}`,
`crates/maestro/src/knowledge/operations/ask/run.rs`,
`crates/maestro/src/cli/ask.rs`, `crates/maestro/src/mcp/server/operations.rs`,
`crates/maestro/tests/it/catalog_answer_preferences.rs`,
`crates/maestro/src/cli/eval/engine.rs`,
`crates/maestro/src/cli/eval/tests/{engine.rs,kernel_engine.rs}`,
`docs/architecture/02-retrieval-and-knowledge-graph.md`,
`specs/001-knowledge-kernel/spec.md`.
**Requirements:** FR-S3-029, SC-S3-010.

- [ ] **Step 1: Red.** Use controlled gateway responses to inspect actual trusted
  prompt inputs for accepted canonical tags (including one without UI translations)
  and three tones. Test no-layer French question fallback, explicit en over a
  French question, canonical `lang`, en/fr/es refusal text and English fallback.
  An unsupported detector records unchecked, never a false pass/refusal. Evaluation
  ignores conflicting user/workspace/flag language and tone on the real ask path.
  Keep English artifact instructions, citations/quotes and prompt identity;
  question text cannot override explicit language. Use the real evidence validator.
- [ ] **Step 2: Green.** Pass optional validated presentation inputs from CLI/MCP
  to the answer port. Update response_language and host-owned refusal text in
  generate.rs, carry the canonical tag in lang, and preserve question-language
  default and evaluation's pinned prompt/profile. Record checked/unchecked language
  honestly, retaining all evidence controls and token ceilings. Changed explicit
  prompts stay uncalibrated until measured; no catalog dependency in knowledge.
- [ ] **Step 3: Check.** Run capped nextest `answer::tests::prompts` and
  `answer::tests::prompt_text` in maestro-knowledge, plus
  `catalog_answer_preferences` in maestro; retain existing denial neighbours.

**Acceptance:** all generated conversation follows the selected tag/tone while
artifact content/logs stay English; no explicit tag means S1's question language.
The prompt port is independent of workspace
storage and client delivery; tests prove instruction/validation behavior, not
unmeasured multilingual model quality.

### C05e Preferences delivered to client and agent sessions [US1, US3] (4 h)

**Phase:** P1/M3

**After:** C05b, C05c, C05d.
**Files:** `crates/maestro-catalog/src/settings/{instructions.rs,tests/instructions.rs}`,
`crates/maestro-catalog/src/hosts/preferences.rs`,
`crates/maestro/src/mcp/preferences.rs`,
`crates/maestro/src/mcp/server/{handler.rs,tests.rs}`,
`crates/maestro/tests/it/{mcp_clients.rs,catalog_client_preferences.rs}`,
`docs/architecture/{06-roadmap.md,08-traceability.md}`,
`specs/003-catalog/research/session-preferences.md`.
**Requirements:** FR-S3-031, FR-S3-036, SC-S3-010.

- [ ] **Step 1: Red.** Assert the exact effective language/tone and fixed English
  artifact/log rules in initialization instructions observed by each of Pi,
  Claude Code, Codex and Copilot client fixtures. A stale value, missing fragment
  or model-supplied replacement must fail; cover the visible UI fallback note.
  Start in a nested workspace and an empty folder, with/without --workspace;
  report selected versus fallback source without paths. Supplied MCP roots never
  change S3's choice. Model instructions contain only canonical quoted tags.
- [ ] **Step 2: Green.** Build one English instruction fragment from validated
  session preferences, delivered by `ClientPreferencesDelivery`'s MCP adapter.
  Native projections reuse only its fixed English artifact/log rule plus the
  instruction to follow current MCP session language/tone, never stored values. Record the named S4 launch-adapter and non-Copilot trust-hook
  obligations, with their actual payload/effect acceptance tests, in 06/08 and
  the handoff note. Add no launcher, plugin or dynamic discovery mechanism.
- [ ] **Step 3: Check.** Run capped nextest filters `catalog_client_preferences`
  and `mcp_clients`; inspect each initialization payload. C08 supplies actual
  host evidence without calling fixture behavior host obedience.

**Acceptance:** a fifth client adds a delivery adapter, not branches in callers.
S3 proves construction/MCP delivery and prepares native adapter input; S4 must
prove launch/resume/delegation consume the fragment and its other host hooks
actually deny unsafe paths. No speculative S3 execution engine is introduced.

### C05g Plain init flow and script parity [US1, US3] (4 h)

**Phase:** P1/M3

**After:** C05a, C05b, C05h, C05i, C05j, C05c; no TUI or OA9 dependency.
**Files:** `crates/maestro/src/cli/init/{mod.rs,flow.rs,plain.rs}`
(move C05's adapter without duplicating its planner),
`crates/maestro/src/cli/init/tests/{mod.rs,flow.rs}`,
S1's existing config-command dispatch (no-argument editor entry point only),
`crates/maestro/tests/it/catalog_init_menu.rs`, `docs/how-to/catalog.md`.
**Requirements:** FR-S3-027, FR-S3-030, FR-S3-035, SC-S3-010, SC-S3-012.

- [ ] **Step 1: Red.** Test D6's five steps as sequential labelled prompts,
  language/tag/tone, narrowing update choice, trust yes/no, Back, errors and
  Ctrl-C/EOF. Assert flags/plain plan parity, zero preview/cancel writes, --yes
  without prompts and fresh-home explicit trust setup; --yes never grants trust.
  Inject a new S1 setting descriptor: init and no-argument `maestro config` must
  show it with current value, allowed values/range, one-line description and source
  layer without per-setting screen code. Test an allowed edit through S1's API/
  journal, locked/authority-only refusal, layer restrictions and zero cancel writes.
- [ ] **Step 2: Green.** Implement the shared flow port and plain adapter over
  one draft/validator/planner, enumerating every S1 registry descriptor rather
  than an editor key list. No-argument config opens this same editor using S1's
  existing layer selection and config operations, never another parser or journal.
  Init remains workspace-only; restricted entries explain why they are not editable.
  No raw mode, alternate screen or new library. Reuse localized messages and
  fallback note. --apply remains necessary for init;
  missing scripted approval gives the exact user trust command, not a prompt.
- [ ] **Step 3: Check.** Run capped nextest `catalog_init_menu` and flow tests on
  three hosts; retain a screen-reader/plain/no-color walkthrough and script parity.

**Acceptance:** the first owner loop and no-argument config have the same
registry-generated every-setting editor without waiting on TUI evidence.
C05k later replaces the renderer, not preferences, trust or owned-file planning.

### C06 Copilot projection and shared JSON ownership [US1] (4 h)

**Phase:** P1/M3

**After:** C01, C04, C05e, C05j; code uses CORE fixtures, not new live receipts.
**Files:** `crates/maestro-catalog/src/hosts/{mod.rs,copilot.rs,shared_json.rs}`,
`crates/maestro-catalog/src/hosts/tests/{mod.rs,copilot.rs,shared_json.rs}`,
`crates/maestro/src/cli/catalog/project.rs`,
`crates/maestro/tests/it/catalog_copilot.rs`.
**Requirements:** FR-S3-006, FR-S3-007, FR-S3-031, FR-S3-036,
SC-S3-001, SC-S3-004, SC-S3-010, SC-S3-012.

**Probe input:** reload inside a running session remains **not run**. Do not
promise in-session refresh from startup discovery evidence; obtain a separate
live receipt or preserve that limit in the adapter's diagnosis.

- [ ] **Step 1: Red.** In an isolated home, test native discovery, exact tools,
  same-name shadowing by declared name (C01: user silently beats project,
  including a renamed user file whose `name:` still matches),
  unrelated MCP entries, edited owned entries, rerun,
  drift and removal; a dry run must leave no files. Assert the native preference
  fragment is present, outside-trust writes refuse and a user-approved exact
  host folder permits only the owned non-secret effects. Project under
  en/fr/es/ja × three tones: every written file is byte-identical, contains only
  fixed English rules/MCP guidance and never embeds language/tone values.
- [ ] **Step 2: Green.** Render the frozen v1 shape qualified by C01 and
  explicit Maestro MCP registration. Own only inserted shared JSON entries;
  preserve unrelated entries, refuse edited owned entries and use C04's
  unchanged-file preconditions for preview/apply/remove/recovery. Record
  registered/observed/stale/failed separately. Deliver C05e's static rule through
  the native `ClientPreferencesDelivery` adapter; the current MCP session
  supplies effective preferences. Ask once to trust the exact external target
  through C05h before C05j permits writes. Never auto-trust HOME or grant secrets.
  Do not bypass enterprise policy.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro catalog_copilot`. C08/C28
  separately repeat C01's real parser/MCP call on the isolated projection under OA2.

**Acceptance:** fixture-backed idempotence, preference-fragment delivery and
owned-only removal; live discovery remains a C08/C28 gate. User MCP entries
survive and no missing tool/model is hidden.
The native delivery adapter shares the fragment/WorkspaceTrust ports, and tests
prove an outside-trust write is refused rather than silently granting a folder.

### C07 Pi projection [US1] (3 h)

**Phase:** P1/M3

**After:** C01, C04, C05e, C05j, C06; code uses CORE fixtures, not new live receipts.
**Files:** `crates/maestro-catalog/src/hosts/pi.rs`,
`crates/maestro-catalog/src/hosts/tests/pi.rs`,
`crates/maestro/tests/it/catalog_pi.rs`; register Pi in the existing projector.
Source fixtures use `core/agents/maestro.agent.md` and
`capabilities/practice/qa/agents/reviewer.agent.md` with owner-local sidecars.
C40 adds qualified-name mapping/collision probes; never read a type-first source
or flatten IDs before building its checked closure.
**Requirements:** FR-S3-006, FR-S3-007, FR-S3-031, FR-S3-036,
SC-S3-001, SC-S3-004, SC-S3-010, SC-S3-012.

**Probe input:** reload inside a running session remains **not run**. Do not
promise in-session refresh from startup discovery evidence; obtain a separate
live receipt or preserve that limit in the adapter's diagnosis.

- [ ] **Step 1: Red.** Test exact tools and skills, missing MCP adapter/provider,
  unsupported fields, restart discovery and unmeasured reload diagnosis,
  shadowing by declared name (C01: project
  silently beats user), edited content and removal in
  an isolated Pi home; prove tool names alone cannot pass the MCP-call check.
  Assert the shared fixed English/MCP-guidance fragment and path-policy port.
  Across en/fr/es/ja × three tones, all projected files are byte-identical and
  contain no stored language/tone values.
- [ ] **Step 2: Green.** Project only C01's supported profile with explicit
  provider loading; reuse C05e's static rule/native delivery port,
  C05j's controlled write gate, the installed adapter and C04. No extension install,
  silently dropped field or inherited/fallback model.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro catalog_pi`. C08/C28 own
  C01's real Pi MCP probe after restarting the projected host under OA2.

**Acceptance:** fixture provider/tool contracts pass; C08/C28 still require real
provider-backed calls. Preference instructions are delivered
through the shared port, missing support is named, and unrelated/edited resources
survive. Maestro's projection writes enforce trust; this does not claim Pi tool
containment, whose hook obligation remains S4.

### C08 First owner loop [US1] (2 h)

**Phase:** P1/M3

**After:** C02, C05a, C05b, C05c, C05d, C05e, C05g, C05h, C05i, C05j, C05l,
C06, C07, C40, C47b; integrated T034/T035 and completed T038 live evidence;
OA2/OA6 for real hosts or private data.
**Files:** `crates/maestro/tests/it/catalog_owner_loop.rs`,
`docs/how-to/catalog.md`, `docs/how-to/knowledge-mcp.md`;
private live receipts at the approved location, never in CORE.
**Requirements:** FR-S3-007, FR-S3-027–031, FR-S3-035, FR-S3-036,
SC-S3-001, SC-S3-004, SC-S3-010, SC-S3-012.

- [ ] **Step 1: Red.** Extend the T038 synthetic process flow to assert init,
  projection, actual search/get/ask, citation generation/span/digest, refusal,
  repeat and removal. Verify workspace language/tone/trust, four-client MCP
  preference delivery and English artifacts/logs, including a language without
  shipped interface strings. Add a separate no-grant kernel, not a second config
  mutating the allowed kernel's shared principal.
- [ ] **Step 2: Green.** Complete the documented owner loop with plain init in
  fresh pinned Copilot and Pi sessions. Confirm explicit --workspace registration
  and path-free source reporting; the guide must not suggest an empty cwd selects
  project preferences. Preserve C01's opposite silent shadow precedence; explain
  Claude Code's interactive approval for project `.mcp.json` versus connected
  local scope. Codex registration is observed, its live call not run: collect
  missing evidence separately, never infer a call from registration. Preserve
  unavailable/uncalibrated markers and private receipts; fix only integration defects within this flow. When C05k is available,
  repeat the walkthrough for OA9 visual acceptance, never gate this first loop on it.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro catalog_owner_loop` and the
  authorized live flow; record client/provider versions and all observed states.

**Acceptance:** first useful checkpoint, not M3. A client-like test alone is
insufficient. Re-estimate remaining work from the amended dependency-closure
hours below; the old 72-hour baseline is historical, not a remaining-work claim.
C31 reuses the landed model-card descriptor, but owner-approved C02a content
still does not gate this loop. Use C02's checked MCP base through C47b's consumer
wiring. C48's qualified graph/vector publication and G25 are not prerequisites.
External S1-settings/owner waits are excluded.

## Workstream 2: Menu, settings, policy and required content [US1, US3, US5]

### C05f Terminal dependency measurements [US1] (2 h)

**Phase:** P1/M3

**Status:** Closed; integrated `3a4e6e4`.
Historical acceptance below is retained. C44 and named v4 follow-ups carry
new behavior; no implementation step is reopened or charged again.

**After:** C05; independent of the first owner loop. OA9 approved ratatui +
crossterm on 2026-09-28; ADR-0020 measurements still precede adoption.
**Files:** `specs/003-catalog/research/interface.md`; scratch manifests only.
**Requirements:** FR-S3-030, SC-S3-010.

- [x] **Step 1: Red.** Define a terminal probe for keyboard/focus, resize,
  Ctrl-C cleanup, plain output and no-color behavior on the three platforms.
- [x] **Step 2: Green.** Measure minimum ratatui/crossterm features, added
  crates, duplicates, native links, licences and vet needs. Keep the prototype
  disposable; no parser work or production dependency adoption in this task.
- [x] **Step 3: Check.** Run the probe and capped `cargo tree -e features` /
  `cargo tree -d` on scratch manifests; record results and exact OA9 decision.

**Acceptance:** reproducible measurement; only the optional renderer waits for
adoption evidence. The plain flow and preference parser have no new dependency.

### C05k Branded terminal renderer [US1, US3] (4 h)

**Phase:** P1/M3

**After:** C05g, C05f, C05c; OA9 library approval recorded; visual acceptance joins M3, not the first owner loop

**Files:** `crates/maestro/src/cli/init/{terminal.rs,mod.rs}`,
`crates/maestro/src/cli/init/tests/terminal.rs`,
`crates/maestro/tests/it/catalog_init_menu.rs`, `docs/how-to/catalog.md`;
approved dependency/feature/vet registrations only.
**Requirements:** FR-S3-030, SC-S3-010.

- [ ] **Step 1: Red.** Add five-screen snapshots/key sequences, Tab/arrows,
  Back, invalid-field focus, resize, Ctrl-C/EOF, cleanup and no-color cases.
  Assert renderer/plain/script plan parity and zero unconfirmed writes. A new
  S1 descriptor appears in init and no-argument config with all four registry
  fields and authorized editing; locked/authority-only neighbours still refuse.
- [ ] **Step 2: Green.** Plug the approved ratatui renderer into C05g's shared
  registry-generated editor for init and no-argument config, with no per-setting
  screens. Apply D6's palette, hierarchy, focus and accessible fallback; never duplicate
  draft validation or file effects. Restore the terminal on every exit.
- [ ] **Step 3: Check.** Run terminal/menu tests on all three hosts, measure
  contrast and retain keyboard/plain/no-color walkthrough plus OA9 visual
  acceptance during the repeated C08 flow; screenshot-only proof is insufficient.

**Acceptance:** the owner's polished menu target is met before C28. A renderer
can be replaced without changing preferences, trust or planning; plain C08 does
not wait for this task or its library decision.

### C09 Dependency and trust measurements [US2, US3, US5] (3 h)

**Phase:** P1/M3

**After:** C00; D4 already approves the required libraries. Existing public
attested artifacts supply real verification fixtures; no publisher-setup gate.
**Files:** `specs/003-catalog/research/{dependencies.md,trust.md}`,
`docs/architecture/08-traceability.md`,
`tests/fixtures/catalog/trust/{valid.json,wrong-signer.json}`;
use scratch manifests only. C10/C19/C22b own DEP-001 exceptions and vet edits
when each measured dependency is adopted, never stale pre-adoption entries.
**Requirements:** FR-S3-008, FR-S3-010, FR-S3-011, FR-S3-016.

- [ ] **Step 1: Red.** Specify a real valid/wrong-signer attestation probe and
  explicit feature/licence/native-link checks. Demonstrate wrong identity is
  refused; a fabricated verifier response cannot satisfy the real probe.
- [ ] **Step 2: Green.** Measure `tar`, a selected JSON Schema 2020-12 crate
  and `cedar-policy` 4.13: minimum features, additions/duplicates/native links,
  licences/vet. The supervisor verifies ADR-0020 evidence, not a second owner
  approval. Freeze verifier/record contracts; recheck pinned `gh` login or
  `GH_TOKEN` requirements and calls per five-minute refresh against account
  limits. Own 08 §17's publisher/roots/rotation row: bind D2 to supplied OA4
  evidence, or record that specific evidence pending for C28.
- [ ] **Step 3: Check.** Capture `~/.local/bin/capped cargo tree -e features`
  and `~/.local/bin/capped cargo tree -d` from the measurement manifests, plus
  real `gh attestation verify` output using the exact argv in the trust note.

**Acceptance:** reproducible counts, approved minimal features and named
exception removal conditions; authentic/wrong-signer outcomes, authentication
requirements and measured polling budget are recorded. C09 closes the 08 §17
row when OA4 evidence arrives; its explicit pending-evidence handoff blocks
C28, never the library/verifier implementation that uses public fixtures.

### C19 Real Cedar checks [US3, US5] (4 h)

**Phase:** P1/M3

**Status:** Closed; integrated `94a4296`, `81a2c39`.
Historical acceptance below is retained. C44 and named v4 follow-ups carry
new behavior; no implementation step is reopened or charged again.

**After:** C03, C09's measured dependencies under the decided D4.
**Files:** `crates/maestro-catalog/src/policy/{mod.rs,check.rs,schema.rs}`,
`crates/maestro-catalog/src/policy/tests/{mod.rs,check.rs}`,
`crates/maestro/src/cli/policy.rs`, `crates/maestro/tests/it/catalog_policy.rs`,
`tests/fixtures/catalog/policy/{schema.json,rules.cedar,cases.json}`;
dependency/CLI registrations from the shared list, including adoption-time
`maestro-quality.toml`/`supply-chain/audits.toml` for measured Cedar requirements.
**Requirements:** FR-S3-016, SC-S3-005.

- [x] **Step 1: Red.** Write allow/deny fixtures before policies for default
  deny, destructive effects, protected paths, egress and MCP tools. Include
  schema errors, missing trusted facts, evaluation errors and zero discovered
  tests; a spy must see zero executor calls on refusal.
- [x] **Step 2: Green.** Wire the real Cedar evaluator/schema and policy
  check/test CLI. Inspect all diagnostics, deny on errors and keep approval-needed
  distinct from permission granted. Checking never executes the requested effect.
- [x] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog policy::tests` and
  `~/.local/bin/capped cargo nextest run -p maestro catalog_policy`.

**Acceptance:** every rule's stimulus reaches real Cedar; allowed neighbours
pass, denied/error neighbours deny, and model text cannot supply trusted facts.

### C22a Graph topology [US5] (3 h)

**Phase:** P1/M3

**After:** C03, C17, C19.
**Files:** `crates/maestro-catalog/src/graph/{mod.rs,types.rs,topology.rs}`,
`crates/maestro-catalog/src/graph/tests/{mod.rs,topology.rs}`,
`tests/fixtures/catalog/graphs/{topology-valid.md,topology-invalid.md}`.
**Requirements:** FR-S3-003, FR-S3-019, FR-S3-042, SC-S3-005.

- [ ] **Step 1: Red.** Cover missing/ineligible references, unreachable nodes,
  paths without terminals, unbounded cycles/maps/subgraph depth and impossible
  reviewer independence, including a mandatory reviewer omitted from closure.
  Reviewed-evidence members compile; placeholder/authored/retired members refuse,
  and compilation alone never creates route-eligible S4 qualification.
  Add `owner_first_example_references_accept`: copy architecture 03 §2.2's
  owner-relative workflow location and qualified agent/skill `requires` and
  references into the otherwise valid topology fixture. Pair it with
  `legacy_example_references_refuse`: move that fixture to a type-first root,
  replace an agent/skill ID with its basename or a source path, or remove its
  declared requirement. Each independent mutation must refuse with the source
  and offending reference, not fall back to a same-named resource.
- [ ] **Step 2: Green.** Build exact dependency closures and topology checks
  for rules 1, 2, 3, 6 and 9 of architecture 03 §2.3. Reject unknown constructs;
  do not build a scheduler, engine or general plugin graph framework.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog graph::tests::topology`.

**Acceptance:** each covered rule has a valid/invalid pair; bounded repair
loops and independent reviewers pass without treating source labels as evidence.

### C22b Graph contracts [US5] (4 h)

**Phase:** P1/M3

**After:** C22a, C19; C09's measured JSON Schema validator approval.
**Files:** `crates/maestro-catalog/src/graph/{contracts.rs,conditions.rs,state.rs}`,
`crates/maestro-catalog/src/graph/tests/contracts.rs`,
`tests/fixtures/catalog/graphs/{contracts-valid.md,contracts-invalid.md}`;
wire full checks into `source/check.rs`; adopting the measured schema validator
owns its `maestro-quality.toml`/`supply-chain/audits.toml` entries.
**Requirements:** FR-S3-016, FR-S3-019, FR-S3-042, SC-S3-005.

- [ ] **Step 1: Red.** Test condition types, exact router edges, tool-policy
  coverage, required sandbox, budgets, read-before-write and a successful path
  missing its output, plus valid neighbours and hostile condition text. Rule 10
  refuses a budget above a Bounded range in the shared S1 setting descriptor,
  never a catalog `settings/classes.toml` authority.
  Add `owner_first_example_references_accept` in this suite: copy architecture
  03 §2.2's qualified output/state contracts, policies and matching `requires`
  into the otherwise valid contract fixture. Pair it with
  `legacy_example_references_refuse`: independently restore
  `contracts/delivery.schema.json`, a basename contract/policy, or an undeclared
  qualified reference. Refuse each mutation; restoring the declared qualified
  IDs passes. The reference excerpt alone is not a complete valid workflow;
  the fixtures must still satisfy all twelve rules, including initial-state and
  successful-path output production.
- [ ] **Step 2: Green.** Implement rules 4, 5, 7, 8, 10, 11 and 12 with real
  JSON Schema validation. Accept only the specified small condition language;
  check all successful paths, not merely one convenient traversal.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog graph::tests` and
  the `catalog_check` process suite after connecting graph validation.

**Acceptance:** all twelve 03 §2.3 rules have a check and positive/negative
fixture; graph checking performs no effect and missing S4 capability stays
unsupported. A smaller language needs an approved 08 disposition, not omission.

### C21 Knowledge workflow and shared policies [US3, US5] (3 h)

**Phase:** P1/M3

**After:** C02, C17, C19, C22b.
**Files (MAN):**
`capabilities/orchestration/application-workflow/workflows/workload-question/workflow.md`,
`capabilities/orchestration/application-workflow/contracts/answer.{schema.json,maestro.toml}`,
`capabilities/orchestration/application-workflow/evals/scenarios/workload-question.{json,maestro.toml}`,
`capabilities/orchestration/application-workflow/package.toml`,
`core/profiles/models/maestro.toml`,
`standards/security/policies/{default-deny,destructive-operations,protected-paths}.{cedar,maestro.toml}`,
`standards/security/policies/{egress-deny-by-default,mcp-allowlist}.{cedar,maestro.toml}`,
`standards/security/policies/schema.{cedarschema.json,maestro.toml}`, `hooks/pre-tool/policy.toml`,
`standards/security/checks/policy-neighbours.{json,maestro.toml}`, `core/package.toml`,
`presets/knowledge-client.toml`, `settings/README.md`, `.github/CODEOWNERS`.
Discovery cards are metadata of exact resources, not authority. Universal policies are standard-owned, common hook subscriptions are inert,
and Maestro session profiles are core; the application workflow is optional and
requires core, never the reverse. No private collection ID enters this preset.
**Requirements:** FR-S3-001, FR-S3-003, FR-S3-016, FR-S3-018, FR-S3-019.

- [ ] **Step 1: Red.** Write missing knowledge-skill/answer-contract/metadata-pair
  cases and all five production policies' allowed/denied neighbours; require
  real checker and evaluator refusals before completing the content.
- [ ] **Step 2: Green.** Author `workload-question`, its answer contract and shared
  policies/hook. Declare Maestro fast/balanced/deep profiles for `copilot` and
  `llamacpp`;
  retain unsupported/unqualified status without S4 evidence. Reuse C02's skill,
  instructions and checked `core/backends/mcp/config.toml` binding through
  `package:core` rather than duplicating them or adding a retired `mcp:` edge.
- [ ] **Step 3: Check.** Run `"$MAESTRO_BIN" catalog check --catalog-dir "$MANIFESTS"`
  and `"$MAESTRO_BIN" policy test --catalog-dir "$MANIFESTS"` on production content.

**Acceptance:** the knowledge workflow's exact references, ownership/08 rows,
policy neighbours and graph checks pass; no invented qualification or vendor text.

### C21b Feature-delivery workflow [US3, US5] (3 h)

**Phase:** P1/M3

**After:** C21.
**Files (MAN):** owner root `core/`, with exact
relative files `package.toml`, `workflows/feature-delivery/workflow.md`,
`agents/{planner,worker,tester,reviewer}.agent.md`,
`contracts/{plan,patch,test-report,review,delivery}.{schema.json,maestro.toml}`,
`profiles/models/{planner,worker,tester,reviewer}.toml`,
`skills/{spec-compliance,security-review}/SKILL.md`,
`evals/scenarios/feature-delivery.{json,maestro.toml}`; global `presets/rust-service.toml`
and generated `.github/CODEOWNERS`.
C01's agent-sidecar decision requires owner-relative
`agents/{planner,worker,tester,reviewer}.maestro.toml`;
each agent's name equals its stem. The two `SKILL.md` files follow C01's
specification-backed `metadata:` format; a host warning reopens the ADR-0005
sidecar decision.
**Requirements:** FR-S3-001, FR-S3-003, FR-S3-018, FR-S3-019.

- [ ] **Step 1: Red.** Require refusal for a missing role/contract/metadata pair,
  removed mandatory reviewer, self-review, missing approval gate and invented role
  qualification, with a valid complete feature-delivery neighbour.
- [ ] **Step 2: Green.** Author the baseline required role definitions, two skills,
  contracts and declarative workflow; builder is a deterministic step. Declare
  fast/balanced/deep for `copilot` and `llamacpp`, marking absent S4 support
  unsupported.
  Require C21's standard policy IDs explicitly and preserve approval obligations.
  Framework declarations belong to mandatory core and every preset closure;
  knowledge-only native projection remains thin. C70 adapts recovered reviewer
  content into this same definition, never creates a duplicate persona.
- [ ] **Step 3: Check.** Run `"$MAESTRO_BIN" catalog check --catalog-dir "$MANIFESTS"`
  and the feature-delivery scenarios through its graph/contract checks.

**Acceptance:** both v1 workflows have exact complete closures, required
independent reviews and honest qualification states; no unused role is added.

## Workstream 3: Trusted distribution and explanation [US2, US3]

### C10 Deterministic bundle compiler [US2, US5] (3 h)

**Phase:** P1/M3

**After:** C03, C09, C22b, C39; compile CORE fixtures, not owner-published content.
**Files:** `crates/maestro-catalog/src/bundle/{mod.rs,manifest.rs,write.rs}`,
`crates/maestro-catalog/src/bundle/tests/{mod.rs,write.rs}`,
`crates/maestro/src/cli/catalog/compile.rs`,
`crates/maestro/tests/it/catalog_compile.rs`;
measured tar dependency registration and adoption-time `maestro-quality.toml`/
`supply-chain/audits.toml` entries from the shared list.
**Requirements:** FR-S3-008, FR-S3-037, SC-S3-002, SC-S3-013.

- [ ] **Step 1: Red.** Reorder inputs and change mtimes/permissions; require
  identical bytes. Change one definition and require a new digest. Include
  executable-looking templates/hooks/scripts that would leave a marker if run.
  With small injected `Limits`, test at/one-past every C11 archive limit:
  entry count (agent plus sidecar = two, plus manifest), per-entry and aggregate
  bytes, full tar stream including headers/padding/end blocks, manifest nesting.
  Source-valid oversized closures refuse before publishing any output bundle.
  Compile C03's synthetic kind with its descriptor only; preserve its data and
  references without a compiler branch, and refuse when its descriptor is absent.
- [ ] **Step 2: Green.** Compile checked sources into sorted tar with fixed
  metadata and `bundle.json`, exact closures, owners/maturity, source commit,
  policy digest, runtime/features/tool contracts and entry points. Consume C03's
  `Limits`, shared with C11, and bound manifest construction/output before
  allocation/publication. Never write a successful bundle the reader will refuse
  for count, sizes or nesting.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog bundle::tests::write`
  and the `catalog_compile` process suite; compare two produced bundle digests.

**Acceptance:** deterministic bytes, changed-input sensitivity and no script
marker; authoring and bundle schemas are separate and versioned.

### C11 Hostile bundle reader [US2] (4 h)

**Phase:** P1/M3

**After:** C10, C03a (model-card hook for the unsupported-role refusal).
**Files:** `crates/maestro-catalog/src/bundle/{read.rs,compatibility.rs}`,
`crates/maestro-catalog/src/bundle/tests/read.rs`.
**Requirements:** FR-S3-009, FR-S3-037, SC-S3-003, SC-S3-013, SC-S3-014.

- [ ] **Step 1: Red.** Build hostile archive fixtures for duplicate, traversal,
  link, device, undeclared and oversized entries, bad schema/digest, truncation,
  unknown features, incompatible runtime and unsupported tool-contract versions.
  Test exact/one-past entry bytes, stream/aggregate payload, entry count including
  the manifest and manifest nesting at small injected `Limits`; load C03's same
  synthetic kind via its descriptor and reject an absent/unsupported descriptor,
  without reader branches. C03 separately
  asserts the production constants. Keep one streamed full-size archive case
  at the production stream boundary, generated/read with a fixed-size buffer
  and no input-sized allocation. Oversized declared sizes refuse before allocation.
  Read a model-card bundle with an unsupported kernel role and require the same
  kernel unknown-role refusal as C03a, before registration is reachable.
- [ ] **Step 2: Green.** Revalidate the normalized schema and every entry under
  the same `Limits` as C10 before returning a verified shape. Do not extract arbitrary
  paths first or treat the author's successful compile as validation.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog bundle::tests::read`.

**Acceptance:** every invalid bundle is refused with no partial installation
or writes outside staging; compatibility fails before activation.

### C12 Scoped install records [US2] (4 h)

**Phase:** P1/M3

**After:** C11; migration numbers above every migration landed or reserved
on `main`, `feat/s1-integration`, `feat/s2-integration` and
`feat/s3-integration` and the deployment-modes track, assigned next free at
landing by the supervisor in coordination with the other slices.
**Files:** `crates/maestro-kernel/src/catalog/{mod.rs,records.rs,store.rs}`,
`crates/maestro-kernel/src/catalog/tests/{mod.rs,installs.rs,pins.rs}`,
`crates/maestro-kernel/migrations/NNNN_catalog.sql`,
`crates/maestro-kernel/src/store/migration.rs`,
`crates/maestro-catalog/src/install/{mod.rs,record.rs}`,
`crates/maestro-catalog/src/install/tests/{mod.rs,records.rs}`.
`NNNN` is the next free number assigned at landing, the only intentional path
binding awaiting integration coordination; do not edit an applied migration.
**Requirements:** FR-S3-012, FR-S3-037, SC-S3-003, SC-S3-013.

- [ ] **Step 1: Red.** Test scoped invisibility, reopen, concurrent installs,
  atomic activation/rollback, exact component closure, artifact pin accounting
  and kernel-table persistence; corrupt references must prevent activation.
  Persist/reopen C03's synthetic kind through the common records without a
  kind-specific column/table/migration. C16b alone owns backup-manifest and pin inclusion.
- [ ] **Step 2: Green.** Add the catalog schema/records using existing kernel
  artifacts, transactions and scopes. The record seam is internal; no unsigned
  or authoring CLI install is exposed while trust is unfinished.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-kernel catalog::tests` and
  `~/.local/bin/capped cargo nextest run -p maestro-catalog install::tests`.

**Acceptance:** install, components and pins switch together; the lock is not
an authorization source. Later trust/discovery tasks use this seam and receive
new migration numbers if they need additional schema.

### C13 Bound attestation verification [US2] (4 h)

**Phase:** P1/M3

**After:** C09, C11, C12; use C09's real public artifact/identity fixtures and
C12's kernel record seam, not OA4 live publisher provisioning.
**Files:** `crates/maestro-catalog/src/trust/{mod.rs,attestation.rs,verifier.rs}`,
`crates/maestro-catalog/src/trust/tests/{mod.rs,attestation.rs,verifier.rs}`,
`crates/maestro/src/cli/health/doctor.rs`,
`crates/maestro/src/cli/health/tests/catalog_verifier.rs`,
`crates/maestro-kernel/src/catalog/{authority.rs,tests/authority.rs}`,
`crates/maestro-kernel/migrations/NNNN_catalog_authority.sql`,
`crates/maestro-kernel/src/store/migration.rs`,
`docs/how-to/catalog.md`, `docs/standards/security.md`.
**Requirements:** FR-S3-010, FR-S3-013, SC-S3-003.

- [ ] **Step 1: Red.** Test byte tamper, wrong signer/repository/workflow/issuer/
  source, forged subject digest, missing verifier and nonzero exit; pair these
  with real valid signature evidence from C09. Independently test exact/one-past
  stdout/stderr bounds at small injected `Limits`, plus clock-controlled
  kill/reap at the configured deadline. C03 asserts the production 16 MiB/1 MiB/
  30 s constants once. Substitute
  gh after successful doctor and before a subsequent verification: refuse before
  launch. Config/model/catalog attempts to replace kernel roots/pins must refuse.
  Make doctor fail readiness on absent/wrong-version/wrong-digest `gh` or missing
  online authentication, without downloading a binary or starting a login.
- [ ] **Step 2: Green.** Store D2's separate publisher bindings and gh
  path/version/digest as kernel authority records, provisioned/rotated only by
  C13a's explicit local authority command with the owner's journalled approval;
  this task tests the record seam with synthetic approved records. Verify that
  executable's digest before every launch, with fixed argv and D2 bounds;
  never resolve a replacement from PATH. Kill/reap on timeout and bind claims
  to locally hashed bytes; exit zero alone proves nothing. Extend doctor with
  pin/auth and detectable excess-scope diagnostics. Document OA4's minimally
  scoped read-only credential and update CORE's SEC-006 rule-map holder.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog trust::tests` and
  C09's real verification commands on the approved positive/negative artifacts.

**Acceptance:** any missing or mismatched verification evidence refuses use;
subprocess doubles prove bounds only, not cryptographic authenticity.

### C13a Explicit publisher and verifier provisioning [US2] (3 h)

**Phase:** P1/M3

**After:** C13; synthetic authority fixtures need no OA4 live credentials.
**Files:** `crates/maestro/src/cli/catalog/authority.rs`,
`crates/maestro/tests/it/catalog_authority.rs`,
`crates/maestro-kernel/src/catalog/{authority.rs,tests/authority.rs}`,
`docs/how-to/catalog.md`; CLI/module registration from the shared list.
**Requirements:** FR-S3-010, FR-S3-013, SC-S3-003.

- [ ] **Step 1: Red.** From a fresh isolated home, provision distinct catalog/
  runtime bindings and a verified gh pin, then rotate them. Assert the exact
  stored values and journal receipt. Missing/mismatched `--confirm`, default-no
  or cancelled terminal confirmation, `--yes`, environment, workspace/catalog
  input and MCP attempts leave authority unchanged. Reject a noncanonical gh
  path, wrong SHA-256 or missing repository/workflow/issuer binding; interrupted
  approval/record commit cannot leave unreceipted authority. Repeating a stale
  confirmation after a concurrent rotation refuses.
- [ ] **Step 2: Green.** Add only `maestro catalog authority set` using the
  plan's exact flags/confirmation contract and C13's kernel records. Display
  both publishers, gh path/version/digest and the current authority revision;
  terminal confirmation defaults to no, non-terminal use requires exact
  `--confirm SHA256` of that proposal. Hash the executable before invoking it
  for its version. Journal approval and atomically replace roots/pin together,
  invalidating cached admissions when authority rotates. Bypass workspace
  preference loading like trust administration; never expose an MCP equivalent.
- [ ] **Step 3: Check.** Run capped nextest `catalog_authority` in maestro and
  `catalog::tests::authority` in maestro-kernel, including C13's refusal cases.
  Document fresh-home provisioning and rotation before doctor/install; no
  command downloads gh, starts login or obtains publisher authority itself.

**Acceptance:** an owner can provision and rotate actual roots/pin through a
reviewed, journalled command. Content, generic `--yes` and MCP never grant
this authority; transferred C20/S4 denies agent-shell invocation of the command
as well.

### C14 Shared freshness and revocation admission [US2, US4] (4 h)

**Phase:** P1/M3

**After:** C12, C13.
**Files:** `crates/maestro-catalog/src/trust/{records.rs,admission.rs,refresh.rs}`,
`crates/maestro-catalog/src/trust/tests/{freshness.rs,revocation.rs}`,
`crates/maestro-kernel/src/catalog/{trust.rs,tests/trust.rs}`,
`crates/maestro-kernel/migrations/NNNN_catalog_trust.sql`,
`crates/maestro-kernel/src/store/migration.rs` (next free at landing).
**Requirements:** FR-S3-011, FR-S3-022, SC-S3-003.

- [ ] **Step 1: Red.** Test five-minute refresh and refusal exactly at
  `min(signed expiry, issued_at + 24 hours)`, including a capped longer signed
  expiry and late download that cannot renew it; clock rollback, record/floor
  replay, entry and bundle revocation,
  interrupted refresh, expiry offline, mid-consult local revocation and cache
  bypass; every public consumer must use the same admission seam.
- [ ] **Step 2: Green.** Authenticate C09's full record set through C13, apply
  it atomically and keep monotonic trust state. Return a caller/snapshot-bound
  admission valid only for current trust, with remaining offline time.
- [ ] **Step 3: Check.** Run the `trust::tests` suites in maestro-catalog and
  `catalog::tests::trust` in maestro-kernel through capped nextest.

**Acceptance:** no stale record regains authority, no partial trust set is
visible, and a locally invalidated in-flight consult does not return a candidate.

### C18 Explicit lock and explanations [US3] (3 h)

**Phase:** P1/M3

**After:** C14, C17, C05b, C03a; S1 settings registry/config-explain API integrated.
**Files:** `crates/maestro-catalog/src/resolve/{mod.rs,lock.rs,explain.rs,tests.rs}`
(catalog lock/provenance extensions to S1's explanation API),
`crates/maestro/src/cli/catalog/explain.rs`,
`crates/maestro-catalog/src/bootstrap/project.rs`,
`crates/maestro/tests/it/catalog_explain.rs`.
**Requirements:** FR-S3-011, FR-S3-015, FR-S3-038, FR-S3-039,
SC-S3-005, SC-S3-014.

- [ ] **Step 1: Red.** Test precedence provenance, immutable session pins,
  explicit versus implicit update, component/runtime/host/model identity drift,
  authoring versus installed locks and absent S4 qualification/observations.
  Explain a revoked bundle and an expired bundle: each must refuse through C14,
  even with a previously cached explanation or a valid-looking old lock.
  Explain model-card declaration versus kernel registration/evaluation/selection
  and the card an ask would use now, separately from M059. The kernel stores no
  answers; do not invent an observed-answer lookup. Changing model_profile must
  leave kernel selections and the immutable card named by a prior answer unchanged.
- [ ] **Step 2: Green.** Extend S1's existing config explanation, never add a
  second config command/parser/journal. Pin declared identities including the exact
  kernel card digest distinct from M059's agent profile, each component's actual
  maturity and supported profiles; show reviewed-versus-qualified maturity in
  lock/preview/explain, plus class/source/requester and observed state. Explain
  the current kernel answerer lookup and the re-registration caveat. C16h owns
  exact lock-bound resource/closure lookup; this task owns only lock/explanation.
  Unsupported is not false, zero or qualified; installed explanations enter
  C14's shared admission and the lock cannot override trust.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog resolve::tests` and
  `~/.local/bin/capped cargo nextest run -p maestro catalog_explain`.

**Acceptance:** every effective value is explainable; updates never silently
change a pinned project and no S4 receipt is fabricated.

### C15 Release and trust-record workflow code [US2, US5] (3 h)

**Phase:** P1/M3

**After:** C02, C10, C13, C22b, C68; MAN workflow code needs no live publisher setup or compiler publication

**Files (MAN):** `.github/workflows/{check.yml,release.yml,trust.yml,trust-watch.yml}`,
`scripts/tests/{trust-workflow.sh,sbom.sh}`, `scripts/sbom.jq`,
`tests/fixtures/sbom/{bundle.json,expected.spdx.json}`,
`docs/{release.md,rotation.md,emergency-revocation.md}`, `README.md`,
`docs/standards/{engineering.md,security.md}` (maps established by C02).
**Requirements:** FR-S3-003, FR-S3-013, SC-S3-002, SC-S3-003.

- [ ] **Step 1: Red.** A runnable workflow contract check must fail for an
  absent six-hour schedule, missing hourly missed-run alert, refresh tied only
  to tags, unpinned compiler/checksum, PR signing permission, omitted catalog/
  policy tests or wrong publisher binding. Reject a missing SPDX JSON SBOM,
  absent asset checksum/attestation, omitted or extra component/relationship and
  mismatched component digest. Generate the SBOM from a pinned fixture closure
  and compare exact contents; mutate each case to prove refusal. Assert the
  release-assets contract from plan Contracts in the workflow fixtures: exact
  tag/asset names, checksum format and attestation subjects. Changing build time
  or input ordering must not change SBOM bytes; bad/missing protected-branch
  CODEOWNERS review must block publication.
- [ ] **Step 2: Green.** Write protected release/check/trust workflows with
  checksum-pinned release inputs and SHA-pinned actions; refuse absent/unverified
  compiler inputs, never invent a release digest. The catalog publisher re-issues
  attested records every six hours. Independent hourly trust-watch alerts the
  maintainer/backup after eight hours without a verified issue. Document minimal
  attestation permissions, auth, rotation and emergency revocation; no privileged
  PR checkout. Generate SPDX 2.3 JSON using checksum-pinned toolbelt jaq 3.1.1
  (verify the toolbelt lock's platform digest, never fabricate one). Derive
  `creationInfo.created` from the source commit's UTC timestamp and
  `documentNamespace` from the bundle digest as D2 defines; sort/serialize
  deterministically. Follow the release-assets contract for version/tag/names,
  SHA256SUMS and subjects; checksum/attest bundle/SBOM and document verification. Map SEC-011 as applicable in MAN before
  release work. Implement against fixture release metadata until publication.
- [ ] **Step 3: Check.** Run `bash scripts/tests/trust-workflow.sh`,
  `bash scripts/tests/sbom.sh`, `actionlint`
  and `zizmor --offline .github/workflows`. Save the exact owner-activation and
  live-verification checklist for C28; do not enable workflows or publish.

**Acceptance:** workflow code and schedule/refusal tests pass without live
publisher setup. C28, not C15, owns the real compiler/tag, periodic refresh,
missed-run alert, withdrawal and rotation proofs after OA4/OA5.

### C16 Verified install and update [US2] (4 h)

**Phase:** P1/M3

**After:** C12, C13, C13a, C14, C18, C05j, C03a; no MAN workflow input.
**Files:** `crates/maestro-catalog/src/install/{download.rs,activate.rs,update.rs}`,
`crates/maestro-catalog/src/install/tests/{updates.rs,recovery.rs}`,
`crates/maestro/src/cli/catalog/{install.rs,update.rs}`,
`crates/maestro/tests/it/catalog_install.rs`.
**Requirements:** FR-S3-009, FR-S3-010, FR-S3-011, FR-S3-012, FR-S3-037,
SC-S3-002, SC-S3-003, SC-S3-013, SC-S3-014.

- [ ] **Step 1: Red.** Interrupt download/verification/activation/lock update;
  require the previous valid install to survive. Test incompatible updates,
  expired offline cache, revoked rollback and attempted authoring promotion,
  using C09/C13's real verified fixture artifacts and controlled downloads.
  Test exact/one-past download bytes and clock-controlled deadline across
  redirects/retries, including absent/false Content-Length, with small injected
  `Limits`; C03 asserts production 256 MiB/60 s once. No partial activation.
  Download fixtures assert plan Contracts' release-assets contract: VERSION/tag,
  both payload names, exact SHA256SUMS grammar and both attestation subjects;
  missing/renamed/extra assets or wrong version mappings refuse.
  Install C03's synthetic kind using its descriptor and resolve its persisted
  resource through C12's scoped records, without an installer branch.
  An unsupported model-card role refuses through C11's kernel validation before
  activation; it never reaches registration. C16h owns the explicit CLI and
  SC-S3-014 registration/history tests, including A, then B, then A expecting B
  as a record of S1's known post-M1 selection gap, not new install behavior.
- [ ] **Step 2: Green.** Compose bounded download, C11/C13 validation, C14
  admission, C05j's path-policy port and C12 atomic records. Update the lock
  only by explicit command here and refuse unsigned installs; C16f later adds
  the approved pre-task auto policy without a second update mechanism. Do not wait for a live catalog release to implement these
  paths; C28 tests the owner-published release. Installation only stores resources;
  it neither registers nor selects models. C16h supplies the separate admitted
  registration CLI, never an install hook or MCP/source bypass.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro catalog_install` and
  `~/.local/bin/capped cargo nextest run -p maestro-catalog install::tests`.
  SC-S3-013's added-kind proof may change only descriptor/fixtures, never the
  production checker/compiler/reader/installer sources.

**Acceptance:** fixture installs/updates are atomic and authenticated; failure
cannot replace a valid install or revive a revoked snapshot. No source checkout
or compiler toolchain is needed by the implemented consumption path.

### C16h Explicit model-card registration [US2, US3, US5] (3 h)

**Phase:** P1/M3

**After:** C16, C18, C03a; CORE fixtures with all evidence already local,
not owner-approved MAN content or cross-machine import.
**Files:** `crates/maestro-catalog/src/resolve/lookup.rs`,
`crates/maestro-catalog/src/resolve/tests/lookup.rs`,
`crates/maestro/src/cli/catalog/register_model_card.rs`,
`crates/maestro/tests/it/catalog_model_cards.rs`, `docs/how-to/catalog.md`;
necessary module/CLI registrations only. C24a reuses this lookup.
**Requirements:** FR-S3-011, FR-S3-015, FR-S3-039, SC-S3-003, SC-S3-014.

- [ ] **Step 1: Red.** Refuse revoked/expired snapshots, lock/identity mismatch,
  absent local evidence and denied collection scope without registration or
  selection changes; a valid rerun is idempotent. Unsupported roles are already
  refused by C03a/C11/C16, not an unreachable CLI-only test. Spy on check,
  compile, install and update: zero model-registration/selection calls. Changing
  `model_profile` makes zero selection calls. Hold an ask's returned registry
  card ID, register a replacement and remove the catalog, then resolve that ID
  to the identical immutable card. Register A, then B, then A again for one
  router entry and expect a later ask to use B. Label this test as the known
  S1 post-M1 explicit-answerer-selection gap; its fix must change the expectation
  deliberately. Do not query nonexistent stored answers or add an answer journal.
- [ ] **Step 2: Green.** Add the exact lock-bound resource/closure lookup over
  C18's lock and C14's caller/snapshot admission. Wire
  `maestro catalog register-model-card ID --collection COLLECTION` to that lookup
  and C03a's existing kernel adapter. Return declaration/version/bundle identity
  and kernel card ID/digest; no source-directory or MCP registration mode. Require
  all referenced evidence already local via the kernel's pin checks; no import,
  download, model load, router mutation or evaluation/selection-record write.
  Document qualification per machine (backend, runtime digest and hardware),
  local-evidence-only M3, the post-M1 digest-matched `--evidence DIR` follow-up,
  and that re-registering an earlier card does not restore it for asks.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro catalog_model_cards` and
  `~/.local/bin/capped cargo nextest run -p maestro-catalog resolve::tests::lookup`;
  retain the actual returned IDs/digests and zero-call assertions.

**Acceptance:** explicit admitted registration reuses one kernel registry and
one exact lookup, without widening generic install/update behavior. Each answer
carries a registry card ID resolving to an immutable card; the kernel stores no
answers. The latest-registration caveat is tested honestly, not presented as
selection by manifest or rollback through re-registration.

### C16b Restore safety and installed-consumer admission [US1, US2] (3 h)

**Phase:** P1/M3

**After:** C16; C06/C07 only for their host-entry-point wiring.
**Files:** `crates/maestro-catalog/src/install/restore.rs`,
`crates/maestro-catalog/src/install/tests/restore.rs`,
`crates/maestro-catalog/src/bootstrap/project.rs`,
`crates/maestro-catalog/src/hosts/{copilot.rs,pi.rs}`,
`crates/maestro/src/cli/backup/{manifest.rs,restore.rs}`,
`crates/maestro/tests/it/{catalog_restore.rs,catalog_admission.rs}`,
`docs/how-to/catalog.md`.
**Requirements:** FR-S3-011, FR-S3-012, SC-S3-003.

- [ ] **Step 1: Red.** Restore an older backup with a revoked digest or lower
  floor; require refusal until fresh authenticated records arrive. Restore a
  pre-rotation backup: current roots and gh pin must survive byte-for-byte,
  with their current approval receipt; the rotated-out root cannot verify it.
  On a fresh destination with no current authority, refuse until C13a explicitly
  provisions it again. Exercise installed init and both projections with revoked/expired caches and require
  the same refusal, without mutating user files. After native text was loaded,
  the refusal/guide must say it cannot be retracted and require restart/removal.
- [ ] **Step 2: Green.** Back up catalog data/component artifact pins and
  historical authority receipts, but never restore roots or the gh executable
  pin over current authority. Preserve the destination's current roots/pin and
  their approval receipt; if absent, require C13a reprovisioning before use.
  Mark restored authenticated trust records unready and refresh against those
  current roots, not the backup's roots. Wire C14's shared
  admission at installed bootstrap/projection entry points; authoring mode
  remains explicitly separate and cannot produce verified install records.
- [ ] **Step 3: Check.** Run capped nextest filters `catalog_restore` and
  `catalog_admission` in maestro and `install::tests::restore` in maestro-catalog.

**Acceptance:** backup restores data, never old trust authority; every installed
init/project path uses admission and preserves user files on refusal.

### C16c Verified release discovery and proposals [US2, US3] (3 h)

**Phase:** P1/M3

**After:** C16b, C05a, C05b.
**Files:** `crates/maestro-catalog/src/install/{sources.rs,proposal.rs}`,
`crates/maestro-catalog/src/install/tests/proposals.rs`,
`crates/maestro/src/cli/update.rs`, `crates/maestro/tests/it/catalog_update_proposals.rs`.
**Requirements:** FR-S3-032, FR-S3-033, SC-S3-011.

- [ ] **Step 1: Red.** Test newer/unchanged releases, wrong publisher, unverified
  notes, incompatible closure and permission/hook diffs for runtime and catalog
  adapters. Propose mode must show old/new identity, changes and one exact apply
  command, with zero activation calls; unknown effects require approval.
- [ ] **Step 2: Green.** Implement `UpdateSource` with runtime/catalog adapters,
  registered by managed target rather than switches in session callers. Reuse
  C13/C14 verified records; catalog components move only with their pinned
  closure. Add thin check/apply CLI planning over C16, not another updater.
- [ ] **Step 3: Check.** Run capped nextest filters `install::tests::proposals`
  and `catalog_update_proposals`; run the same source-port contract against
  both adapters and refuse an unbound publisher.

**Acceptance:** only verified managed releases form proposals; no Cargo, host,
model, gh, Qdrant or router update. Adding a source requires an approved binding
and adapter/contract tests, not changes to discovery-policy callers.

### C16d Shared update receipts and safe rollback [US2, US3] (4 h)

**Phase:** P1/M3

**After:** C16c, C05j.
**Files:** `crates/maestro-catalog/src/install/{activate.rs,update.rs,rollback.rs,receipt.rs}`,
`crates/maestro-catalog/src/install/tests/{receipts.rs,rollback.rs}`,
`crates/maestro-kernel/src/catalog/{updates.rs,tests/updates.rs}`,
`crates/maestro/src/cli/catalog/update.rs`, `crates/maestro/src/cli/update.rs`;
next-free migration/registration only if existing records cannot represent it.
**Requirements:** FR-S3-034, FR-S3-036, SC-S3-010, SC-S3-011, SC-S3-012.

- [ ] **Step 1: Red.** Interrupt every receipt/pin/activation boundary; assert
  previous valid state or a recoverable pending operation, never an unreceipted
  active catalog update. Test rollback, revoked rollback, irreversible migration,
  stale approval, outside-trust targets and active-session denial with zero
  effects. Across en/fr/es/ja × three tones, receipts/logs are byte-identical
  after normalizing timestamps and IDs.
- [ ] **Step 2: Green.** Extend C16's transaction/recovery seam with retained
  prior artifacts/state, exact receipts and linked rollback. Revalidate trust
  and filesystem policy on both directions; require exclusive idle leases and
  reversible state before switching. Catalog update and generic update CLI
  share this lifecycle port; do not create a second authority store.
- [ ] **Step 3: Check.** Run capped nextest filters `install::tests::receipts`,
  `install::tests::rollback` and kernel `catalog::tests::updates`, including the
  existing C16 interruption and C16b restored-trust regressions.

**Acceptance:** every applied update has an English durable receipt and retained
rollback target, never authority to revive revocation. Lifecycle callers are
independent of release source and use the same trust-policy port; updates never
mint a folder-trust approval.

### C16e Verified runtime proposal contract [US2] (2 h)

**Phase:** P1/M3

**After:** C16c; OA4 runtime publisher bindings for live proof only.
**Files:** `crates/maestro-catalog/src/install/{runtime.rs,tests/runtime.rs}`.
**Requirements:** FR-S3-032, FR-S3-033, FR-S3-034, SC-S3-011.

- [ ] **Step 1: Red.** Exercise authentic/wrong-publisher runtime metadata,
  incompatible versions, missing installer contracts and auto requests. Require
  propose-only capability and zero staging, activation or rollback calls.
- [ ] **Step 2: Green.** Reuse C13/C14 and C16c's runtime source adapter for a
  verified release identity/compatibility proposal. Expose no runtime activation
  capability; retain distinct publisher bindings and bounded untrusted notes.
- [ ] **Step 3: Check.** Run capped nextest `install::tests::runtime` and the
  source-port contracts; wrong publisher and every apply attempt must refuse.

**Acceptance:** verified runtime proposals, not a new installer or cross-platform
handoff. Only catalog adapters expose the existing activation lifecycle in S3.

### C16g Runtime install-command presentation [US2] (2 h)

**Phase:** P1/M3

**After:** C16e.
**Files:** `crates/maestro/src/cli/update.rs`,
`crates/maestro/tests/it/runtime_update.rs`, `docs/how-to/catalog.md`,
`docs/architecture/{06-roadmap.md,08-traceability.md}`.
**Requirements:** FR-S3-032, FR-S3-033, SC-S3-011.

- [ ] **Step 1: Red.** With approved released-installer fixtures, assert the
  exact verified release plus install command; unsupported installation layouts
  report unsupported rather than inventing a command. Runtime apply/rollback and
  auto make zero executor calls; notes cannot become shell instructions.
- [ ] **Step 2: Green.** Render C16e's verified proposal and approved installer
  command as display data only. Record runtime auto-apply with the installer as
  a named follow-up: layout, startup handoff, receipts and trusted rollback.
- [ ] **Step 3: Check.** Run capped nextest `runtime_update` and document/link
  checks; no test invokes a package manager or replaces any executable.

**Acceptance:** one actionable command for supported released installations,
with no S3 runtime installation effects or implied cross-OS activation proof.

### C16f Startup policy, daily checks and mandatory consent [US1, US2, US3] (4 h)

**Phase:** P1/M3

**After:** C16c, C16d, C16e, C16g, C05b, C05h, C05j, C05e.
**Files:** `crates/maestro-catalog/src/install/{startup.rs,tests/startup.rs}`,
`crates/maestro/src/cli/{run.rs,update.rs}`,
`crates/maestro/src/mcp/run.rs`, `crates/maestro/tests/it/startup_updates.rs`,
`docs/how-to/catalog.md`.
**Requirements:** FR-S3-032, FR-S3-033, FR-S3-034, FR-S3-036,
SC-S3-011, SC-S3-012.

- [ ] **Step 1: Red.** With a fake clock and real lifecycle guards, test 24-hour
  throttling across simultaneous sessions, recorded offline failures, five-second
  timeout, off/propose/user-only catalog auto, revoked/expired updates and a
  safe current install. Off makes zero network calls for startup discovery;
  explicit update check still works. Runtime and MCP always make zero activation
  calls. MCP receives only a fixed ID/target/version notice, never release notes,
  paths or install commands. Widened permissions/hooks ask; --yes/model text never
  approve. Active tasks/sessions and kernel job leases forbid activation.
- [ ] **Step 2: Green.** Run bounded discovery/policy before admitting the first
  task/tool call unless off; use shared kernel check state and idle lease. CLI
  auto applies only a verified safe catalog diff; uncertainty/missing approval
  leaves a proposal. MCP only delivers a fixed notice through C05e's port. Trust additions always use C05h's user-only command, never an
  update's permission dialog. Preserve current trust expiry despite offline checks.
- [ ] **Step 3: Check.** Run capped nextest filter `startup_updates` and
  `install::tests::startup`; verify every apply/rollback is receipted, pending
  proposals survive restart and no background or mid-task activation occurs.

**Acceptance:** off, default propose and user-only catalog auto use one policy over
`UpdateSource` adapters. A new source/client does not change callers; independent
approval and path-policy ports keep updates from widening trust or permissions.

### C20 Transferred live Copilot policy hook [US1, US3] (4 h)

**Phase:** S4/C20

**After:** C06, C19, C05i, C05j, C62, C63; S4 runtime and trusted-host qualification, OA2 for actual approved live proof

**Files:** `crates/maestro-catalog/src/hosts/copilot.rs`,
`crates/maestro-catalog/src/policy/{native.rs,normalize.rs}`,
`crates/maestro-catalog/src/policy/tests/native.rs`,
`crates/maestro/tests/it/catalog_hook.rs`, `docs/how-to/catalog.md`.
**Requirements:** FR-S3-016, FR-S3-017, FR-S3-036, SC-S3-005, SC-S3-012.

- [ ] **Step 1: Red.** Create synthetic preToolUse event fixtures for C01's host
  pin (C01 did not capture a hook payload); C20 may capture one real payload
  under OA2 as its fixture source, without a new code-landing wait. Drive the
  fixtures through the real normalizer/Cedar guard: allow, deny, malformed request, unknown tool,
  opaque shell, missing fact, error and
  timeout. Try model-supplied identity/approval, outside-trust writes, secret
  reads even inside trust, link escapes and agent-shell trust administration,
  including trust-add with a correct --confirm-path and authority-set with a
  correct --confirm;
  denied cases record no effect, with ordinary allowed neighbours.
- [ ] **Step 2: Green.** Normalize only supported operations into C19's real
  evaluator and the same `WorkspaceTrust` port as Maestro-owned effects; load
  hooks only through the explicit projection. Report absent hook
  as unprotected; an approval-needed result cannot be manufactured into allow.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro catalog_hook`. Retain any
  OA2-authorized real-host probe separately. The S4 exit owns the live Copilot
  `preToolUse` proof: allow, deny, and hook error leading to deny; synthetic
  events alone cannot satisfy that S4 exit.

**Transfer:** 4 h removed from S3 and retained in S4; no project saving.

**Acceptance:** real policy denial, not merely hidden tools; errors/timeouts
fail closed when the hook is installed. Documentation makes no broker,
acceptance, journal or sandbox claim for native mode. Extending secret-path
rules changes checked data, not this hook; other clients' trust hooks stay the
named S4 qualification obligation.

## Workstream 4: Measured routing and exact impact [US4]

### C23 Reviewed intent labels [US4] (3 h)

**Phase:** P1/M3

**After:** C10, C14, C22b; independent review of every label arranged by the
supervisor. No MAN content/repository or publisher input.
**Files:** `tests/fixtures/catalog/routing/{routing.jsonl,review.json,split.json,eligibility.json,check.jq,digests.json,bundle.tar,README.md}`;
checked synthetic source under `tests/fixtures/catalog/routing/source/`:
`capabilities/practice/qa/workflows/{answer,change,review,test,build,docs,security,release,triage,migrate}/workflow.md`
and only the contracts/roles/policies these ten synthetic workflows reference.
**Requirements:** FR-S3-020, SC-S3-006.

- [ ] **Step 1: Red.** Define checks for fewer than 100 cases, duplicate IDs,
  missing alternatives/review, conflicting no-match/clarification labels,
  tuning/held-out overlap, fewer than 20 tuning/80 held-out/60 held-out matchable
  cases, fewer than ten eligible synthetic workflows, absent negative/adversarial/
  distractor cohorts or a missing/mismatched bundle/eligibility/input digest.
- [ ] **Step 2: Green.** Draft public/synthetic intents, independently review
  each, fix confirmed errors and freeze exact cohort sizes/splits. Check the
  synthetic sources with C22b, compile with C10 and vendor the resulting bundle
  plus `digests.json`; record the reproducible compile command. Pin suite,
  split, review, source, compiled bundle, eligibility and profile digests before
  scoring. Include only explicit
  test role/profile grants, never live qualification records. Include forged authority,
  revoked resources, contradictory skills, low-similarity required reviewers,
  stale indexes and failed embedders. Do not copy private questions.
- [ ] **Step 3: Check.** Run the fixture check through the installed toolbelt:
  from `tests/fixtures/catalog/routing`, run
  `jaq -e -s -f check.jq routing.jsonl`; verify the pinned input digests and
  compile twice to compare bytes. Check review/split IDs and workflow references
  against that exact compiled bundle/eligibility fixture; CORE needs no MAN
  checkout or network. C24 adds the workflow-ID parser and scorer.

**Acceptance:** at least 100 reviewed cases with alternatives, honest negative
labels and at least ten eligible synthetic workflow candidates; no outcome
is tuned on the held-out set. This fixture never grants live qualification.

### C24a Exact resolve/search and eligibility [US4] (3 h)

**Phase:** P1/M3

**After:** C14, C18, C16h, C22b, C23; no release or real-host prerequisite.
**Files:** `crates/maestro-catalog/src/resolve/{lookup.rs,search.rs}`,
`crates/maestro-catalog/src/route/{mod.rs,request.rs,eligibility.rs,result.rs}`,
`crates/maestro-catalog/src/route/tests/{mod.rs,eligibility.rs}`,
`crates/maestro/src/cli/catalog/{resolve.rs,search.rs}`,
`crates/maestro/src/mcp/catalog.rs`,
`crates/maestro/src/mcp/server/{handler.rs,operations.rs}`,
`crates/maestro/tests/it/catalog_resolve.rs`.
**Requirements:** FR-S3-011, FR-S3-021, FR-S3-022, SC-S3-006.

- [ ] **Step 1: Red.** Test exact IDs/closure, scope/revocation/qualification
  exclusion, caller-bound cache partitioning, expired snapshots and still-valid
  offline use. Synthetic eligibility must work only in the test harness; a real
  unqualified executable workflow returns `incompatible`.
- [ ] **Step 2: Green.** Implement shared pre-limit eligibility and bounded
  catalog browsing over C16h's exact lock-bound definition/closure lookup, not
  a second lookup. Add resolve/search
  to the existing CLI/MCP, binding caller, reasons and snapshot; no raw storage
  filter or separate server. Consume C14 records directly in fixture tests.
- [ ] **Step 3: Check.** Run capped nextest filters `route::tests::eligibility`
  in maestro-catalog and `catalog_resolve` in maestro, on C23's frozen snapshot.

**Acceptance:** exact authorized results, mandatory closure complete, no
synthetic qualification accepted by normal installed consumers.

### C24 Exact-ID and lexical baseline [US4] (4 h)

**Phase:** P1/M3

**After:** C14, C18, C22b, C23, C24a; no release-setup prerequisite.
**Files:** `crates/maestro-catalog/src/route/{lexical.rs,tests/lexical.rs}`,
`crates/maestro-catalog/src/eval/{mod.rs,intents.rs,score.rs}`,
`crates/maestro-catalog/src/eval/tests/{mod.rs,intents.rs,score.rs}`,
`crates/maestro/src/cli/catalog/route.rs`,
`crates/maestro/src/mcp/catalog.rs`,
`crates/maestro/tests/it/catalog_route.rs`.
**Requirements:** FR-S3-011, FR-S3-020, FR-S3-021, FR-S3-022, SC-S3-006.

- [ ] **Step 1: Red.** Hand-score workflow-ID alternatives/negative labels;
  test all five statuses, deterministic ties, low-similarity required reviewers,
  lexical limits and offline behavior. Hand-score D5's top-1/top-3, exact
  dependency completeness and unnecessary-context formulas. Refuse any changed
  C23 input digest or invalid cohort size before held-out scoring.
- [ ] **Step 2: Green.** Route exact IDs and structured lexical cards through
  C24a's eligibility/closure; add only the route CLI/MCP adapter. Report reasons,
  snapshot and explicit no-index state. Reuse S1 statistical methods, not its
  section-ID labels, for the workflow scorer.
- [ ] **Step 3: Check.** Run `route::tests` and `eval::tests` in maestro-catalog
  and `catalog_route` in maestro through capped nextest. Record baseline metrics
  on C23's pinned CORE bundle/eligibility fixture, with held-out top-1 and
  negative/distractor cohorts separate; record route p50/p95 latency.

**Acceptance:** held-out matchable top-1 ≥ 90 % (OA10 approved by the owner,
2026-09-28, amending D5) and exact closure completeness 100 %. Report top-3
as a diagnostic; D5's original top-3 ≥ 90 % bar is historical, not acceptance.
Scores are not probabilities. Real unqualified workflows stay `incompatible`;
no failed or empty-denominator comparison is called a passing baseline.

### C25 Rebuildable catalog cards [US4] (3 h)

**Phase:** P1/M3

**After:** C10, C12, C14; verified CORE fixture records, not release setup.
**Files:** `crates/maestro-catalog/src/discovery/{mod.rs,cards.rs,publish.rs,tests.rs}`,
`crates/maestro-kernel/src/catalog/{discovery.rs,tests/discovery.rs}`,
`crates/maestro-kernel/migrations/NNNN_catalog_discovery.sql`,
`crates/maestro-kernel/src/store/migration.rs` (next free at landing),
`crates/maestro-knowledge/tests/it/catalog_discovery.rs`.
**Requirements:** FR-S3-023, SC-S3-007.

- [ ] **Step 1: Red.** Test one card per workflow/agent/skill, canonical IDs,
  digest/snapshot/owner/maturity, exact rebuild equality, stale generations,
  partial publication and catalog-to-knowledge scope leakage. All card/type
  assertions stay in catalog `discovery::tests`. The knowledge integration test
  uses only knowledge types for collection separation and scope isolation:
  no dev-dependency on maestro-catalog and no dependency cycle.
- [ ] **Step 2: Green.** Feed cards through the S1 pipeline in a separate scoped
  catalog collection, one generation per bundle; record the binding only after
  verification. Keep exact definitions and dependencies in bundle authority.
- [ ] **Step 3: Check.** Run `discovery::tests` in maestro-catalog and
  `catalog_discovery` in maestro-knowledge through capped nextest with the
  existing pinned Qdrant fixture; rebuild and compare IDs/digests/results.

**Acceptance:** discovery is replaceable without changing definitions or
scopes; incomplete/stale generations never masquerade as the current snapshot.

### C26 Hybrid comparison with pre-limit eligibility [US4] (4 h)

**Phase:** P1/M3

**After:** C23, C24, C25; OA6 only if a model/provider needs new access.
**Files:** `crates/maestro-catalog/src/route/{hybrid.rs,tests/hybrid.rs}`,
`crates/maestro-catalog/src/eval/{compare.rs,tests/compare.rs}`,
`crates/maestro-knowledge/src/eval/{mod.rs,bootstrap.rs}` and
`crates/maestro-knowledge/src/eval/tests/{mod.rs,intervals.rs,compare.rs}` (shared
entry-point and unchanged-S1 regressions beside the existing interval tests),
`crates/maestro-knowledge/src/search/{filter.rs,request.rs}`,
`crates/maestro-knowledge/src/search/routes/{dense.rs,lexical.rs,identifier.rs,structured.rs}`
and `graph.rs` if S2's R4 has landed,
`crates/maestro-knowledge/src/search/tests/catalog_eligibility.rs`,
`crates/maestro-knowledge/tests/it/qdrant_projection/catalog_eligibility.rs`,
`specs/003-catalog/research/routing.md`.
**Requirements:** FR-S3-022, FR-S3-024, SC-S3-006, SC-S3-007.

- [ ] **Step 1: Red.** Put ineligible high-scoring IDs ahead of eligible IDs
  beyond each branch's top-k; require eligible results without post-filter
  starvation. Cover every retrieval branch present at landing, including S2 R4
  graph, or prove catalog queries cannot reach it; serialize with S2 G12/G14.
  Test empty eligibility, stale index, failed embedder and cache invalidation.
  Hand-check paired top-1 differences, seeded interval repeatability and refusal
  when the lower bound is zero/negative or any C23 input/split digest changed.
  Pin an existing S1 report/interval before exposing the shared entry point;
  require identical S1 results afterward.
- [ ] **Step 2: Green.** Carry eligible IDs into every contributing S1 branch
  before limits, preserving existing knowledge semantics; compare dense/BM25/
  fusion with C24's baseline on the same held-out fixture/profiles. Apply D5's
  top-1 paired bootstrap (95 %, 2,000 resamples, seed 42). Expose a narrow
  paired-difference entry point from S1's `eval/bootstrap.rs` via `eval/mod.rs`,
  reusing its generator, resampling and nearest-rank percentiles without copying
  them or changing S1's 2,000-resample default/results. Tuning cases never
  enter this comparison. Keep lexical fallback and S1 free-room admission;
  never unload a chat model.
- [ ] **Step 3: Check.** Run capped nextest filters `catalog_eligibility`,
  `hybrid` and `compare` in their respective crates, plus maestro-knowledge's
  `eval::tests` regressions; record paired intervals,
  top-1/top-3, no-match, clarification, exact closure/unnecessary-context metrics,
  distractor results and route p50/p95 latency, with all input/cohort identities.

**Acceptance:** enable hybrid only when held-out matchable top-1 ≥ 90 %
(OA10 approved by the owner, 2026-09-28, amending D5), the paired top-1 gain
interval's lower bound is strictly positive, and exact closure completeness is
100 %; otherwise ship the passing baseline. D5's original top-3 ≥ 90 % bar
is historical; top-3 remains a reported diagnostic. A failed comparison is
retained; no benchmark result is invented to enable hybrid.

### C27a Scoped catalog dependency projection [US4] (4 h)

**Phase:** P1/M3

**After:** C12, C14; S2 G25 qualification and S2 G27 public typed-edge port at
`crates/maestro-knowledge/src/graph/projection/port.rs` integrated.
**Files:** `crates/maestro-catalog/src/impact/{mod.rs,edges.rs,projection.rs}`,
`crates/maestro-catalog/src/impact/tests/{mod.rs,projection.rs}`,
`docs/architecture/08-traceability.md`.
**Requirements:** FR-S3-025, SC-S3-007.

- [ ] **Step 1: Red.** Project C12's exact scoped component edges into a
  disposable qualified S2 backend. Reject cross-scope writes/reads, wrong bundle
  snapshots and knowledge-claim namespace mixing; require exact ID/edge equality
  after deleting/rebuilding only the catalog projection.
- [ ] **Step 2: Green.** Add the catalog edge schema and write/read adapters
  over G27's application-ID-only operations. Bind scopes and bundle snapshot,
  publish complete projections only and rebuild from C12 authority. Do not
  turn catalog edges into evidence-span claims or create a second graph store.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog impact::tests::projection`
  against the qualified S2 fixture; record exact rebuild and isolation evidence.

**Acceptance:** S3 owns a real scoped, rebuildable catalog projection through
S2's public port. Missing G25/G27 blocks this task and impact/M3 explicitly;
C00/08 name both slice owners, never an assumed S2 catalog deliverable.

### C27 Exact catalog impact [US4] (3 h)

**Phase:** P1/M3

**After:** C22b, C24, C27a.
**Files:** `crates/maestro-catalog/src/impact/{traverse.rs,tests/traverse.rs}`,
`crates/maestro/src/cli/catalog/impact.rs`,
`crates/maestro/src/mcp/catalog.rs`,
`crates/maestro/tests/it/catalog_impact.rs`,
`docs/architecture/08-traceability.md`.
**Requirements:** FR-S3-011, FR-S3-025, SC-S3-007.

- [ ] **Step 1: Red.** Test direct/transitive skill, policy and contract impact,
  cycles already admitted by the graph contract, unauthorized nodes, snapshot
  mismatch and identical results after rebuilding C27a. A revoked bundle or
  expired bundle must refuse impact through C14, including cached traversal.
- [ ] **Step 2: Green.** Traverse C27a's exact catalog dependencies through
  its scoped read adapter after shared trust admission. Return snapshot-bound
  transitive results; never infer edges from similarity or bypass missing S2
  qualification with an unapproved closure fallback.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro catalog_impact` and the
  `impact::tests` suite against the qualified S2 rebuild fixture.

**Acceptance:** exact scoped results with snapshot evidence. S2 absence blocks
this M3 exit; a bounded in-memory fallback requires its own explicit approval.

## Workstream 5: M3 exit and approved recovery [US1–US5]

### C28 Tagged M3 proof [US1, US2, US3, US4, US5] (3 h)

**Phase:** P1/M3

**After:** C00, C01, C03, C03a, C02, C02a, C04a, C04, C17, C05, C05a, C05b, C05h, C05i, C05j, C05c, C05l, C05d, C05e, C05g, C06, C07, C08, C05f, C05k, C09, C19, C22a, C22b, C21, C21b, C10, C11, C12, C13, C13a, C14, C18, C15, C16, C16h, C16b, C16c, C16d, C16e, C16g, C16f, C23, C24a, C24, C25, C26, C27a, C27, C29, C38, C30, C31, C32, C33, C34, C35, C36, C37, C39, C40, C44, C45a, C45b, C46, C47a, C47b, C48, C49a, C49b, C50, C51a, C52a, C52b, C53a, C54, C55, C60, C62, C63, C64, C65, C66, C68, C70, C72, C76a, C76b, C79a, C79b, C80a, C80b, C81a, C81b, C82a, C82b, C84, C90, C91, C92, C93, C94; integrated T034/T035 and T038 live evidence, M1 release, qualified S2, OA4/OA5/OA7/OA9 actions and final CI; no P2/S6/S4 gate

**Files:** `scripts/tests/catalog-m3.sh`,
`docs/how-to/catalog.md`, `docs/how-to/knowledge-mcp.md`,
`docs/architecture/08-traceability.md`,
`specs/003-catalog/traceability.json`,
`specs/003-catalog/research/m3-evidence.md`, `docs/standards/security.md`;
verify MAN's rule map without editing another lane's checkout. Private receipts
remain private.
**Requirements:** FR-S3-017, FR-S3-026, FR-S3-051, FR-S3-056, FR-S3-057, FR-S3-058, FR-S3-061, FR-S3-064, FR-S3-065, SC-S3-001, SC-S3-002, SC-S3-003, SC-S3-004, SC-S3-005, SC-S3-006, SC-S3-007, SC-S3-008, SC-S3-009, SC-S3-010, SC-S3-011, SC-S3-012, SC-S3-013, SC-S3-014, SC-S3-015, SC-S3-016, SC-S3-018, SC-S3-019, SC-S3-021, SC-S3-022, FR-S3-068, SC-S3-025.

- [ ] **Step 1: Red.** Write the shell acceptance harness; fail its preflight
  if `command -v cargo`, `command -v rustc` or `command -v python3` succeeds,
  a source checkout is present or doctor rejects the released `maestro`/pinned
  `gh`. It must also fail a missing/mismatched SPDX JSON SBOM/closure, per-asset
  checksum or attestation, absent verification instructions, any exit/key/client evidence,
  M1 release, 08 §17 publisher closure, missed trust-refresh alert or current CI.
  Require all Phase 1 kind/config valid/refusal fixtures, deterministic schema/
  index/ownership generation, mandatory standards, eight language declarations
  and verified delegation, package/backend transactions and gap G01/G07–G11
  evidence. Require C29's complete recovery/defer ledger; no missing case passes.
  S3 ten-point hook maps make no live protection claim; C20 is S4. Require the descriptor-only
  lifecycle proof, C16h's card canonical-digest/returned-ID/refusal suite and
  A/B/A known-gap test, C03a/C11/C16 unknown-role refusals and owner-approved
  manifest winner changes. Each machine needs its qualified card and local
  evidence; detached install is not a cross-machine evidence-import proof.
  A model card is not S4 agent qualification.
- [ ] **Step 2: Green.** Copy only the script into OA5's disposable clean WSL
  user/container with released binaries, basic shell utilities and approved
  read authentication. Provision roots and the gh pin with C13a's exact
  confirmation before doctor/install. Run install → init → projection-file
  generation → update → remove without host execution. Separately run the live
  host stage in OA2's approved homes with pinned Copilot CLI, Pi/existing adapter,
  Codex and Claude Code; collect real load/MCP receipts there, without claiming
  toolchain absence. Record ten-point host states without launching a hook. C20 live effects are
  S4 receipts, never an M3 gate or claimed result here.
  Verify OA4/OA5 release, six-hour re-issue, missed-run alert, withdrawal and
  rotation evidence; finish C09's owned publisher-row evidence handoff. Record
  expiry/revocation/offline/restore cases, C23's synthetic suite/eligibility
  digests and both top-1/top-3 metrics. Verify held-out matchable top-1 ≥ 90 %
  against OA10's approved amendment (owner, 2026-09-28) before final acceptance;
  top-3 cannot substitute for the first-selection bar.
  Include real installed `incompatible` ("not qualified until
  S4") results. Include preference/trust/client-delivery receipts and verified
  startup off/propose/catalog-auto/consent/rollback/offline evidence, runtime
  propose-only and MCP zero activation. Include C05k/OA9 visual acceptance;
  no mid-task effects or claims of runtime installer qualification.
  Verify bundle/SBOM checksums, attestations and exact pinned closure against
  release instructions; record CORE/MAN SEC-011 rule-map evidence. Update only
  delivered portions of existing traceability rows; preserve S4/follow-up
  obligations. Document outcomes in `docs/how-to/catalog.md`.
- [ ] **Step 3: Check.** Run `bash catalog-m3.sh` inside that clean environment,
  not through cargo. In the checkout run offline links and C00's traceability
  suite. Quote final CI platform/test counts, coverage, killed/missed/timeout
  totals and commit/suite/eligibility digests in the evidence note.

**URL-rule amendment (+0 h):** Require C66 typed source-rule/signed-review
fixtures and published schema drift evidence; N07 type synchronization must be
complete. No live S6 catalog adapter or private-package publication is an M3 gate.

**Acceptance:** every M3 criterion has current observed evidence; no skipped
live test or pending owner/S2/CI action is called passed. Only the owner accepts
M3 and authorizes publication; lanes neither release nor integrate themselves.

### C29 Pre-M3 legacy disposition and provenance audit [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C68, C70, C72, C76b, C79b, C82b; owner-approved legacy access, no private collection access.
**Files:** specs/003-catalog/research/{comparison.md,comparison-inventory.json}; MAN docs/catalog/migration.md.
**Requirements:** FR-S3-001, FR-S3-026, SC-S3-009.
**Named tests:** `legacy_inventory_has_395_unique_dispositions`, `legacy_deferred_row_names_phase2_task`, `legacy_provenance_and_attribution_are_complete`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Audit the authorized manifest/template inventory exactly once by old relative path, SHA-256, attribution, destination and keep/convert/drop/defer reason. Verify Phase 1 recovered/imported content and record C71/C73/C74/C75/C77/C52c/C78 deferrals explicitly. Do not copy content in this audit or reopen landed work.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Exactly 395 source files have one disposition; every recovery is a named bounded task. No C28 dependency, no claim that deferred content shipped, no loss of obligations or attribution.

## Owner-first migration (2026-09-30)

**Historical budget retained, contract amended by C44:** C30–C39 are the
**24–34 h S3 increment plus 1 h** approved for core-workflow-label segregation
in C33: **25–35 h**. C40 is the **2–4 h native increment**, and C41–C43 the
**6–10 h deferred S6 contract**. The combined increment is **33–49 h**, versus
the proposal's original 32–48 h. These rows are already in the pre-v4 whole-plan
baseline; do not add their hours again to C44–C96b.
No content seed is published until C39 passes. C41–C43 never gate C02/C08/M3;
unsupported nonempty collection content refuses until that later work lands.
QA is synthetic demonstration content only. All tests named here are required
future cases, not claims that the current `/1` implementation already has them.

### C38 Owner-first architecture, plan and README [US1, US5] (3–4 h)

**Phase:** P1/M3

**Status:** Closed; integrated `cd88a17`, `b39ce3c`.
Historical acceptance below is retained. C44 and named v4 follow-ups carry
new behavior; no implementation step is reopened or charged again.

**After:** C00; approved owner-first proposal and supervisor segregation rulings.
**Files:** `docs/architecture/03-agent-orchestration.md`,
`specs/003-catalog/{spec.md,plan.md,tasks.md,traceability.json}` (the inventory's
85 keys/six exclusions stay fixed); separately MAN `README.md` only.
**Requirements:** FR-S3-040–048, SC-S3-009.

- [x] **Step 1: Red.** Check the existing source/output markers, owner paths,
  six segregation requirements and task coverage against the proposal. Retain
  the missing `/2`/migration-task failure and the exact 57-task/191-hour baseline.
- [x] **Step 2: Green.** Amend architecture 03 §1.1/§1.2, D13, C02's exact
  file list and affected C02a/C05/C07/C21/C21b paths. Document `maestro-source/2`,
  `maestro-cli/catalog-check/2`, `maestro-project/2` and
  `maestro-authoring-lock/2`. Keep delivery separate from core; name the sole
  inventory-name selector exception and the deferred S6 contract. Replace the
  MAN README with the tree, root boundaries, QA/MCP authoring steps and private
  mount limits; create no resource, vendor content or owner identity.
- [x] **Step 3: Check.** Run rumdl, offline lychee, the `catalog_traceability`
  conventions suite and an exact FR/SC-to-task/budget/DAG recomputation. Commit
  CORE normally and push only its lane branch; commit/push MAN and open its
  README-only PR against main. Record commands, exits, hashes and hour deltas.

**Superseded contract:** C44 replaces the earlier tree and core segregation;
these closed steps record historical work, not current v4 requirements.

**Acceptance:** six falsifiable segregation rules and complete mappings; no
source/runtime implementation or private-data read, and no delivered claim.

### C30 Bounded v4 area discovery [US1, US5] (3–4 h)

**Phase:** P1/M3
**After:** C38, C44, C03.
**Files:** crates/maestro-catalog/src/source/{descriptor.rs,registry.rs,walk.rs}; crates/maestro-catalog/src/source/tests/{directory.rs,layout.rs,bounds.rs,registry.rs}.
**Requirements:** FR-S3-040, FR-S3-046, FR-S3-049, SC-S3-015.
**Named tests:** `v4_area_placement_accepts`, `nested_or_unknown_area_refuses`, `aggregate_walk_limit_refuses`, `functional_naming_exception_is_exact`; required cases, not reported results.

- [x] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [x] **Step 2: Green.** Discover common/core/team/root-language/root-standard areas and checked support roots through descriptor scope/fixed placements. Count all folders/assets/mounts under aggregate bounds. Register the reviewed two-category naming table from shared adapter metadata; no package self-exemption or product inference heuristic.
- [x] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** No generic per-kind walker switches, overlapping placement, symlink/include escapes or unchecked nonempty trees; extensions/collections still refuse until registered.

### C31 Area-scoped descriptors and package roots [US1, US5] (3–4 h)

**Phase:** P1/M3
**After:** C30, C03a.
**Files:** crates/maestro-catalog/src/source/kinds/{builtin.rs,agent.rs,skill.rs,instructions.rs,model_card.rs,preset.rs,package.rs}; crates/maestro-catalog/src/source/{descriptor.rs,registry.rs}; crates/maestro-catalog/src/source/tests/{registry.rs,layout.rs,extension.rs}.
**Requirements:** FR-S3-040, FR-S3-046, SC-S3-013, SC-S3-015.
**Named tests:** `area_package_placement_roundtrips`, `role_card_path_matches_identity`, `ambiguous_area_descriptor_refuses`; required cases, not reported results.

- [x] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [x] **Step 2: Green.** Scope existing shapes to registered areas; register package closure roots and preset area/inventory selectors, preserving native metadata/hooks. Move cards to llm/models/<role>; retain exact kernel validator/fingerprint. Changed descriptor versions increment for the one pending /2 cutover. MCP is registered config in C45/C47, never a new mcp resource.
- [x] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Unsupported nonempty kinds/configs refuse; language/standard/delegation semantics belong to C79–C81, not unpriced work in this placement task.

### C32 Qualified v4 IDs and single schema cutover [US1, US5] (2–3 h)

**Phase:** P1/M3
**After:** C31.
**Files:** crates/maestro-catalog/src/source/{types.rs,parse.rs,load.rs,metadata.rs}; crates/maestro-catalog/src/source/tests/ existing 13 fixture modules; tests/fixtures/catalog/{source,model-cards}/; crates/maestro/src/cli/catalog/check.rs; crates/maestro/tests/it/catalog_check.rs.
**Requirements:** FR-S3-002, FR-S3-041, FR-S3-046, SC-S3-016.
**Named tests:** `same_stem_different_kind_accepts`, `duplicate_kind_namespace_name_refuses`, `duplicate_area_namespace_refuses`, `old_or_mixed_layout_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Use kind:namespace/local-name plus package/language/standard roots and global preset IDs. Keep 64-character segment grammar, globally unique area namespaces and per-kind/per-namespace local names. Migrate all existing fixture assertions and /2 check output; old capability IDs/paths and locks get explicit diagnostics.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** No silent alias/rebind or kernel-card/preference schema change; native names and all bounds/refusal regressions remain.

### C33 Qualified layers and removable packages [US1, US5] (3–4 h)

**Phase:** P1/M3
**After:** C32.
**Files:** crates/maestro-catalog/src/source/{check.rs,metadata.rs}; crates/maestro-catalog/src/source/kinds/{agent.rs,package.rs}; crates/maestro-catalog/src/source/tests/{references.rs,layout.rs,accepted.rs,schema.rs,support.rs}.
**Requirements:** FR-S3-042, FR-S3-043, FR-S3-046, SC-S3-015, SC-S3-016.
**Named tests:** `common_to_core_refuses`, `core_to_team_refuses`, `language_to_team_refuses`, `removed_package_dangling_reference_refuses`, `package_removal_keeps_core_bytes`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Resolve explicit qualified requires only; workflow labels select/grant nothing. Enforce global→global, core→global and selected-team/language rules; no common/standard→core, core→team or language→team edge. Every required common/core/standard member belongs to every preset closure. Knowledge-only native context stays thin although framework declarations are available. Raw MCP binding owners must be selected; C47b supplies config wiring.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Removal reports every surviving missing edge, preserves immutable roots and unrelated selections, and never falls back to a basename. Keep historical +1 h C33 adjustment, no second charge.

### C34 Area ownership and mandatory roots [US1, US5] (1–2 h)

**Phase:** P1/M3
**After:** C33, C80a.
**Files:** crates/maestro-catalog/src/source/kinds/package.rs; crates/maestro-catalog/src/source/{check.rs,metadata.rs}; crates/maestro-catalog/src/source/tests/{rulings.rs,references.rs,schema.rs}.
**Requirements:** FR-S3-041, FR-S3-042, FR-S3-046, SC-S3-015, SC-S3-016.
**Named tests:** `area_owner_reference_mismatch_refuses`, `mandatory_roots_selected_once`, `missing_reviewed_maestro_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Use C80a owners/maintainers schema and derived resource ownership. Select reviewed common/core and all standards exactly once, with reviewed agent:core/maestro. Check namespace consistency, no independent resource owner list, and required closure maturity.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** One canonical Maestro and one owner record per area, not one principal only; declaration/offline validation confers no GitHub approval or runtime grant.

### C35 Delegated CODEOWNERS generation [US1, US5] (2–3 h)

**Phase:** P1/M3
**After:** C34, C80a.
**Files:** crates/maestro-catalog/src/source/{ownership.rs,tests/ownership.rs}; crates/maestro/src/cli/catalog/codeowners.rs; crates/maestro/tests/it/catalog_codeowners.rs; MAN .github/{CODEOWNERS,workflows/check.yml}.
**Requirements:** FR-S3-041, FR-S3-045, FR-S3-058, SC-S3-015, SC-S3-018.
**Named tests:** `codeowners_drift_refuses`, `descriptor_owner_rule_wins_last`, `removed_area_rule_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Render anchored owner+maintainer content rules followed by owners-only descriptor/exception rules. Root descriptor owners govern shared support, generator, CI, ownership policy and generated CODEOWNERS. --check compares without writing; CI refuses stale/missing/extra/edited rules.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** No second owner registry, broad-rule protection bypass or claim of independent quorums; C80b verifies trusted identities/approvals.

### C36 Area-based preset and inventory composition [US1, US5] (3–4 h)

**Phase:** P1/M3
**After:** C34, C05, C50.
**Files:** crates/maestro-catalog/src/bootstrap/{compose.rs,tests.rs}; crates/maestro-catalog/src/source/kinds/preset.rs; tests/fixtures/catalog/bootstrap/; crates/maestro/tests/it/catalog_init.rs.
**Requirements:** FR-S3-004, FR-S3-042, FR-S3-047, SC-S3-004, SC-S3-016.
**Named tests:** `mandatory_roots_selected_once`, `unselected_inventory_refuses`, `distinct_inventory_output_collision_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Extend PresetPort using C50 area/inventory selectors and explicit owner-local files. Include common and selected language starter inputs once; use the existing S1 registry and C04 writer. Preserve strict JSON, hostile input bounds, zero-write previews and inert scripts.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** One checked selection/compose path; even identical outputs from distinct sources collide. Rust lives in languages/rust, never a team capability.

### C37 Complete source-bound project and authoring locks [US1, US5] (2–3 h)

**Phase:** P1/M3
**After:** C36.
**Files:** crates/maestro-catalog/src/bootstrap/{project.rs,tests.rs}; crates/maestro/src/cli/init.rs; crates/maestro/tests/it/catalog_init.rs.
**Requirements:** FR-S3-046, FR-S3-047, SC-S3-004, SC-S3-016.
**Named tests:** `old_authoring_lock_requires_preview`, `every_selected_input_is_locked`, `changed_source_path_requires_preview`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Emit project/authoring-lock /2 with exact selected areas and complete source identity/revision/digest inventory. Include descriptors, resources/sidecars, presets, explicit inventories/assets and selected checked configs. C47a adds runtime non-resource wiring rather than hiding it here. Recheck exact bytes before apply through C04.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** No incomplete source closure, implicit authority migration or automatic lock rebind; changed user bytes survive replay/removal.

### C39 Migration verification and review reserve [US1, US5] (3–4 h)

**Phase:** P1/M3

**After:** C35, C37.
**Files:** only fixes in C30–C37's named source/test/fixture files;
`specs/003-catalog/research/owner-first-migration.md` for public evidence.
**Requirements:** SC-S3-008, SC-S3-009, SC-S3-015, SC-S3-016.

- [ ] **Step 1: Red.** Require an evidence matrix for every six-rule neighbour,
  source/CLI `/2` marker, descriptor version, migrated fixture family and old-lock
  refusal; absent/zero-test evidence fails this acceptance review.
- [ ] **Step 2: Green.** Repair only migration regressions, retaining C04,
  source bounds, namespace collisions, settings authority and kernel card
  fingerprints. Run focused source/bootstrap/CLI checks, normal workspace and
  three-target checks, then independent review; do not run local mutations.
- [ ] **Step 3: Check.** Record exact commands/exits/test totals, three-OS CI,
  normal hooks, Markdown/links, generated CODEOWNERS drift proof and task
  mappings. Record mutation/coverage evidence from CI when available, never
  invented local results. Native live evidence is C40/C08; S6 is separate.

**Acceptance:** the public v4 checker/bootstrap migration is ready for
C02 seed publication. This 3–4 h is the proposal's migration reserve, not a
second addition to the baseline 16–24 h review/CI reserve.

### C40 Owner-qualified native projection delta [US1] (2–4 h)

**Phase:** P1/M3

**After:** C07, C33, C37; OA2 for pinned live host probes.
**Files:** `crates/maestro-catalog/src/hosts/{copilot.rs,pi.rs}`,
`crates/maestro-catalog/src/hosts/tests/{copilot.rs,pi.rs}`,
`crates/maestro/tests/it/{catalog_copilot.rs,catalog_pi.rs}`,
`specs/003-catalog/research/hosts.md`.
**Requirements:** FR-S3-006, SC-S3-001, SC-S3-016.

- [ ] **Step 1: Red.** Add `native_alias_collision_refuses` for non-injective
  hyphen joining, host normalization/length limits and user shadows; test
  `qa/test-planning` → `qa-test-planning` with MCP server/tool rewrites and
  reserved agent alias `maestro`. Partial name-only rewriting must fail.
- [ ] **Step 2: Green.** Map source IDs, paths and references together through
  existing host adapters; record exact source-to-native mapping in provenance.
  Keep source local-name validation and C01's opposite user/project precedence
  checks. No slash-support assumption, model fallback or provider installation.
- [ ] **Step 3: Check.** Run focused catalog_copilot/catalog_pi tests and the
  C07/C08 pinned live discovery/MCP/restart probes under OA2. Keep missing
  evidence blocked and in-session reload not-run until actually measured.

**Acceptance:** incremental alias/collision proof, not a re-estimate of both
planned host adapters; registration alone never launches a server.

### C41 S6 collection descriptor contract [US5] (2–3 h)

**Phase:** S6/collections

**After:** C31, C34, C39, C66; S6 contract work, not an M3 prerequisite.
**Files:** `crates/maestro-catalog/src/source/kinds/{builtin.rs,collection.rs}`,
`crates/maestro-catalog/src/source/tests/collection.rs`,
`tests/fixtures/catalog/collections/{public.json,invalid.json}`;
necessary module registration only. No MAN collection seed in this task.
**Requirements:** FR-S3-040, FR-S3-048, FR-S3-068, SC-S3-017, SC-S3-025.

- [ ] **Step 1: Red.** Test descriptor absence, exact owner-relative
  `knowledge/collections/<name>/collection.json` placement, strict fields,
  ADR-0014 strict JSON/core declaration, exact source-policy reference,
  binding-only credentials/storage and unsupported versions/types. Add
  `collection_local_url_rules_refuse` and `collection_source_digest_is_exact`.
  `collection_install_is_offline` observes zero fetch calls.
- [ ] **Step 2: Green.** Register the collection descriptor and isolated
  contract validator using supported descriptor shapes; no generic checker
  branch or second collection registry. Preserve named owner, qualified ID,
  schema, provenance requirements and independent runtime ACLs. Rules belong
  only to C66 source resources; the collection stores no duplicate policy.
- [ ] **Step 3: Check.** Run capped nextest `source::tests::collection` in
  maestro-catalog with synthetic fixtures; show a removed descriptor refuses
  nonempty collection content rather than ignoring the knowledge subtree.

**Acceptance:** catalog contract only; no crawling, provider provisioning,
new approval store or collection activation. Signed-review rule admission
comes from the source package; current local access remains separate.

### C42 S6 additive private-source mount [US5] (2–4 h)

**Phase:** S6/collections

**After:** C41, C35, C37; private access remains separately approved.
**Files:** `crates/maestro-catalog/src/source/{tree.rs,overlay.rs}`,
`crates/maestro-catalog/src/source/tests/overlay.rs`,
`crates/maestro-catalog/src/bootstrap/project.rs`,
`crates/maestro/src/cli/catalog/check.rs`, `crates/maestro/tests/it/catalog_overlay.rs`;
necessary module registration only; synthetic sources, no private checkout.
**Requirements:** FR-S3-044, SC-S3-015, SC-S3-017.

- [ ] **Step 1: Red.** Add `overlay_cannot_shadow_public`,
  `public_closure_cannot_require_private`,
  `missing_overlay_preserves_public_selection` and `combined_sources_share_limits`.
  Cover duplicate bytes/IDs/paths, symlinks, owner/core/trust overrides,
  self-authorized publisher, indirect private edges and reordered sources.
- [ ] **Step 2: Green.** Add the explicit opt-in pinned `SourceTree` adapter,
  restricted to application-workflow's `workload-docs` collection subtree and
  `workload-private` preset. The mount adds no owner manifest; public ownership remains
  authoritative and private publishing/access authorization stays separate.
  Check public alone first, then combined inputs with aggregate limits and
  source-aware diagnostics/locks. Never last-wins or silently shadow.
- [ ] **Step 3: Check.** Run capped nextest `source::tests::overlay` and
  `bootstrap::tests` in maestro-catalog, plus `catalog_overlay` in maestro.
  Verify missing/unauthorized mounts leave public presets usable; public release
  fixtures package no private bytes and offline checks make zero network calls.

**Acceptance:** additive composition only; private corpus/evidence stays outside
catalog inputs, public CI and public releases.

### C43 S6 source-rule admission and provenance contract [US5] (2–3 h)

**Phase:** S6/collections

**After:** C41, C42; this task is a synthetic contract. The separately assigned
S6 runtime adapter also needs C69 for real private-package consumption and
recorded manifest review, never C42 mount expansion.
**Files:** `crates/maestro-catalog/src/source/kinds/collection.rs`,
`crates/maestro-catalog/src/source/tests/collection.rs`,
`specs/003-catalog/research/collection-contract.md` for the S6 handoff.
**Requirements:** FR-S3-048, FR-S3-068, SC-S3-017, SC-S3-025.

- [ ] **Step 1: Red.** Add `unadmitted_source_rules_never_fetch`,
  `exclusion_wins_on_redirect` and `retained_original_has_provenance` using
  synthetic HTTPS URLs. Refuse off-origin/path/version/type, unapproved redirect,
  denied access, credential-bearing URL, login/error page and forbidden binary
  contract cases; verify exclusions win and unadmitted rules admit no URL.
- [ ] **Step 2: Green.** Check collection-to-source identity/digest links and
  preserved rule/decision/migration references, signed catalog plus recorded
  owner/maintainer review, expiry and current admission. Reuse the source types,
  never collection-local `[approved_urls]` or a new rule parser. Require S6
  seed/discovered-link/every-redirect access checks and private
  URL/version/digest/transformation provenance. Audit chrome removal/exact-body
  deduplication; keep admitted originals under private retention policy. Do not
  authorize relevance-based deletion or deletion of existing evidence.
- [ ] **Step 3: Check.** Run capped nextest `source::tests::collection`, Markdown
  and offline links. The handoff distinguishes pure contract/fixture validation
  from later crawler enforcement; check/install never fetch, and no real private
  URL, corpus read, ingestion or provisioning occurs here.

**Acceptance:** a falsifiable S6 consumer contract, not a crawler or a claim that
private collection setup/content approval is complete.

## Approved manifest v4 and gap tasks

All following names are planned checks, not passing implementation evidence.
The owner's 20:45 URL-rule amendment changes only C66's estimate: 3→4 h;
C52a/b/C68 and C41/C43 reuse their existing budgets. The 57 design task IDs
remain, now 204 h including this separately recorded +1 h.
Each row from design §8.4 appears once; umbrella C45/C47/C49/C51/C52/C53 IDs
carry no second task or budget. Physical order does not change Phase/After.

### C44 Record approved manifest v4 contract [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C38.
**Files:** specs/003-catalog/{spec.md,plan.md,tasks.md,traceability.json}; docs/architecture/{01-knowledge-pipeline.md,03-agent-orchestration.md,05-platform-and-operations.md,06-roadmap.md,07-extensibility.md,08-traceability.md,09-reverse-engineering.md}; README.md; docs/adr/{0012-catalog-and-runtime-in-separate-repositories.md,0013-extensions-through-events-and-operations-out-of-process.md,0014-strict-json-for-collection-and-source-policy.md,0022-manifest-layout-v4-and-language-neutral-extensions.md,README.md}; generated .github/copilot-instructions.md.
**Requirements:** FR-S3-040, FR-S3-041, FR-S3-042, FR-S3-043, FR-S3-044, FR-S3-045, FR-S3-046, FR-S3-047, FR-S3-048, FR-S3-049, FR-S3-050, FR-S3-051, FR-S3-052, FR-S3-053, FR-S3-054, FR-S3-055, FR-S3-056, FR-S3-057, FR-S3-058, FR-S3-059, FR-S3-060, FR-S3-061, FR-S3-062, FR-S3-063, FR-S3-064, FR-S3-065, FR-S3-066, FR-S3-067, SC-S3-009, SC-S3-015, SC-S3-016, SC-S3-018, SC-S3-019, SC-S3-020, SC-S3-021, SC-S3-022, SC-S3-023, SC-S3-024, FR-S3-068, SC-S3-025.
**Design coverage:** MD01, MD12 (approved design §8.4).
**Named tests:** `c44_contract_reconciliation`, `catalog_traceability_inventory_matches_exact_rows_and_dispositions`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Amend the four S3 documents, architecture/README and ADR-0012/0013/0022. Reconcile exact requirement links, phases, task totals and acyclic dependencies; retain the frozen 85 architecture keys and six exclusions. Refuse stale source/scope claims or invented delivered evidence.
- [ ] **Step 3: Check.** Run `prek run --from-ref 815ff33 --to-ref HEAD`, the
  conventions `catalog_traceability` suite and the C44 reconciliation of every
  task/requirement/gap/phase/edge. Record commands, exits and pushed hash in the
  ledger report. No implementation or runtime test result is claimed.

**URL-rule amendment (+0 h):** Record the 20:45 URL-rule decision and its +1 h C66 delta; ADR-0014/architecture 01 join the same documentation sync.

**Acceptance:** Amend the four S3 documents, architecture/README and ADR-0012/0013/0022. Reconcile exact requirement links, phases, task totals and acyclic dependencies; retain the frozen 85 architecture keys and six exclusions. Refuse stale source/scope claims or invented delivered evidence.

### C45a Strict backend base descriptors [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C44, C39.
**Files:** crates/maestro-catalog/src/source/{backends.rs,kinds/backend.rs,tests/backends.rs}; tests/fixtures/catalog/backends/.
**Requirements:** FR-S3-002, FR-S3-037, FR-S3-050, SC-S3-019.
**Design coverage:** MD02, MD03 (approved design §8.4).
**Named tests:** `backend_valid_base_accepts`, `inactive_backend_table_refuses`, `backend_numeric_bounds_refuse`, `uncompiled_backend_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Registered backend base types/settings over bounded SourceTree. Unknown/inactive bad tables, zero/overflow/power-of-two and unavailable selections refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Registered backend base types/settings over bounded SourceTree. Unknown/inactive bad tables, zero/overflow/power-of-two and unavailable selections refuse.

### C45b Add-or-narrow backend extensions [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C45a.
**Files:** crates/maestro-catalog/src/source/{backends.rs,backend_extensions.rs,tests/backend_extensions.rs}; tests/fixtures/catalog/backends/.
**Requirements:** FR-S3-042, FR-S3-043, FR-S3-047, FR-S3-050, SC-S3-019.
**Design coverage:** MD03, MD04 (approved design §8.4).
**Named tests:** `backend_extension_narrows`, `backend_replacement_refuses`, `extension_aggregate_limit_refuses`, `extension_collision_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Typed role extensions/selectors, owner-scoped bindings and removal checks. Replace/type/endpoint/widening/collision attempts refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Typed role extensions/selectors, owner-scoped bindings and removal checks. Replace/type/endpoint/widening/collision attempts refuse.

### C46 Manifest defaults through the S1 registry [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C45a, C17, C05b.
**Files:** crates/maestro-catalog/src/settings/{defaults.rs,tests/defaults.rs}; crates/maestro-catalog/src/source/kinds/settings.rs; S1 registered graph setting descriptors at their integrated module.
**Requirements:** FR-S3-014, FR-S3-015, FR-S3-027, FR-S3-028, FR-S3-029, FR-S3-030, FR-S3-031, FR-S3-050, SC-S3-010, SC-S3-019.
**Design coverage:** MD03, MD11 (approved design §8.4).
**Named tests:** `manifest_default_producer_is_unique`, `four_layers_keep_frozen_semantics`, `masked_invalid_default_refuses`, `graph_bounds_match_s2`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Catalog defaults feed S1 once; graph descriptor sync. Four layers, masked invalid input, locked fields and missing defaults/compiled adapters tested.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Catalog defaults feed S1 once; graph descriptor sync. Four layers, masked invalid input, locked fields and missing defaults/compiled adapters tested.

### C47a Frozen defaults and complete non-resource locks [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C46, C37.
**Files:** crates/maestro-catalog/src/bootstrap/{project.rs,tests.rs}; crates/maestro-catalog/src/settings/{session.rs,tests/session.rs}; crates/maestro-catalog/src/bundle/{write.rs,read.rs}; existing S2 settings-consumer port.
**Requirements:** FR-S3-008, FR-S3-009, FR-S3-014, FR-S3-015, FR-S3-028, FR-S3-047, FR-S3-050, SC-S3-010, SC-S3-019.
**Design coverage:** MD03, MD11 (approved design §8.4).
**Named tests:** `nonresource_input_change_requires_preview`, `session_keeps_admitted_defaults`, `bundle_preserves_config_closure`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Init/session frozen defaults, complete non-resource locks/bundle preservation and S2 consumer seam. Changed config/lock cannot replay; no live engine needed for fixtures.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Init/session frozen defaults, complete non-resource locks/bundle preservation and S2 consumer seam. Changed config/lock cannot replay; no live engine needed for fixtures.

### C47b Vector and MCP configuration adapters [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C47a, C45b, C40.
**Files:** crates/maestro-catalog/src/source/{backend_extensions.rs,tests/backend_extensions.rs}; crates/maestro-catalog/src/hosts/{bindings.rs,tests/bindings.rs}; existing vector and MCP configuration ports.
**Requirements:** FR-S3-006, FR-S3-042, FR-S3-050, SC-S3-016, SC-S3-019.
**Design coverage:** MD03, MD04 (approved design §8.4).
**Named tests:** `mcp_binding_owner_must_be_selected`, `legacy_mcp_edge_requires_migration`, `vector_identity_is_unchanged`, `endpoint_replacement_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Vector/MCP binding adapters and retired MCP-edge migration. Unknown/unselected server, alias shadow and changed endpoint refuse without launch.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Vector/MCP binding adapters and retired MCP-edge migration. Unknown/unselected server, alias shadow and changed endpoint refuse without launch.

### C48 Publish checked backend declarations [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C02, C45b, C47b; external G25/E07a qualification and approved real owners before actual pins; does not wait for G22.
**Files:** MAN core/backends/{graphdb,vectordb,mcp}/config.toml; core/package.toml; settings/defaults.toml; retire core/mcp/*.toml.
**Requirements:** FR-S3-014, FR-S3-050, SC-S3-019.
**Design coverage:** MD03 (approved design §8.4).
**Named tests:** `core_backend_fixtures_accept`, `backend_metadata_requires_qualified_pin`, `source_config_never_launches`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Publish the three checked core configs; this task reuses
  C02's MCP base without a second declaration or seed dependency on G25.
  Add qualified graph/vector metadata after G25/E07a and applicable backend
  qualification plus approved owners, never an invented pin. Preserve the MCP
  cutover and C02/C08's independent knowledge-only checkpoint.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Three checked core configs; C48 reuses C02's MCP base and adds
qualified graph/vector publication without moving its external waits into C08.
No invented pin, duplicate server registry or extra seed charge; estimate stays 3 h.

### C49a Native and featureless graph acceptance [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C47b, C48, C18, C16b; external S2 G27 E11, G28 and G22 integrated.
**Files:** crates/maestro-catalog/src/settings/tests/backends.rs; crates/maestro/tests/it/catalog_graph_backend.rs; specs/003-catalog/research/backend-acceptance.md.
**Requirements:** FR-S3-011, FR-S3-015, FR-S3-050, SC-S3-019.
**Design coverage:** MD03, MD11 (approved design §8.4).
**Named tests:** `graph_none_makes_zero_calls`, `graph_unavailable_never_becomes_disabled`, `installed_graph_requires_current_admission`, `graph_explain_keeps_pin`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Native/featureless graph, installed admission/lock/explain proofs; additionally G27 E11/G28/G22. No silent disabled fallback.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Native/featureless graph, installed admission/lock/explain proofs; additionally G27 E11/G28/G22. No silent disabled fallback.

### C49b Vector and MCP package round-trip [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C47b, C48, C18, C16b, C55.
**Files:** crates/maestro-catalog/src/install/tests/packages.rs; crates/maestro/tests/it/catalog_backends.rs.
**Requirements:** FR-S3-011, FR-S3-012, FR-S3-043, FR-S3-050, SC-S3-003, SC-S3-019.
**Design coverage:** MD03, MD04 (approved design §8.4).
**Named tests:** `backend_add_remove_keeps_core_bytes`, `backend_collision_keeps_previous_install`, `backend_remove_preserves_shared_bindings`, `backend_tamper_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Vector/MCP/backend-extension round-trip, collision/removal/trust negatives and unchanged core bytes; executable extension proof belongs to C59.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Vector/MCP/backend-extension round-trip, collision/removal/trust negatives and unchanged core bytes; executable extension proof belongs to C59.

### C50 Owner-local bootstrap inventories [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C44, C34, C05.
**Files:** crates/maestro-catalog/src/bootstrap/{compose.rs,inventory.rs,tests/inventory.rs}; crates/maestro-catalog/src/source/kinds/preset.rs; tests/fixtures/catalog/bootstrap/.
**Requirements:** FR-S3-004, FR-S3-005, FR-S3-047, SC-S3-004, SC-S3-016.
**Design coverage:** MD04 (approved design §8.4).
**Named tests:** `selected_owner_inventory_accepts`, `unselected_inventory_refuses`, `inventory_escape_refuses`, `identical_output_collision_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Owner-local explicit inventories through PresetPort; unknown/unselected inventory, escape, changed input and identical-output collision refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Owner-local explicit inventories through PresetPort; unknown/unselected inventory, escape, changed input and identical-output collision refuse.

### C51a Pinned catalog index and type navigation [US1, US5] (2 h)

**Phase:** P1/M3
**After:** C35, C37, C53a.
**Files:** crates/maestro-catalog/src/source/{index.rs,tests/index.rs}; crates/maestro/src/cli/catalog/index.rs; MAN marketplace/index.json and docs/catalog/by-type.md.
**Requirements:** FR-S3-046, FR-S3-051, FR-S3-052, SC-S3-021.
**Design coverage:** MD01, MD07, MD09 (approved design §8.4).
**Named tests:** `catalog_index_is_deterministic`, `stale_extra_index_row_refuses`, `public_index_excludes_private`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Generated pinned-catalog marketplace/type view and drift CLI; stale/extra/private rows refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Generated pinned-catalog marketplace/type view and drift CLI; stale/extra/private rows refuse.

### C51b Independent-release index fields [US1, US5] (3 h)

**Phase:** P2/L1
**After:** C51a, C53b, C28.
**Files:** crates/maestro-catalog/src/source/{index.rs,tests/index.rs}; MAN marketplace/index.json and docs/catalog/by-type.md.
**Requirements:** FR-S3-052, SC-S3-020.
**Design coverage:** MD07, MD08 (approved design §8.4).
**Named tests:** `release_index_binds_signed_digest`, `unavailable_release_is_noninstallable`, `deprecation_index_drift_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Independent-release/version/deprecation listing; mutable or unavailable release never becomes installable.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Independent-release/version/deprecation listing; mutable or unavailable release never becomes installable.

### C52a Registry schemas and editor associations [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C32, C44.
**Files:** crates/maestro-catalog/src/source/{schema.rs,tests/schema.rs}; crates/maestro/src/cli/catalog/schema.rs; MAN schemas/source-2/.
**Requirements:** FR-S3-002, FR-S3-037, FR-S3-051, SC-S3-013, SC-S3-021, FR-S3-068, SC-S3-025.
**Design coverage:** MD02, MD09 (approved design §8.4).
**Named tests:** `schema_export_is_deterministic`, `stale_schema_refuses`, `unknown_descriptor_version_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Registry/settings schema export and editor association index; drift and unsupported descriptor versions refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**URL-rule amendment (+0 h):** Export core-derived source-policy/decision/promotion/migration JSON Schemas alongside the registry schemas; do not hand-maintain them.

**Acceptance:** Registry/settings schema export and editor association index; drift and unsupported descriptor versions refuse.

### C52b Delegated schemas and every-shape fixtures [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C52a, C45b, C62, C63, C64, C65, C66, C79a, C80a, C81b.
**Files:** crates/maestro-catalog/src/source/{schema.rs,fixtures.rs,tests/fixtures.rs}; crates/maestro/src/cli/catalog/fixtures.rs; MAN fixtures/{valid,invalid}/ and schemas/source-2/.
**Requirements:** FR-S3-002, FR-S3-037, FR-S3-038, FR-S3-051, FR-S3-055, SC-S3-013, SC-S3-014, SC-S3-021, FR-S3-068, SC-S3-025.
**Design coverage:** MD02, MD09, MD11 (approved design §8.4).
**Named tests:** `owning_validator_exports_exact_shape`, `missing_kind_fixture_refuses`, `zero_refusal_cases_refuse`, `stale_fixture_registration_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Semantic-hook schema export and every-kind fixture inventory; delegated fields stay exact, missing/zero refusal neighbours fail.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**URL-rule amendment (+0 h):** Include C66's four source-rule JSON families in schema/fixture inventory and editor associations, with actual core-type export and drift refusal.

**Acceptance:** Semantic-hook schema export and every-kind fixture inventory; delegated fields stay exact, missing/zero refusal neighbours fail.

### C52c Authoring templates and package-new [US1, US5] (3 h)

**Phase:** P2/A0
**After:** C52b, C28.
**Files:** crates/maestro-catalog/src/bootstrap/{authoring.rs,tests/authoring.rs}; crates/maestro/src/cli/package/new.rs; MAN templates/{package,agent,skill,prompt}/.
**Requirements:** FR-S3-005, FR-S3-041, FR-S3-059, SC-S3-021.
**Design coverage:** MD02, MD07 (approved design §8.4).
**Named tests:** `package_new_preview_writes_nothing`, `package_new_occupied_output_refuses`, `package_new_cannot_invent_owner`, `package_new_stays_authored`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** MAN kit plus package-new preview/apply over C04. Occupied outputs, invented owner and automatic reviewed maturity refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** MAN kit plus package-new preview/apply over C04. Occupied outputs, invented owner and automatic reviewed maturity refuse.

### C53a Exact versions and compatibility [US1, US5] (2 h)

**Phase:** P1/M3
**After:** C34.
**Files:** crates/maestro-catalog/src/source/{versions.rs,kinds/package.rs,tests/versions.rs}; crates/maestro-catalog/src/bundle/read.rs.
**Requirements:** FR-S3-008, FR-S3-009, FR-S3-041, FR-S3-052, SC-S3-002, SC-S3-003.
**Design coverage:** MD08 (approved design §8.4).
**Named tests:** `exact_package_pins_accept`, `conflicting_exact_pin_refuses`, `incompatible_runtime_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Exact package version/dependency pins and existing runtime compatibility; conflicting exact pins refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Exact package version/dependency pins and existing runtime compatibility; conflicting exact pins refuse.

### C53b Intervals and signed deprecation [US1, US5] (3 h)

**Phase:** P2/L1
**After:** C53a, C28.
**Files:** crates/maestro-catalog/src/source/{versions.rs,tests/versions.rs}; crates/maestro-catalog/src/install/{deprecation.rs,tests/deprecation.rs}.
**Requirements:** FR-S3-052, SC-S3-020.
**Design coverage:** MD08 (approved design §8.4).
**Named tests:** `compatible_lock_is_retained`, `highest_admitted_stable_selected`, `empty_version_intersection_refuses`, `republished_version_refuses`, `premature_deprecation_removal_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Dependency intervals, deterministic resolution and deprecation workflow; empty intersections/republished versions refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Dependency intervals, deterministic resolution and deprecation workflow; empty intersections/republished versions refuse.

### C54 Verified package-add transaction [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C16, C18, C47b, C53a, C51a, C60.
**Files:** crates/maestro-catalog/src/install/{packages.rs,tests/packages.rs}; crates/maestro/src/cli/package/{mod.rs,add.rs}; crates/maestro/tests/it/catalog_package.rs.
**Requirements:** FR-S3-008, FR-S3-009, FR-S3-010, FR-S3-011, FR-S3-012, FR-S3-013, FR-S3-015, FR-S3-032, FR-S3-033, FR-S3-034, FR-S3-047, FR-S3-052, SC-S3-002, SC-S3-003, SC-S3-011, SC-S3-019.
**Design coverage:** MD07, MD08 (approved design §8.4).
**Named tests:** `package_add_verified_selection_accepts`, `package_tamper_keeps_prior_install`, `package_preview_change_refuses`, `interrupted_package_activation_recovers`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Package-add plan/apply over verified lifecycle and complete pins. Tamper, incompatibility, changed preview and interrupted activation keep prior install.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Package-add plan/apply over verified lifecycle and complete pins. Tamper, incompatibility, changed preview and interrupted activation keep prior install.

### C55 Owned package removal [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C54.
**Files:** crates/maestro-catalog/src/install/{packages.rs,tests/packages.rs}; crates/maestro/src/cli/package/remove.rs; crates/maestro/tests/it/catalog_package.rs.
**Requirements:** FR-S3-005, FR-S3-012, FR-S3-043, FR-S3-047, FR-S3-052, SC-S3-003, SC-S3-004, SC-S3-019.
**Design coverage:** MD04, MD07 (approved design §8.4).
**Named tests:** `package_remove_preserves_edits_and_evidence`, `mandatory_package_remove_refuses`, `reverse_dependency_remove_refuses`, `shared_dependency_survives`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Package-remove transaction/receipt. Refuse mandatory roots/reverse dependants; preserve shared dependencies, edited files and stored evidence.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Package-remove transaction/receipt. Refuse mandatory roots/reverse dependants; preserve shared dependencies, edited files and stored evidence.

### C56 Package selection in plain and terminal init [US1, US5] (3 h)

**Phase:** P2/L1
**After:** C54, C55, C05k, C51b, C28.
**Files:** crates/maestro/src/cli/init/{flow.rs,terminal.rs}; crates/maestro/tests/it/catalog_init_menu.rs.
**Requirements:** FR-S3-030, FR-S3-047, FR-S3-052, SC-S3-010, SC-S3-020.
**Design coverage:** MD07 (approved design §8.4).
**Named tests:** `package_menu_script_plan_parity`, `package_menu_cancel_writes_nothing`, `inaccessible_private_entry_is_hidden`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Init plain/TUI package selection through the same package plan. Cancel writes nothing; script/menu parity and inaccessible private entries tested.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Init plain/TUI package selection through the same package plan. Cancel writes nothing; script/menu parity and inaccessible private entries tested.

### C57 Catalog-wide frozen explanation [US1, US5] (3 h)

**Phase:** P2/A1
**After:** C18, C47b, C53a, C28.
**Files:** crates/maestro-catalog/src/resolve/{explain.rs,tests/explain.rs}; crates/maestro/src/cli/catalog/explain.rs; crates/maestro/src/mcp/catalog.rs.
**Requirements:** FR-S3-014, FR-S3-015, FR-S3-028, FR-S3-054, SC-S3-010, SC-S3-021.
**Design coverage:** MD07, MD11 (approved design §8.4).
**Named tests:** `catalog_explain_uses_frozen_contributors`, `stale_explain_lock_refuses`, `explain_redacts_secret_private_and_mcp_paths`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Catalog-explain adapter over C18/S1 provenance. Exact contributors/ignored widenings, stale lock refusal, secret/private/path redaction.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Catalog-explain adapter over C18/S1 provenance. Exact contributors/ignored widenings, stale lock refusal, secret/private/path redaction.

### C58 Any-language extension descriptor [US1, US5] (4 h)

**Phase:** P2/X1
**After:** C32, C53a, C28.
**Files:** crates/maestro-catalog/src/source/kinds/{builtin.rs,extension.rs}; crates/maestro-catalog/src/source/tests/extension_contract.rs; tests/fixtures/catalog/extensions/.
**Requirements:** FR-S3-002, FR-S3-037, FR-S3-052, FR-S3-053, FR-S3-054, FR-S3-055, SC-S3-020.
**Design coverage:** MD02, MD06, MD08 (approved design §8.4).
**Named tests:** `extension_local_or_release_oneof`, `extension_mutable_release_refuses`, `extension_missing_contract_refuses`, `extension_unknown_hook_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Extension kind, code source one-of, runtime/tool/config/test/eval contracts. Unpinned release, missing contract and unknown hook subscription refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Extension kind, code source one-of, runtime/tool/config/test/eval contracts. Unpinned release, missing contract and unknown hook subscription refuse.

### C59 Extension artifact and MCP projection handoff [US1, US5] (4 h)

**Phase:** P2/X1
**After:** C54, C58, C61, C63, C67.
**Files:** crates/maestro-catalog/src/install/{extensions.rs,tests/extensions.rs}; crates/maestro-catalog/src/hosts/bindings.rs; crates/maestro/tests/it/catalog_extensions.rs; docs/how-to/catalog.md.
**Requirements:** FR-S3-010, FR-S3-011, FR-S3-012, FR-S3-043, FR-S3-050, FR-S3-053, FR-S3-054, SC-S3-020.
**Design coverage:** MD06, MD07 (approved design §8.4).
**Named tests:** `extension_verified_projection_accepts`, `extension_install_launches_nothing`, `extension_collision_refuses`, `extension_stale_artifact_refuses`, `extension_remove_preserves_core`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Derived MCP registration/projection ownership, artifact pin checks, removal and S4 handoff record. Collisions/stale pins refuse; install launches zero processes.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Derived MCP registration/projection ownership, artifact pin checks, removal and S4 handoff record. Collisions/stale pins refuse; install launches zero processes.

### C60 Typed secret references without resolution [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C32, C17.
**Files:** crates/maestro-catalog/src/source/{secrets.rs,tests/secrets.rs}; crates/maestro-catalog/src/settings/defaults.rs.
**Requirements:** FR-S3-002, FR-S3-014, FR-S3-054, SC-S3-019.
**Design coverage:** MD02, MD06 (approved design §8.4).
**Named tests:** `secret_reference_oneof_accepts`, `secret_literal_channels_refuse`, `check_install_explain_never_resolve_secrets`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Shared typed secret references across settings/backends/extensions. Literal secrets/URL credentials/argument channels refuse; explain never resolves a secret.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Shared typed secret references across settings/backends/extensions. Literal secrets/URL credentials/argument channels refuse; explain never resolves a secret.

### C61 Extension-scoped Cedar grants [US1, US5] (4 h)

**Phase:** P2/X1
**After:** C19, C58, C60.
**Files:** crates/maestro-catalog/src/policy/{extensions.rs,tests/extensions.rs}; tests/fixtures/catalog/extensions/policies/.
**Requirements:** FR-S3-016, FR-S3-053, FR-S3-054, SC-S3-005, SC-S3-020.
**Design coverage:** MD06 (approved design §8.4).
**Named tests:** `extension_exact_grant_allows`, `extension_missing_grant_denies`, `extension_wrong_digest_denies`, `extension_egress_widening_denies`, `extension_unknown_facts_deny`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Extension-principal Cedar/grant fixture adapter. No grant, wrong digest, wider egress and unknown trusted facts deny; permitted neighbour passes.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Extension-principal Cedar/grant fixture adapter. No grant, wrong digest, wider egress and unknown trusted facts deny; permitted neighbour passes.

### C62 Ten hook descriptors and platform bindings [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C32, C64; supervisor-approved correction to design §8.4: the common
prompt/contracts neighbour uses production descriptors, not substitutes.
**Files:** crates/maestro-catalog/src/source/kinds/{builtin.rs,hook.rs}; crates/maestro-catalog/src/source/tests/hooks.rs; tests/fixtures/catalog/hooks/.
**Requirements:** FR-S3-002, FR-S3-017, FR-S3-042, FR-S3-055, SC-S3-015, SC-S3-021.
**Design coverage:** MD02, MD05 (approved design §8.4).
**Named tests:** `ten_hook_points_validate_without_execution`, `common_prompt_common_contract_accepts`, `common_to_core_refuses`, `dangling_hook_tool_refuses`, `invalid_hook_timeout_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Ten hook-point descriptors, action references and implicit engine/checker event-protocol bindings; no event-contract catalog dependency. Fixtures: a common prompt with common input/output contracts passes; common → core dependency refuses. Unknown point, dangling tool and invalid timeout refuse; no execution path.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Ten hook-point descriptors, action references and implicit engine/checker event-protocol bindings; no event-contract catalog dependency. Fixtures: a common prompt with common input/output contracts passes; common → core dependency refuses. Unknown point, dangling tool and invalid timeout refuse; no execution path.

### C63 Three explicit host capability maps [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C62, C01.
**Files:** crates/maestro-catalog/src/source/{host_config.rs,tests/host_config.rs}; MAN core/hosts/{pi,claude-code,copilot}/config.toml.
**Requirements:** FR-S3-006, FR-S3-017, FR-S3-049, SC-S3-015, SC-S3-016, SC-S3-021.
**Design coverage:** MD02, MD05 (approved design §8.4).
**Named tests:** `each_host_has_ten_explicit_states`, `unsupported_blocking_hook_is_unprotected`, `host_config_cannot_register_code`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Three host mapping configs through registered adapters. All ten events get explicit states; unmapped/unsupported blocking use cannot appear protected.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Three host mapping configs through registered adapters. All ten events get explicit states; unmapped/unsupported blocking use cannot appear protected.

### C64 Prompt handoff contract and eval shapes [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C32, C22b.
**Files:** crates/maestro-catalog/src/source/kinds/{prompt.rs,handoff.rs,contract.rs,eval.rs}; crates/maestro-catalog/src/source/tests/contracts.rs; tests/fixtures/catalog/contracts/.
**Requirements:** FR-S3-002, FR-S3-016, FR-S3-037, FR-S3-055, SC-S3-005, SC-S3-021.
**Design coverage:** MD02 (approved design §8.4).
**Named tests:** `common_contract_reference_accepts`, `unfilled_prompt_template_refuses`, `handoff_missing_section_refuses`, `external_contract_reference_refuses`, `dangling_contract_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Prompt/handoff/eval-case descriptors and isolated body/reference checks using existing JSON contract consumer. Unfilled template, missing sections and dangling contracts refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Prompt/handoff/eval-case descriptors and isolated body/reference checks using existing JSON contract consumer. Unfilled template, missing sections and dangling contracts refuse.

### C65 Quality profiles separate from session profiles [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C32, C17.
**Files:** crates/maestro-catalog/src/source/kinds/quality_profile.rs; crates/maestro-catalog/src/source/tests/quality_profile.rs; tests/fixtures/catalog/quality/.
**Requirements:** FR-S3-014, FR-S3-037, FR-S3-056, FR-S3-057, SC-S3-018.
**Design coverage:** MD02, MD11 (approved design §8.4).
**Named tests:** `quality_profile_is_not_session_profile`, `quality_thresholds_only_narrow`, `missing_required_binding_stays_unresolved`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Quality-profile descriptor distinct from M059, baseline/check accumulation and threshold narrowing. Missing bindings stay unresolved; weakened gate refuses.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Quality-profile descriptor distinct from M059, baseline/check accumulation and threshold narrowing. Missing bindings stay unresolved; weakened gate refuses.

### C66 Knowledge sources and manifest-owned URL rules [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C32; synchronize existing S6 wire types and N07 IdentityMigration
only after reviewed fixes land; not the S6 runtime catalog adapter.
**Files:** crates/maestro-catalog/src/source/kinds/knowledge_source.rs; crates/maestro-catalog/src/source/tests/knowledge_source.rs; tests/fixtures/catalog/sources/; owning core wire types are consumed, not copied. C52b publishes the typed synthetic companions in MAN fixtures, not a real source seed.
**Requirements:** FR-S3-002, FR-S3-037, FR-S3-054, FR-S3-055, FR-S3-068, SC-S3-021, SC-S3-025.
**Design coverage:** MD02 (approved design §8.4).
**Named tests:** `pinned_source_with_provenance_accepts`, `mutable_unqualified_source_refuses`, `source_secret_literal_refuses`, `source_check_never_fetches`, `source_url_rule_families_roundtrip`,
`source_rule_duplicate_key_refuses`, `source_rule_digest_or_expiry_refuses`,
`source_review_evidence_is_not_self_asserted`, `source_private_inventory_never_publishes`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Knowledge-source descriptor plus strict JSON policy, decisions/promotions with expiry and URL-identity migrations beside source.toml. Reuse core types/validators, exact inventories/digests and ownership-derived signed-review evidence. Author valid/invalid neighbours for every family, including fabricated review, expiry, changed digest and private leakage. C52a/b/C68 consume the registrations/fixtures. No collection-local URL rules, per-site engine rules, network calls or S6 runtime adapter.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Knowledge-source descriptor plus strict JSON policy, decisions/promotions with expiry and URL-identity migrations beside source.toml. Reuse core types/validators, exact inventories/digests and ownership-derived signed-review evidence. Author valid/invalid neighbours for every family, including fabricated review, expiry, changed digest and private leakage. C52a/b/C68 consume the registrations/fixtures. No collection-local URL rules, per-site engine rules, network calls or S6 runtime adapter.

### C67 Bounded offline package eval checkpoint [US1, US5] (4 h)

**Phase:** P2/E1
**After:** C64, C65, C66, C62, C58, C19, C22b.
**Files:** crates/maestro-catalog/src/eval/{package.rs,drivers.rs,tests/package.rs}; crates/maestro/src/cli/catalog/eval.rs; MAN .github/workflows/check.yml.
**Requirements:** FR-S3-013, FR-S3-051, FR-S3-060, SC-S3-020, SC-S3-021.
**Design coverage:** MD09 (approved design §8.4).
**Named tests:** `package_eval_report_binds_source_closure`, `wrong_eval_expectation_fails`, `missing_eval_driver_fails`, `zero_package_evals_fail`, `eval_egress_is_denied`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Registered offline package-eval drivers and exact revision-bound reports. Wrong expectation, missing driver, zero tests and unauthorized egress fail.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Registered offline package-eval drivers and exact revision-bound reports. Wrong expectation, missing driver, zero tests and unauthorized egress fail.

### C68 Four-checkpoint CI wiring [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C52b, C51a, C35, C80b.
**Files:** crates/maestro/src/cli/catalog/{schema.rs,fixtures.rs,index.rs,owners.rs}; crates/maestro/tests/it/catalog_checkpoints.rs; MAN .pre-commit-config.yaml and .github/workflows/{check,release}.yml.
**Requirements:** FR-S3-013, FR-S3-051, FR-S3-058, SC-S3-021, FR-S3-068, SC-S3-025.
**Design coverage:** MD09 (approved design §8.4).
**Named tests:** `checkpoint_generation_drift_fails`, `checkpoint_missing_refusal_fails`, `release_check_subject_mismatch_fails`, `untrusted_pr_has_no_credentials`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Four-checkpoint/fixture commands and MAN CI integration. Schema/index/owner drift, missing valid/refusal cases and release-check mismatch fail; generic package evals stay C67.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**URL-rule amendment (+0 h):** All four checkpoints include C66 source-rule fixtures and schema drift; CI supplies recorded ownership review and install rechecks signed digest-bound admission.

**Acceptance:** Four-checkpoint/fixture commands and MAN CI integration. Schema/index/owner drift, missing valid/refusal cases and release-check mismatch fail; generic package evals stay C67.

### C69 Signed independent and private packages [US1, US5] (4 h)

**Phase:** P2/L1
**After:** C13a, C14, C16b, C53b, C67, C54.
**Files:** crates/maestro-catalog/src/install/{package_source.rs,tests/package_source.rs}; crates/maestro-catalog/src/trust/admission.rs; tests/fixtures/catalog/private-packages/.
**Requirements:** FR-S3-010, FR-S3-011, FR-S3-012, FR-S3-013, FR-S3-044, FR-S3-052, FR-S3-060, SC-S3-003, SC-S3-020, FR-S3-068, SC-S3-025.
**Design coverage:** MD08, MD09 (approved design §8.4).
**Named tests:** `private_package_admitted_by_local_authority`, `self_authorized_publisher_refuses`, `public_private_metadata_leak_refuses`, `indirect_private_dependency_refuses`, `missing_private_access_preserves_public`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Signed private-package source adapter and synthetic public/private CI/admission proof. Self-authorized publisher, metadata leakage, indirect public-private dependency and missing access refuse locally.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**URL-rule amendment (+0 h):** Private knowledge-source URL inventories and their exact rule closure stay in admitted private packages; publish no real private URL/metadata in public indexes or fixtures.

**Acceptance:** Signed private-package source adapter and synthetic public/private CI/admission proof. Self-authorized publisher, metadata leakage, indirect public-private dependency and missing access refuse locally.

### C70 Recover core personas review skill and handoff [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C02, C64.
**Files:** MAN core/agents/{maestro,reviewer,steward,bootstrapper}.{agent.md,maestro.toml}; skills/reviewing-changes/SKILL.md; core/handoffs/implementation-to-review/; docs/catalog/migration.md.
**Requirements:** FR-S3-001, FR-S3-018, FR-S3-026, FR-S3-055, SC-S3-009.
**Design coverage:** MD10 (approved design §8.4).
**Named tests:** `recovered_core_roles_have_native_sections`, `maestro_has_one_prompt_authority`, `recovered_handoff_keeps_independent_review`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Four authored core agents, Maestro prompt merge, review skill and handoff. Native shape/closure and author-reviewer separation cases pass.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Four authored core agents, Maestro prompt merge, review skill and handoff. Native shape/closure and author-reviewer separation cases pass.

### C71 Recover Translator and translation skill [US1, US5] (3 h)

**Phase:** P2/R1
**After:** C02, C64, C28.
**Files:** MAN capabilities/governance/content-translation/{package.toml,agents,skills}; docs/catalog/migration.md.
**Requirements:** FR-S3-001, FR-S3-026, FR-S3-041, FR-S3-042, SC-S3-009.
**Design coverage:** MD10 (approved design §8.4).
**Named tests:** `translation_references_are_owner_local`, `translator_grants_no_authority_or_runtime_localization`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Translator package, translation skill and two references; scoped links, no runtime localization or authority claims.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Translator package, translation skill and two references; scoped links, no runtime localization or authority claims.

### C72 Author missing framework roles [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C21b.
**Files:** MAN core/agents/{researcher,builder,releaser}.{agent.md,maestro.toml}; core/package.toml; docs/catalog/migration.md.
**Requirements:** FR-S3-001, FR-S3-018, FR-S3-019, FR-S3-026, SC-S3-009.
**Design coverage:** MD10 (approved design §8.4).
**Named tests:** `new_roles_are_not_placeholders`, `builder_is_deterministic_step`, `releaser_has_no_publication_grant`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Researcher/builder/releaser definitions beyond baseline delivery work; replace placeholder semantics, keep builder deterministic and release unprivileged.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Researcher/builder/releaser definitions beyond baseline delivery work; replace placeholder semantics, keep builder deterministic and release unprivileged.

### C73 Reconcile legacy instruction families [US1, US5] (4 h)

**Phase:** P2/R1
**After:** C02, C82a, C82b, C28.
**Files:** MAN standards/{engineering,security}/instructions/; docs/catalog/migration.md.
**Requirements:** FR-S3-001, FR-S3-026, FR-S3-057, SC-S3-009, SC-S3-018.
**Design coverage:** MD10 (approved design §8.4).
**Named tests:** `legacy_instruction_inventory_is_complete`, `stale_or_competing_rule_authority_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Two instruction families and thirteen references reconciled with current standards; conflicting/stale rule authority refuses review.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Two instruction families and thirteen references reconciled with current standards; conflicting/stale rule authority refuses review.

### C74a Recover path egress and tool Cedar rules [US1, US5] (4 h)

**Phase:** P2/R1
**After:** C21, C19, C81b, C28.
**Files:** MAN standards/security/{policies,checks}/; docs/catalog/migration.md.
**Requirements:** FR-S3-016, FR-S3-026, FR-S3-057, SC-S3-005, SC-S3-018.
**Design coverage:** MD10, MD14 (approved design §8.4).
**Named tests:** `recovered_path_egress_tool_allow_neighbours`, `recovered_path_egress_tool_denials`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Protected-path, egress and tool-permission YAML responsibilities to Cedar; each retained rule gets real allowed/denied neighbours.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Protected-path, egress and tool-permission YAML responsibilities to Cedar; each retained rule gets real allowed/denied neighbours.

### C74b Recover destructive and prompt-integrity obligations [US1, US5] (4 h)

**Phase:** P2/R1
**After:** C74a, C73, C28.
**Files:** MAN standards/security/{policies,instructions,checks}/; docs/catalog/migration.md.
**Requirements:** FR-S3-016, FR-S3-026, FR-S3-057, SC-S3-005, SC-S3-018.
**Design coverage:** MD10, MD14 (approved design §8.4).
**Named tests:** `legacy_rule_disposition_has_no_gaps`, `cmd_008_has_one_owner`, `unsupported_prose_is_not_claimed_enforced`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Destructive/prompt-integrity rules and complete semantic disposition; CMD-008 stays single-owned and unsupported claims remain explicit.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Destructive/prompt-integrity rules and complete semantic disposition; CMD-008 stays single-owned and unsupported claims remain explicit.

### C75 Convert six legacy hook documents [US1, US5] (3 h)

**Phase:** P2/R1
**After:** C62, C63, C74b, C28.
**Files:** MAN hooks/{pre-tool,prompt-submit,agent-stop}/; docs/catalog/migration.md.
**Requirements:** FR-S3-017, FR-S3-026, SC-S3-009.
**Design coverage:** MD05, MD10 (approved design §8.4).
**Named tests:** `six_hook_documents_have_dispositions`, `unsupported_hook_timing_is_explicit`, `hook_docs_never_claim_s3_execution`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Six hook-doc conversions, event placement and unsupported timing notes; no false ten-event coverage or S3 protection claim.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Six hook-doc conversions, event placement and unsupported timing notes; no false ten-event coverage or S3 protection claim.

### C76a Baseline and Rust Python Go profiles [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C65, C82a.
**Files:** MAN standards/quality/; languages/rust/profiles/quality/default.toml; languages/{python,go}/profiles/quality/; docs/catalog/migration.md.
**Requirements:** FR-S3-026, FR-S3-056, FR-S3-057, SC-S3-018.
**Design coverage:** MD10 (approved design §8.4).
**Named tests:** `baseline_and_three_profiles_keep_gate_categories`, `absent_quality_binding_is_unresolved`, `rust_seed_profile_is_extended_not_duplicated`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Recover the baseline and Python/Go quality profiles;
  extend C02's same Rust profile, `quality-profile:rust/default`, in place.
  Preserve its language reference and complete gate categories; add recovered
  rules/thresholds and unresolved binding cases, never a second Rust profile.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Baseline plus Rust/Python/Go quality profiles; C02 remains the
minimal Rust producer, C76a extends its same identity/file. Mandatory gates,
thresholds and unresolved bindings remain; recovery stays 4 h, with no duplicate
seed work and no C76a → C02 prerequisite cycle.

### C76b Five remaining language quality profiles [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C76a.
**Files:** MAN languages/{java,typescript,javascript,configuration-management,infrastructure-provisioning}/profiles/quality/; docs/catalog/migration.md.
**Requirements:** FR-S3-026, FR-S3-049, FR-S3-056, FR-S3-057, SC-S3-018.
**Design coverage:** MD10 (approved design §8.4).
**Named tests:** `eight_profiles_have_complete_categories`, `quality_identity_is_functional`, `historical_weaker_threshold_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Java/TypeScript/JavaScript/configuration-management/infrastructure-provisioning profiles; retain actual applicability and product-free identities.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Java/TypeScript/JavaScript/configuration-management/infrastructure-provisioning profiles; retain actual applicability and product-free identities.

### C77a Recover repository starter inventory [US1, US5] (4 h)

**Phase:** P2/R1
**After:** C50, C73, C28.
**Files:** MAN bootstrap/repository.toml and bootstrap/repository/files/; docs/catalog/migration.md.
**Requirements:** FR-S3-004, FR-S3-005, FR-S3-026, FR-S3-047, SC-S3-004.
**Design coverage:** MD04, MD10 (approved design §8.4).
**Named tests:** `repository_starter_has_39_retained_payloads`, `repository_language_stub_is_dropped`, `starter_json_is_strict_and_inert`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Repository's 40 starter payloads audited, one stub removed; inventory complete, generated JSON strict, no script runs.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Repository's 40 starter payloads audited, one stub removed; inventory complete, generated JSON strict, no script runs.

### C77b Recover Rust Python Go starters [US1, US5] (4 h)

**Phase:** P2/R1
**After:** C77a, C76a, C28.
**Files:** MAN languages/{rust,python,go}/bootstrap/; docs/catalog/migration.md.
**Requirements:** FR-S3-004, FR-S3-026, FR-S3-047, FR-S3-056, SC-S3-004.
**Design coverage:** MD04, MD10 (approved design §8.4).
**Named tests:** `three_language_starters_have_20_payloads`, `starter_binding_and_composition_refusals`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Rust/Python/Go starters: 20 payloads; full composition/collision/required-binding cases.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Rust/Python/Go starters: 20 payloads; full composition/collision/required-binding cases.

### C77c Recover Java and script-language starters [US1, US5] (4 h)

**Phase:** P2/R1
**After:** C77a, C76b, C28.
**Files:** MAN languages/{java,typescript,javascript}/bootstrap/; docs/catalog/migration.md.
**Requirements:** FR-S3-004, FR-S3-026, FR-S3-047, FR-S3-056, SC-S3-004.
**Design coverage:** MD04, MD10 (approved design §8.4).
**Named tests:** `three_language_starters_have_33_payloads`, `starter_workflow_remains_inert`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Java/TypeScript/JavaScript starters: 33 payloads; same composition and inert-workflow proof.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Java/TypeScript/JavaScript starters: 33 payloads; same composition and inert-workflow proof.

### C77d Recover automation and infrastructure starters [US1, US5] (3 h)

**Phase:** P2/R1
**After:** C77a, C76b, C28.
**Files:** MAN languages/{configuration-management,infrastructure-provisioning}/bootstrap/; docs/catalog/migration.md.
**Requirements:** FR-S3-004, FR-S3-026, FR-S3-047, FR-S3-049, FR-S3-056, SC-S3-004.
**Design coverage:** MD04, MD10 (approved design §8.4).
**Named tests:** `automation_starters_have_14_payloads`, `starter_native_filename_exception_is_exact`, `starter_never_activates_tools`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Configuration-management/infrastructure-provisioning starters: 14 payloads; required filename exceptions and no setup/activation.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Configuration-management/infrastructure-provisioning starters: 14 payloads; required filename exceptions and no setup/activation.

### C78 Descriptor-led scaffolder persona [US1, US5] (3 h)

**Phase:** P2/A0
**After:** C52c, C64, C65, C79a, C28.
**Files:** MAN core/agents/scaffolder.{agent.md,maestro.toml}; core/evals/scenarios/scaffolder/; templates/; core/package.toml.
**Requirements:** FR-S3-018, FR-S3-059, SC-S3-021.
**Design coverage:** MD16 (approved design §8.4).
**Named tests:** `scaffolder_authors_supported_kinds_with_tests`, `scaffolder_unknown_kind_refuses`, `scaffolder_cannot_self_approve`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Scaffolder persona and supported-kind authoring scenarios; output tests/evals, checkpoint evidence and refusal of unavailable kinds/self-approval.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Scaffolder persona and supported-kind authoring scenarios; output tests/evals, checkpoint evidence and refusal of unavailable kinds/self-approval.

### C79a Root language areas and manager choices [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C32, C65.
**Files:** crates/maestro-catalog/src/source/kinds/{language.rs,quality_profile.rs}; crates/maestro-catalog/src/source/tests/language.rs; tests/fixtures/catalog/languages/.
**Requirements:** FR-S3-040, FR-S3-041, FR-S3-042, FR-S3-046, FR-S3-049, FR-S3-056, FR-S3-057, SC-S3-015, SC-S3-018.
**Design coverage:** MD02, MD13 (approved design §8.4).
**Named tests:** `language_has_one_id_and_manager_default`, `duplicate_manager_default_refuses`, `language_product_path_refuses`, `language_cannot_weaken_standard`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Root language descriptor/profile contract, one language ID, manager default/alternatives and complete gate categories. Duplicate defaults, product paths or weakening a standard refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Root language descriptor/profile contract, one language ID, manager default/alternatives and complete gate categories. Duplicate defaults, product paths or weakening a standard refuse.

### C79b Eight complete language declarations [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C79a, C76b, C82b.
**Files:** MAN languages/{rust,python,typescript,javascript,java,go,configuration-management,infrastructure-provisioning}/{package.toml,instructions,bootstrap}; presets/rust-service.toml.
**Requirements:** FR-S3-004, FR-S3-047, FR-S3-056, FR-S3-057, SC-S3-004, SC-S3-018.
**Design coverage:** MD13 (approved design §8.4).
**Named tests:** `eight_language_areas_are_declared`, `rust_gate_pin_is_reused`, `nonrust_gate_gaps_are_explicit`, `minimal_rust_init_works`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Complete eight language profiles/instructions/CI-template declarations, Rust gate pin and truthful non-Rust gaps. No full gate implementation charged here.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Complete eight language profiles/instructions/CI-template declarations, Rust gate pin and truthful non-Rust gaps. No full gate implementation charged here.

### C80a Owners maintainers and protected delegation [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C32.
**Files:** crates/maestro-catalog/src/source/kinds/package.rs; crates/maestro-catalog/src/source/{ownership.rs,tests/ownership.rs}.
**Requirements:** FR-S3-041, FR-S3-045, FR-S3-058, SC-S3-015, SC-S3-018.
**Design coverage:** MD01, MD15 (approved design §8.4).
**Named tests:** `area_owners_maintainers_validate`, `resource_ownership_is_derived`, `broad_codeowners_rule_cannot_override_descriptor`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Owners/maintainers area schema, derived resource ownership and delegated CODEOWNERS semantics. Descriptor broad-rule override and self-delegation refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Owners/maintainers area schema, derived resource ownership and delegated CODEOWNERS semantics. Descriptor broad-rule override and self-delegation refuse.

### C80b Trusted identity and base-owner approval CI [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C80a, C35.
**Files:** crates/maestro-catalog/src/source/{owners.rs,tests/owners.rs}; crates/maestro/src/cli/catalog/owners.rs; MAN .github/workflows/check-owners.yml.
**Requirements:** FR-S3-003, FR-S3-013, FR-S3-041, FR-S3-045, FR-S3-058, SC-S3-018, SC-S3-021, FR-S3-068, SC-S3-025.
**Design coverage:** MD09, MD15 (approved design §8.4).
**Named tests:** `verified_existing_principals_accept`, `unknown_team_or_missing_access_refuses`, `stale_head_approval_refuses`, `newly_added_owner_cannot_self_approve`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Trusted CI principal-existence/base-owner approval checks. Unknown user/team, missing access, stale-head approval and newly self-added owner fail; no token to PR code.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**URL-rule amendment (+0 h):** Source-rule approvals bind the exact admitted head/digests under these existing owner/maintainer rules; metadata cannot assert its own reviewed admission.

**Acceptance:** Trusted CI principal-existence/base-owner approval checks. Unknown user/team, missing access, stale-head approval and newly self-added owner fail; no token to PR code.

### C81a Mandatory standards and registered checks [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C34.
**Files:** crates/maestro-catalog/src/source/kinds/{standard.rs,standard_check.rs}; crates/maestro-catalog/src/source/tests/standards.rs.
**Requirements:** FR-S3-037, FR-S3-040, FR-S3-042, FR-S3-046, FR-S3-057, SC-S3-015, SC-S3-018.
**Design coverage:** MD02, MD14 (approved design §8.4).
**Named tests:** `every_standard_is_pinned_once`, `missing_or_optional_standard_refuses`, `standard_removal_refuses`, `duplicate_rule_identity_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Standard/check descriptors and always-selected standard closure. Missing standard, opt-out/removal and duplicate rule identity refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Standard/check descriptors and always-selected standard closure. Missing standard, opt-out/removal and duplicate rule identity refuse.

### C81b Restrictive standards and central exceptions [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C81a, C19, C17.
**Files:** crates/maestro-catalog/src/source/{standards.rs,tests/standards.rs}; crates/maestro-catalog/src/policy/standards.rs.
**Requirements:** FR-S3-014, FR-S3-016, FR-S3-043, FR-S3-057, SC-S3-005, SC-S3-018.
**Design coverage:** MD04, MD14 (approved design §8.4).
**Named tests:** `standard_constraints_only_narrow`, `local_expired_or_wider_exception_refuses`, `nonnegotiable_exception_refuses`, `secret_scan_weakening_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Add-or-narrow standard constraints and central exceptions. Local/expired/wider exception, non-negotiable exception and weaker secret-scan settings refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Add-or-narrow standard constraints and central exceptions. Local/expired/wider exception, non-negotiable exception and weaker secret-scan settings refuse.

### C82a Pinned import of canonical organization rules [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C02, C81b, C35.
**Files:** MAN standards/{engineering,security,architecture,quality}/; docs/standards/; docs/catalog/migration.md.
**Requirements:** FR-S3-001, FR-S3-026, FR-S3-057, SC-S3-009, SC-S3-018.
**Design coverage:** MD10, MD14 (approved design §8.4).
**Named tests:** `org_import_matches_pinned_sources`, `normative_rules_and_local_observations_stay_distinct`, `imported_standard_drift_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Import org golden rules and engineering/northstar/security documents with rule/source pins; normative text and repo-local rule-map observations stay distinct.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Import org golden rules and engineering/northstar/security documents with rule/source pins; normative text and repo-local rule-map observations stay distinct.

### C82b Import four gate standards without a second gate [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C82a.
**Files:** MAN standards/ and languages/rust/; docs/catalog/migration.md; four source documents engineering.md, security.md, northstar.md, controls.md at the approved rust-workflows revision.
**Requirements:** FR-S3-026, FR-S3-056, FR-S3-057, SC-S3-009, SC-S3-018.
**Design coverage:** MD10, MD13, MD14 (approved design §8.4).
**Named tests:** `four_gate_standard_imports_have_pins`, `rust_gate_authority_is_unchanged`, `imported_rule_mapping_is_preserved`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Import four rust-workflows standards documents, preserve mappings and existing gate authority; no copied second Rust gate.
- [ ] **Step 3: Check.** Run the pinned `maestro catalog check --catalog-dir "$MANIFESTS"`
  and `catalog fixtures` plus affected generation checks; use `policy test`
  for Cedar and `catalog eval --all --offline` only after E1. Documentation-only
  changes run Markdown/link/drift checks. Record nonzero relevant case counts,
  exact exits and review; missing runtime evidence is unsupported, not passed.

**Acceptance:** Import four rust-workflows standards documents, preserve mappings and existing gate authority; no copied second Rust gate.

### C83a Verified standards input in rust-workflows [US1, US5] (4 h)

**Phase:** P2/ST1
**After:** C82a, C82b, C15, C28; coordinated owner-approved source-edit freeze; no independent manifest standards edits before ST1.
**Files:** RW existing standards-input, guide-generation and drift-check modules plus their tests; docs/standards/{engineering,security,northstar,controls}.md.
**Requirements:** FR-S3-051, FR-S3-057, SC-S3-018, SC-S3-021.
**Design coverage:** MD09, MD14 (approved design §8.4).
**Named tests:** `standard_render_uses_verified_pin`, `rendered_guide_is_reproducible`, `gate_floor_or_rule_id_drift_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Rust-workflows verified manifest-standard input/render adapter; preserve rule IDs and gate floors, reproducible guide/carry output and drift refusal.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Rust-workflows verified manifest-standard input/render adapter; preserve rule IDs and gate floors, reproducible guide/carry output and drift refusal.

### C83b Single-authority standards render switch [US1, US5] (4 h)

**Phase:** P2/ST1
**After:** C83a; coordinated owner-approved source-edit freeze; no independent manifest standards edits before ST1.
**Files:** ORG scripts/quality-sync.py and its existing tests; golden-rules/ and generated docs/standards/ mirrors; MAN standards authority metadata.
**Requirements:** FR-S3-051, FR-S3-057, SC-S3-018, SC-S3-021.
**Design coverage:** MD09, MD14 (approved design §8.4).
**Named tests:** `only_manifest_standards_are_editable`, `reverse_sync_refuses`, `coordinated_authority_switch_has_no_dual_writer`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Switch org quality-sync/guide generation to pinned standards; atomic authoring-authority cutover, read-only generated mirrors and reverse-sync refusal.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Switch org quality-sync/guide generation to pinned standards; atomic authoring-authority cutover, read-only generated mirrors and reverse-sync refusal.

### C84 Selectable starter catalog contracts [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C50, C51a, C52b, C79b.
**Files:** MAN marketplace/index.json; languages/<language>/contracts/starter-input.schema.json plus sidecars; bootstrap inventories; docs/catalog/templates.md; CORE crates/maestro-catalog/src/source/{index.rs,fixtures.rs,tests/index.rs,tests/fixtures.rs}.
**Requirements:** FR-S3-047, FR-S3-051, FR-S3-061, SC-S3-022.
**Gap holder:** G01; charged once, split only as listed in accounting.
**Named tests:** `starter_index_covers_each_eligible_inventory_once`, `starter_unknown_parameter_refuses`, `starter_missing_standard_or_stale_output_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Extend the existing index with exact input/output contract references and selector metadata; do not build another registry. Index every eligible declared inventory once; reject unknown parameters/selectors, missing standards and stale/extra/private rows. Check/install starts no generator.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Extend the existing index with exact input/output contract references and selector metadata; do not build another registry. Index every eligible declared inventory once; reject unknown parameters/selectors, missing standards and stale/extra/private rows. Check/install starts no generator.

### C85a Optional starter-renderer extension contract [US1, US5] (3 h)

**Phase:** P2/X1
**After:** C28, C58, C59, C84.
**Files:** MAN capabilities/practice/project-creation/package.toml; capabilities/practice/project-creation/extensions/starter-renderer/{extension.toml,contracts/,tests/}; generated .github/CODEOWNERS.
**Requirements:** FR-S3-053, FR-S3-061, SC-S3-020, SC-S3-023.
**Gap holder:** G02; charged once, split only as listed in accounting.
**Named tests:** `renderer_contract_requires_pinned_adapter`, `unavailable_renderer_stays_unsupported`, `renderer_is_never_bootstrap_script`, `renderer_missing_owning_package_refuses`, `renderer_missing_package_reference_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Declare `package:project-creation` with approved owners
  and `extension:project-creation/starter-renderer` in its `requires`; derive
  extension ownership from that `package.toml` and regenerate CODEOWNERS.
  Declare the optional out-of-process renderer and typed input/output contract
  using X1. Require a separately qualified real adapter before use; this task
  does not implement a renderer, install a runtime, run post-generation scripts
  or grant remote writes.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** The owning package and its exact extension reference check with
X1's renderer/input/output contract. Missing owner or extension reference refuses.
No renderer implementation, runtime install, post-generation scripts or remote
writes; a real adapter needs separate qualification. Ownership files are part of
the existing 3 h declaration task, not a new task or budget.

### C85b Isolated starter generation verifier [US1, US5] (3 h)

**Phase:** P2/E1
**After:** C85a, C67.
**Files:** MAN .github/workflows/verify-starters.yml; capabilities/practice/project-creation/extensions/starter-renderer/tests/; capabilities/practice/project-creation/evals/ synthetic cases.
**Requirements:** FR-S3-060, FR-S3-061, SC-S3-020, SC-S3-023.
**Gap holder:** G02; charged once, split only as listed in accounting.
**Named tests:** `synthetic_render_output_matches_expected`, `changed_render_output_fails`, `check_install_never_calls_renderer`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Use an isolated supported offline fixture driver to compare one synthetic rendered output with its pinned expectation. Generator and verifier are separate; stale output must fail. Absent real rendering support is labelled unsupported, not a passing production adapter.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Use an isolated supported offline fixture driver to compare one synthetic rendered output with its pinned expectation. Generator and verifier are separate; stale output must fail. Absent real rendering support is labelled unsupported, not a passing production adapter.

### C86 Consumer setup receipt extension [US1, US5] (4 h)

**Phase:** P2/L1
**After:** C28, C54, C55, C84.
**Files:** crates/maestro-catalog/src/install/{receipts.rs,tests/receipts.rs}; MAN core/contracts/consumer-setup.{schema.json,maestro.toml}; docs/catalog/consumer-setup.md.
**Requirements:** FR-S3-012, FR-S3-034, FR-S3-062, SC-S3-023.
**Gap holder:** G03; charged once, split only as listed in accounting.
**Named tests:** `consumer_inputs_reproduce_receipt_outputs`, `changed_consumer_parameters_require_preview`, `consumer_receipt_never_holds_secret_values`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Extend the existing receipt with inventory/template ID, source/generator pin, schema, normalized nonsecret inputs, output digests, standard snapshot and approved operation. Actual records remain external. Refuse literal secrets and undeclared tool/endpoint/grant additions; removal preserves edited outputs and consumer data.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Extend the existing receipt with inventory/template ID, source/generator pin, schema, normalized nonsecret inputs, output digests, standard snapshot and approved operation. Actual records remain external. Refuse literal secrets and undeclared tool/endpoint/grant additions; removal preserves edited outputs and consumer data.

### C87 Portable project enrollment plan [US1, US5] (4 h)

**Phase:** P2/L1
**After:** C86, C64.
**Files:** MAN capabilities/operations/project-onboarding/{package.toml,contracts,workflows,bootstrap}/.
**Requirements:** FR-S3-055, FR-S3-062, SC-S3-023.
**Gap holder:** G04; charged once, split only as listed in accounting.
**Named tests:** `synthetic_enrollment_bindings_validate`, `enrollment_cannot_create_grants_or_endpoints`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Provide typed environment/stack/tool-instance binding contracts, an existing-kind workflow and inert synthetic starter. Real consumer records stay outside manifests; no fleet registry, remote provisioning or enrollment service. Missing or unauthorized bindings refuse.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Provide typed environment/stack/tool-instance binding contracts, an existing-kind workflow and inert synthetic starter. Real consumer records stay outside manifests; no fleet registry, remote provisioning or enrollment service. Missing or unauthorized bindings refuse.

### C88a Phase evidence and artifact trace contracts [US1, US5] (4 h)

**Phase:** P2/R1
**After:** C28, C64.
**Files:** MAN core/contracts/{phase-evidence,artifact-trace}.{schema.json,maestro.toml}; contract fixtures.
**Requirements:** FR-S3-055, FR-S3-063, SC-S3-023.
**Gap holder:** G05; charged once, split only as listed in accounting.
**Named tests:** `phase_evidence_is_revision_bound`, `stale_artifact_trace_refuses`, `missing_blocking_approval_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Define stable phase/step IDs, revision-bound progress, pending prerequisites, blocking/advisory approvals, resumable state and requirement/control/test/artifact links. Require references/evidence, but never claim schema validation authenticates an approval.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Define stable phase/step IDs, revision-bound progress, pending prerequisites, blocking/advisory approvals, resumable state and requirement/control/test/artifact links. Require references/evidence, but never claim schema validation authenticates an approval.

### C88b Lifecycle phase and knowledge-selection kit [US1, US5] (3 h)

**Phase:** P2/R1
**After:** C88a, C21b, C66.
**Files:** MAN core/workflows/feature-delivery/phases/; workflow input inventory and cases.
**Requirements:** FR-S3-019, FR-S3-055, FR-S3-057, FR-S3-063, SC-S3-023.
**Gap holder:** G05; charged once, split only as listed in accounting.
**Named tests:** `lifecycle_phases_use_existing_graph`, `phase_missing_prerequisite_refuses`, `phase_cannot_omit_standard_or_invent_source`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Add inception/construction/operations assets inside the existing graph with stable steps and explicit knowledge-source requirements. No second workflow engine or fetch during checks. Required missing prerequisite/source evidence blocks its downstream phase.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Add inception/construction/operations assets inside the existing graph with stable steps and explicit knowledge-source requirements. No second workflow engine or fetch during checks. Required missing prerequisite/source evidence blocks its downstream phase.

### C88c Release-to-operations handoff and progress template [US1, US5] (3 h)

**Phase:** P2/R1
**After:** C88a, C88b, C52c.
**Files:** MAN core/handoffs/release-to-operations/; templates/workflow/; core/contracts/ trace fixtures.
**Requirements:** FR-S3-019, FR-S3-055, FR-S3-063, SC-S3-023.
**Gap holder:** G05; charged once, split only as listed in accounting.
**Named tests:** `operations_handoff_requires_readiness_evidence`, `progress_template_links_exact_revision`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Wire artifact trace and approval evidence to the handoff and inert progress template. Operations is readiness/runbook transfer, not a deployed operator. Reject stale or missing trace links; template output grants no runtime authority.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Wire artifact trace and approval evidence to the handoff and inert progress template. Operations is readiness/runbook transfer, not a deployed operator. Reject stale or missing trace links; template output grants no runtime authority.

### C89a One synthetic read-only lifecycle walkthrough [US1, US5] (3 h)

**Phase:** P2/R1
**After:** C88c.
**Files:** MAN docs/examples/lifecycle/; skills/walkthrough/SKILL.md; explicit owner-local assets.
**Requirements:** FR-S3-063, FR-S3-064, SC-S3-023.
**Gap holder:** G06; charged once, split only as listed in accounting.
**Named tests:** `walkthrough_labels_each_phase_honestly`, `walkthrough_has_no_live_service_calls`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Author one bounded synthetic walkthrough from scratch with complete/partial/unavailable phase labels and read-only replay instructions. Do not copy mocked source content or add interactive training/live sandbox provisioning.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Author one bounded synthetic walkthrough from scratch with complete/partial/unavailable phase labels and read-only replay instructions. Do not copy mocked source content or add interactive training/live sandbox provisioning.

### C89b Walkthrough replay evaluation [US1, US5] (3 h)

**Phase:** P2/E1
**After:** C89a, C67.
**Files:** MAN evals/{scenarios,data}/walkthrough/; eval TOML and input inventory.
**Requirements:** FR-S3-060, FR-S3-063, SC-S3-023.
**Gap holder:** G06; charged once, split only as listed in accounting.
**Named tests:** `walkthrough_expected_replay_matches`, `partial_phase_cannot_count_as_qualified`, `fabricated_trace_evidence_fails`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Use a supported registered offline driver and exact synthetic assets/expectations. Refuse fabricated IDs, stale evidence and missing phases; a successful content replay cannot qualify actual workflow execution.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Use a supported registered offline driver and exact synthetic assets/expectations. Refuse fabricated IDs, stale evidence and missing phases; a successful content replay cannot qualify actual workflow execution.

### C90 Catalog governance navigation [US1, US5] (2 h)

**Phase:** P1/M3
**After:** C80b, C82b.
**Files:** MAN docs/governance/{ownership,contributing,security-reporting}.md; docs/standards/ links.
**Requirements:** FR-S3-058, FR-S3-064, SC-S3-022.
**Gap holder:** G07; charged once, split only as listed in accounting.
**Named tests:** `governance_links_resolve`, `guide_cannot_create_second_authority`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Explain decision ownership, maintainer delegation, contribution, vulnerability reporting and the import/render transition. Link canonical rules/descriptors, do not duplicate editable normative content or invent owner identities.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Explain decision ownership, maintainer delegation, contribution, vulnerability reporting and the import/render transition. Link canonical rules/descriptors, do not duplicate editable normative content or invent owner identities.

### C91 Consumer and contributor setup guides [US1, US5] (3 h)

**Phase:** P1/M3
**After:** C54, C55, C63.
**Files:** MAN docs/setup/{installation,prerequisites,contributing,mcp,recovery}.md.
**Requirements:** FR-S3-010, FR-S3-035, FR-S3-036, FR-S3-054, FR-S3-064, SC-S3-022.
**Gap holder:** G08; charged once, split only as listed in accounting.
**Named tests:** `setup_commands_match_registered_cli`, `mcp_auth_guidance_uses_references`, `recovery_preserves_owned_files`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Document installation/prerequisites/contributor setup, MCP authentication/pre-flight and recovery through existing checked commands and receipts. No second installer, automatic tool setup or embedded credential values. Label unsupported hosts/features honestly.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Document installation/prerequisites/contributor setup, MCP authentication/pre-flight and recovery through existing checked commands and receipts. No second installer, automatic tool setup or embedded credential values. Label unsupported hosts/features honestly.

### C92 Release notes and canonical roadmap navigation [US1, US5] (2 h)

**Phase:** P1/M3
**After:** C15, C51a.
**Files:** MAN `docs/releases/<actual-version>.md`; `docs/roadmap.md`.
**Requirements:** FR-S3-013, FR-S3-052, FR-S3-064, SC-S3-022.
**Gap holder:** G09; charged once, split only as listed in accounting.
**Named tests:** `release_note_binds_actual_signed_identity`, `roadmap_links_canonical_tasks`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Tie notes to the actual released identity, not an invented version/digest. A missing release blocks that publication input, not fixture preparation. Maintain one roadmap linking the task ledger; do not copy independent task lists.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Tie notes to the actual released identity, not an invented version/digest. A missing release blocks that publication input, not fixture preparation. Maintain one roadmap linking the task ledger; do not copy independent task lists.

### C93 Catalog reader index [US1, US5] (1 h)

**Phase:** P1/M3
**After:** C84, C90, C91, C92, C94.
**Files:** MAN docs/README.md; docs/catalog/by-type.md generation cross-links.
**Requirements:** FR-S3-051, FR-S3-064, SC-S3-021, SC-S3-022.
**Gap holder:** G10; charged once, split only as listed in accounting.
**Named tests:** `reader_index_links_contracts_guides_and_kinds`, `reader_navigation_drift_fails`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Join guides, contracts, examples, registered kinds and unsupported states in one reader entry point over generated indexes. Link/drift checks reject stale or broken destinations, without a second hand-maintained component registry.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Join guides, contracts, examples, registered kinds and unsupported states in one reader entry point over generated indexes. Link/drift checks reject stale or broken destinations, without a second hand-maintained component registry.

### C94 Redacted guard-decision evidence contract [US1, US5] (4 h)

**Phase:** P1/M3
**After:** C62, C64.
**Files:** MAN core/contracts/guard-evidence.{schema.json,maestro.toml}; docs/catalog/events.md; evals/scenarios/guard-outcomes/; contract fixtures.
**Requirements:** FR-S3-017, FR-S3-055, FR-S3-065, SC-S3-022.
**Gap holder:** G11; charged once, split only as listed in accounting.
**Named tests:** `guard_outcomes_are_distinct`, `guard_evidence_rejects_raw_secret_or_command`, `warning_is_not_enforcement_proof`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Define refusal/warning/confirmation-required output evidence attributed to rule/tool/session with synthetic redacted cases. It is not another hook-event protocol/point. Live mediation, trusted attribution, publication, retention and delivery remain S4.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Define refusal/warning/confirmation-required output evidence attributed to rule/tool/session with synthetic redacted cases. It is not another hook-event protocol/point. Live mediation, trusted attribution, publication, retention and delivery remain S4.

### C95a Per-host context accounting contract [US1, US5] (3 h)

**Phase:** P2/A1
**After:** C28, C63, C65.
**Files:** crates/maestro-catalog/src/source/{host_config.rs,tests/host_config.rs}; MAN core/hosts/<host>/config.toml; docs/catalog/context.md.
**Requirements:** FR-S3-051, FR-S3-066, SC-S3-024.
**Gap holder:** G12; charged once, split only as listed in accounting.
**Named tests:** `host_context_fields_are_typed`, `context_ceiling_requires_approved_config`, `neutral_projection_evidence_has_no_core_dependency`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Implement the reserved typed descriptor fields for approved aggregate injected-context/description ceilings and neutral projection evidence. Do not copy source numbers or claim tokenizer/model latency. Missing approved limits cannot be treated as measured support.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Implement the reserved typed descriptor fields for approved aggregate injected-context/description ceilings and neutral projection evidence. Do not copy source numbers or claim tokenizer/model latency. Missing approved limits cannot be treated as measured support.

### C95b Static context load check and refusals [US1, US5] (3 h)

**Phase:** P2/A1
**After:** C95a, C81b.
**Files:** crates/maestro-catalog/src/hosts/{context.rs,tests/context.rs}; MAN standards/quality/checks/context-load.toml; projection fixtures.
**Requirements:** FR-S3-057, FR-S3-066, SC-S3-024.
**Gap holder:** G12; charged once, split only as listed in accounting.
**Named tests:** `context_includes_generated_descriptions`, `context_lf_crlf_counts_match`, `stable_rule_references_deduplicate`, `context_overflow_refuses_without_weakening`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Count generated descriptions and reviewed always-loaded selections, deduplicate stable rule references and test LF/CRLF equivalence. Standard check consumes neutral evidence; overflow refuses/requires reviewed restructuring, never removes a standard or disables scanning.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Count generated descriptions and reviewed always-loaded selections, deduplicate stable rule references and test LF/CRLF equivalence. Standard check consumes neutral evidence; overflow refuses/requires reviewed restructuring, never removes a standard or disables scanning.

### C96a Portable code-graph declaration kit [US1, US5] (3 h)

**Phase:** P2/R1
**After:** C28, C64, C66.
**Files:** MAN capabilities/practice/code-analysis/{package.toml,skills/build-code-graph/SKILL.md,contracts,knowledge/sources/code-graph/source.toml}.
**Requirements:** FR-S3-055, FR-S3-067, SC-S3-024.
**Gap holder:** G13; charged once, split only as listed in accounting.
**Named tests:** `code_graph_contract_has_identity_and_provenance`, `code_graph_unknown_source_subtype_refuses`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Define code-graph/indexing-input contracts and an explicit recipe with source revision/digest, node/edge identity, extractor, relation/confidence/provenance and exclusions. Product implementation is a value. Reuse supported source shapes; no new ingester or read authorization.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Define code-graph/indexing-input contracts and an explicit recipe with source revision/digest, node/edge identity, extractor, relation/confidence/provenance and exclusions. Product implementation is a value. Reuse supported source shapes; no new ingester or read authorization.

### C96b Synthetic code-graph freshness and refusal cases [US1, US5] (3 h)

**Phase:** P2/R1
**After:** C96a, C67.
**Files:** MAN capabilities/practice/code-analysis/{evals,contracts}/; explicit synthetic graph assets.
**Requirements:** FR-S3-060, FR-S3-067, SC-S3-024.
**Gap holder:** G13; charged once, split only as listed in accounting.
**Named tests:** `changed_graph_source_is_stale`, `graph_missing_provenance_refuses`, `malformed_graph_relation_refuses`, `graph_check_makes_zero_build_fetch_import_calls`; required cases, not reported results.

- [ ] **Step 1: Red.** Add the named passing/refusal neighbours for the acceptance
  below; run the focused fixture/check command and retain the failure.
- [ ] **Step 2: Green.** Add exact synthetic fresh/stale/malformed/unknown-config cases through the existing driver. Keep full generated graphs, caches and repositories external; zero fetch/build/import on check. This proves declarations only, not code-graph ingestion or live freshness.
- [ ] **Step 3: Check.** Run the affected source/bootstrap/CLI test filters through
  capped nextest; MAN uses the pinned `catalog check` and fixture/drift commands.
  Record nonzero test counts, exact exits, applicable lane gates and review.

**Acceptance:** Add exact synthetic fresh/stale/malformed/unknown-config cases through the existing driver. Keep full generated graphs, caches and repositories external; zero fetch/build/import on check. This proves declarations only, not code-graph ingestion or live freshness.

## Dependencies & Execution Order

Each **After** field begins with the exact internal predecessor list; text after
its semicolon names external evidence, not hidden C-task edges. All pending
work obeys the v4 phase labels. Closed task edges retain historical target
weights for whole-plan comparison; new C46/C47 wiring never reopens them.

C44 → C30 → C31 → C32 → C33, joined with C80a at C34, then C50 → C36 → C37
and C35 join C39. C02 additionally waits for C51a/C52b, not deferred package-new,
extensions or generic evals. It supplies the minimal Rust profile and MCP base;
C76a extends the profile. C08 additionally joins C47b's MCP consumer wiring, not
C48 or G25-qualified backend publication. C15 joins Phase 1 checkpoint wiring C68.
C29 audits C68 plus selected Phase 1 recovery/imports **before C28**, no longer after it.
C28 joins every Phase 1 task and existing external live/release/quality evidence.
C20, C41–C43 and all Phase 2 tasks cannot enter the M3 ancestor closure.
C66 separately needs the owning core source-policy types synchronized, including
N07's IdentityMigration after review fixes land. The S6 runtime catalog adapter
is an external handoff, not an M3 gate; it additionally needs C69 for real private
packages. C41/C43 consume C66's rules rather than define collection-local rules.

The S2 sequence is qualification → C48 → G22 → C49a, never C48↔G22.
Existing S2 additions remain G01 1 h (after C44), G26 1 h (after C46/G25/G01),
G27 E07a 2 h (after C47a/G01/fork/G25/E04), E08b 1 h (reader/writer/publication),
G28 1 h (after C47a/graph prerequisites), G22 1 h (after C48/release inputs).
S2 keeps 37 G tasks/14 E slices, 149 h bounded G + 43–89 h E = 192–238 h.
C49a also requires E11/G28; qualified pins are execution inputs, never invented.

Phase 2 begins immediately after M3. ST1 precedes independent standards edits;
X1 requires its shared E1 eval checkpoint before extension publication/S4 use.
A0 adds package-new/scaffolder; R1 adds deferred recovery/content kits; L1 adds
independent/private lifecycle; A1 adds explain/context accounting. No new
numbered project milestone or second layout migration is implied. Pulling a
Phase 2 task forward needs explicit approval, its real prerequisites and a
recomputed cost/exit gate, not merely deleting its C28 phase-boundary edge.

### Critical paths and effort

All sums include completed work at historical weights; they are not remaining
hours. C03 retains its approved 6 h exception; all new implementation tasks are
at most 4 h. C20 remains visible only as a 4 h S4 transfer, not a S3 task/saving.

| Whole-plan boundary | Tasks | Hours |
| --- | ---: | ---: |
| Pre-amendment S3 baseline | 68 | 218–230 |
| Retained S3 baseline after C20 transfer, including C02 +2 h | 67 | 216–228 |
| Design §8.4 task IDs, including the later C66 +1 h | 57 | 204 |
| Gap holders, both phases | 19 | 58 |
| **S3 Phase 1/M3** | **108** | **357–369** |
| **S3 Phase 2** | **35** | **121** |
| **S3 both phases** | **143** | **478–490** |
| Deferred S6 C41–C43 | 3 | 6–10 |
| S3 + S6 | 146 | 484–500 |
| Transferred C20, S4 | 1 | 4 |
| All task headings (S3 + S6 + transferred S4) | 147 | 488–504 |

| Amendment reconciliation | Phase 1 h | Phase 2 h | Both h |
| --- | ---: | ---: | ---: |
| Original design new tasks (35 P1, 22 P2) | 124 | 79 | 203 |
| Owner 20:45 URL rules, C66 3→4 h | 1 | 0 | 1 |
| Existing C02 v4 seed adjustment, 2→3 h | 1 | 0 | 1 |
| Already approved S2 delta | 7 | 0 | 7 |
| Gap additions (6 P1 tasks, 13 P2 tasks) | 16 | 42 | 58 |
| Approved amendment before validation fixes | 149 | 121 | 270 |
| C02 validation fix, 3→4 h: Rust profile/reference, MCP base and refusal cases | 1 | 0 | 1 |
| C08/C21/C21b/C44/C48/C76a/C85a validation fixes within existing budgets | 0 | 0 | 0 |
| **Amendment after validation fixes** | **150** | **121** | **271** |

The design's original Phase 2 40 h plus minimal-option deferrals 39 h = 79 h;
gaps add 42 h = 121 h. The exact minimal deferrals are C52c+C78 authoring 6 h,
C71 Translator 3 h, C73 instructions 4 h, C74a/b policies 8 h, C75 hooks 3 h,
C77a–d starters 15 h: **39 h**, no work disappears. The design's 132 h minimal
Phase 1 plus gap 16 h was **148 h**; the owner's 20:45 URL-rule amendment
adds C66 1 h, making the approved **149 h**. C02's validation fix adds **1 h**
for the conforming Rust profile/reference, MCP base and refusal cases: **150 h**
Phase 1. C08 adds only dependency edges; C21/C21b sidecars, C85a ownership,
C76a/C48 reuse and C44 architecture corrections add **0 h**. No recovery hour
is removed or charged twice. C52a/b, C68 and C41/C43 add 0 h by reusing the same
type/schema/checkpoint contracts; the S6 runtime adapter is separately tasked
and not estimated here. Against the earlier approved 27 h engine amendment
this is **244 h extra**, not another 271 h stacked on 27 h.

| Gap obligation | Holder tasks | Phase/milestone | h |
| --- | --- | --- | ---: |
| G01 | C84 | P1/M3 | 4 |
| G02 | C85a, C85b | P2/X1, P2/E1 | 6 |
| G03 | C86 | P2/L1 | 4 |
| G04 | C87 | P2/L1 | 4 |
| G05 | C88a, C88b, C88c | P2/R1 | 10 |
| G06 | C89a, C89b | P2/R1, P2/E1 | 6 |
| G07 | C90 | P1/M3 | 2 |
| G08 | C91 | P1/M3 | 3 |
| G09 | C92 | P1/M3 | 2 |
| G10 | C93 | P1/M3 | 1 |
| G11 | C94 | P1/M3 | 4 |
| G12 | C95a, C95b | P2/A1 | 6 |
| G13 | C96a, C96b | P2/R1 | 6 |

G01 and G07–G11 are the six Phase 1 obligations (4 + 2 + 3 + 2 + 1 + 4 = 16 h).
G12 is Phase 2 per-host context accounting, not part of that 12 h guide/guard
bundle. Split gaps retain one obligation and disjoint bounded deliverables;
no index, selector, receipt, workflow engine or fixture framework is charged twice.

| Checkpoint | Dependency-closure effort | Internal longest path |
| --- | --- | --- |
| C08 owner loop | 191–203 h (57 tasks) | 66–74 h |
| C28 M3 | 357–369 h (108 tasks) | 79–86 h |
| C43 S6 contract handoff | 70–84 h (24 tasks) | 42–53 h |

Lower-bound M3 chain (task weights included):

```text
C00 → C01 → C03 → C03a → C31 → C32 → C80a → C34 → C50 → C36 → C37 → C39 → C10 → C11 → C12 → C13 → C14 → C18 → C16 → C16h → C24a → C24 → C26 → C28
```

Before validation fixes, C08 had 54 tasks / 178–190 h and a 62–70 h path.
Adding C47b adds exactly C46/C47a/C47b (12 h); C02 adds 1 h, yielding 57 tasks /
191–203 h and a 66–74 h path. C48 and C76a remain outside that closure; C02
supplies their seed inputs. M3 gains only C02's 1 h and retains its 79–86 h
path. The values above are recomputed from the final edges; do not reuse an
older path or divide it by lane count. External G25/native qualification, M1,
hosts, owner credentials/approval, C66 wire-type synchronization (including
reviewed N07), CI and shared-file serialization are excluded.
The graph is acyclic with no Phase 2/S6/S4 ancestor of M3. The supervisor
approved adding C64 to C62's predecessors: the hook-layer common-prompt fixture
needs C64's real prompt/contract descriptors. This corrects design §8.4 without
changing costs, closures or the C08/M3 paths above.

The unchanged 16–24 h baseline CI/review reserve gives **494–514 h for S3**
or **500–524 h with S6**; do not add C39's included migration review twice.
S3 both phases + approved S2 192–238 h + transferred C20 4 h = **674–732 h**,
excluding S6 and the reserve. This is 244 h above the engine-approved combined
430–488 h. Seven later full non-Rust gate projects total **168–280 h** outside
these numbers; each needs its own approved task plan. S4 execution beyond C20,
real connectors/renderers, live model/router work and S6 crawling remain separate.

### Requirements coverage

Regenerated from every task's Requirements field; ranges are expanded exactly.
The same inverse appears in traceability.json. Phase-specific success criteria
are not all M3 gates: Phase 2/S6/S4 evidence stays at its named checkpoint.

| Requirement | Tasks |
| --- | --- |
| FR-S3-001 | C00, C03, C03a, C02, C02a, C21, C21b, C29, C70, C71, C72, C73, C82a |
| FR-S3-002 | C01, C03, C03a, C02, C32, C45a, C52a, C52b, C58, C60, C62, C64, C66 |
| FR-S3-003 | C03, C02, C02a, C22a, C21, C21b, C15, C80b |
| FR-S3-004 | C05, C36, C50, C77a, C77b, C77c, C77d, C79b |
| FR-S3-005 | C04a, C04, C05, C05j, C50, C52c, C55, C77a |
| FR-S3-006 | C01, C06, C07, C40, C47b, C63 |
| FR-S3-007 | C02, C06, C07, C08 |
| FR-S3-008 | C09, C10, C47a, C53a, C54 |
| FR-S3-009 | C11, C16, C47a, C53a, C54 |
| FR-S3-010 | C09, C13, C13a, C16, C54, C59, C69, C91 |
| FR-S3-011 | C09, C14, C18, C16, C16h, C16b, C24a, C24, C27, C49a, C49b, C54, C59, C69 |
| FR-S3-012 | C12, C16, C16b, C49b, C54, C55, C59, C69, C86 |
| FR-S3-013 | C13, C13a, C15, C54, C67, C68, C69, C80b, C92 |
| FR-S3-014 | C03, C17, C05b, C46, C47a, C48, C57, C60, C65, C81b |
| FR-S3-015 | C18, C16h, C46, C47a, C49a, C54, C57 |
| FR-S3-016 | C09, C19, C22b, C21, C20, C61, C64, C74a, C74b, C81b |
| FR-S3-017 | C20, C28, C62, C63, C75, C94 |
| FR-S3-018 | C21, C21b, C70, C72, C78 |
| FR-S3-019 | C00, C22a, C22b, C21, C21b, C72, C88b, C88c |
| FR-S3-020 | C23, C24 |
| FR-S3-021 | C24a, C24 |
| FR-S3-022 | C14, C24a, C24, C26 |
| FR-S3-023 | C25 |
| FR-S3-024 | C00, C26 |
| FR-S3-025 | C27a, C27 |
| FR-S3-026 | C28, C29, C70, C71, C72, C73, C74a, C74b, C75, C76a, C76b, C77a, C77b, C77c, C77d, C82a, C82b |
| FR-S3-027 | C17, C05a, C05j, C05g, C08, C46 |
| FR-S3-028 | C05b, C08, C46, C47a, C57 |
| FR-S3-029 | C05c, C05l, C05d, C08, C46 |
| FR-S3-030 | C05g, C08, C05f, C05k, C46, C56 |
| FR-S3-031 | C05e, C06, C07, C08, C46 |
| FR-S3-032 | C16c, C16e, C16g, C16f, C54 |
| FR-S3-033 | C17, C05a, C16c, C16e, C16g, C16f, C54 |
| FR-S3-034 | C16d, C16e, C16f, C54, C86 |
| FR-S3-035 | C05h, C05j, C05g, C08, C91 |
| FR-S3-036 | C05h, C05i, C05j, C05e, C06, C07, C08, C16d, C16f, C20, C91 |
| FR-S3-037 | C03, C10, C11, C12, C16, C45a, C52a, C52b, C58, C64, C65, C66, C81a |
| FR-S3-038 | C03a, C02a, C18, C52b |
| FR-S3-039 | C03a, C02a, C18, C16h |
| FR-S3-040 | C02, C38, C30, C31, C41, C44, C79a, C81a |
| FR-S3-041 | C02, C38, C32, C34, C35, C44, C52c, C53a, C71, C79a, C80a, C80b |
| FR-S3-042 | C02, C22a, C22b, C38, C33, C34, C36, C44, C45b, C47b, C62, C71, C79a, C81a |
| FR-S3-043 | C38, C33, C44, C45b, C49b, C55, C59, C81b |
| FR-S3-044 | C38, C42, C44, C69 |
| FR-S3-045 | C38, C35, C44, C80a, C80b |
| FR-S3-046 | C02, C38, C30, C31, C32, C33, C34, C37, C44, C51a, C79a, C81a |
| FR-S3-047 | C02, C38, C36, C37, C44, C45b, C47a, C50, C54, C55, C56, C77a, C77b, C77c, C77d, C79b, C84 |
| FR-S3-048 | C38, C41, C43, C44 |
| FR-S3-049 | C30, C44, C63, C76b, C77d, C79a |
| FR-S3-050 | C02, C44, C45a, C45b, C46, C47a, C47b, C48, C49a, C49b, C59 |
| FR-S3-051 | C02, C28, C44, C51a, C52a, C52b, C67, C68, C83a, C83b, C84, C93, C95a |
| FR-S3-052 | C44, C51a, C51b, C53a, C53b, C54, C55, C56, C58, C69, C92 |
| FR-S3-053 | C44, C58, C59, C61, C85a |
| FR-S3-054 | C44, C57, C58, C59, C60, C61, C66, C91 |
| FR-S3-055 | C44, C52b, C58, C62, C64, C66, C70, C87, C88a, C88b, C88c, C94, C96a |
| FR-S3-056 | C02, C28, C44, C65, C76a, C76b, C77b, C77c, C77d, C79a, C79b, C82b |
| FR-S3-057 | C28, C44, C65, C73, C74a, C74b, C76a, C76b, C79a, C79b, C81a, C81b, C82a, C82b, C83a, C83b, C88b, C95b |
| FR-S3-058 | C28, C35, C44, C68, C80a, C80b, C90 |
| FR-S3-059 | C44, C52c, C78 |
| FR-S3-060 | C44, C67, C69, C85b, C89b, C96b |
| FR-S3-061 | C28, C44, C84, C85a, C85b |
| FR-S3-062 | C44, C86, C87 |
| FR-S3-063 | C44, C88a, C88b, C88c, C89a, C89b |
| FR-S3-064 | C28, C44, C89a, C90, C91, C92, C93 |
| FR-S3-065 | C28, C44, C94 |
| FR-S3-066 | C44, C95a, C95b |
| FR-S3-067 | C44, C96a, C96b |
| FR-S3-068 | C28, C41, C43, C44, C52a, C52b, C66, C68, C69, C80b |
| SC-S3-001 | C01, C06, C07, C08, C28, C40 |
| SC-S3-002 | C10, C15, C16, C28, C53a, C54 |
| SC-S3-003 | C11, C12, C13, C13a, C14, C15, C16, C16h, C16b, C28, C49b, C53a, C54, C55, C69 |
| SC-S3-004 | C02, C04a, C04, C05, C06, C07, C08, C28, C36, C37, C50, C55, C77a, C77b, C77c, C77d, C79b |
| SC-S3-005 | C03, C17, C19, C22a, C22b, C18, C20, C28, C61, C64, C74a, C74b, C81b |
| SC-S3-006 | C23, C24a, C24, C26, C28 |
| SC-S3-007 | C25, C26, C27a, C27, C28 |
| SC-S3-008 | C28, C39 |
| SC-S3-009 | C00, C28, C29, C38, C39, C44, C70, C71, C72, C73, C75, C82a, C82b |
| SC-S3-010 | C05a, C05b, C05j, C05c, C05l, C05d, C05e, C05g, C06, C07, C08, C05f, C05k, C16d, C28, C46, C47a, C56, C57 |
| SC-S3-011 | C16c, C16d, C16e, C16g, C16f, C28, C54 |
| SC-S3-012 | C05h, C05i, C05j, C05g, C06, C07, C08, C16d, C16f, C20, C28 |
| SC-S3-013 | C03, C10, C11, C12, C16, C28, C31, C52a, C52b |
| SC-S3-014 | C03a, C11, C18, C16, C16h, C28, C52b |
| SC-S3-015 | C28, C30, C31, C33, C34, C35, C39, C42, C44, C62, C63, C79a, C80a, C81a |
| SC-S3-016 | C28, C32, C33, C34, C36, C37, C39, C40, C44, C47b, C50, C63 |
| SC-S3-017 | C41, C42, C43 |
| SC-S3-018 | C28, C35, C44, C65, C73, C74a, C74b, C76a, C76b, C79a, C79b, C80a, C80b, C81a, C81b, C82a, C82b, C83a, C83b |
| SC-S3-019 | C28, C44, C45a, C45b, C46, C47a, C47b, C48, C49a, C49b, C54, C55, C60 |
| SC-S3-020 | C44, C51b, C53b, C56, C58, C59, C61, C67, C69, C85a, C85b |
| SC-S3-021 | C02, C28, C44, C51a, C52a, C52b, C52c, C57, C62, C63, C64, C66, C67, C68, C78, C80b, C83a, C83b, C93 |
| SC-S3-022 | C28, C44, C84, C90, C91, C92, C93, C94 |
| SC-S3-023 | C44, C85a, C85b, C86, C87, C88a, C88b, C88c, C89a, C89b |
| SC-S3-024 | C44, C95a, C95b, C96a, C96b |
| SC-S3-025 | C28, C41, C43, C44, C52a, C52b, C66, C68, C69, C80b |

## Implementation Strategy

1. Finish C08 before treating the platform as useful. It exercises one explicit
   preset and the existing M1 answerer, without waiting for routing or an engine.
2. Keep the trusted install boundary closed until verification, freshness and
   atomic records are ready. Internal tested seams are not unsigned CLI features.
3. Add policies/settings/graphs only as needed by the two workflows; finish
   declarative checks before publishing their compiled closure.
4. Measure routing against frozen labels; retain the simpler passing route.
   Do not let synthetic qualification fixtures authorize live roles.
5. Close M3 only with observed release, host, S2 and full CI evidence. Audit
   the approved recovery ledger before M3; start the named Phase 2 work after it.

## Needs owner action

The [plan's owner-action register](plan.md#needs-owner-action) is authoritative;
only its explicitly approved scope authorizes assigned lane operations.
Remaining owner actions are blockers or handoffs, not implied permissions.
The C00 inventory/hook approval is recorded separately, dated 2026-09-28.
OA7 below names only M3 acceptance and eventual main release.

| Action group | Register | Tasks waiting |
| --- | --- | --- |
| Repository created (README/MIT only); supply owners/protections and approve public model-card winner changes | OA1 and evening card decision | OA1 gates MAN landing: C02, C02a, C21, C21b, C15. Winner approval gates only C02a and its C28 acceptance, never C15/C21, C23 or CORE consumers |
| **Approved 2026-09-28:** probe/test already-installed Copilot CLI, Pi, Claude Code and Codex, each in an isolated temporary home; C01 pins exact installed versions. No installs/upgrades, real owner configuration or enterprise policy changes; broader scope needs fresh approval | OA2 | Bounded C01, C06, C07, C08, C28 host tests; C20 is S4; missing access still blocks and OA6 data approval remains separate |
| Bind publishers and standalone pinned `gh` with repository-bound read-only fine-grained authentication; authorized maintainer handles any unlisted licence | OA4 | C09's publisher-row evidence closure and C28, not verifier/dependency implementation |
| Publish compiler/catalog; enable six-hour trust attestations/hourly alerts; supply clean environment and drills; authorize private/model access | OA5, OA6 | C28 release proof; C08/C23/C26 only for the requested private/model access |
| Accept M3 and eventual main release; authorized pre-M3 recovery; any S2 fallback separately | OA7, OA8 | C28, C29 |
| **Approved 2026-09-28:** ratatui + crossterm under ADR-0020; C05f measurements/vet still required, branded visual acceptance pending; no parser dependency | OA9 | C05k after measurement, C28 after visual evidence; never first plain C08 |
| **Approved by the owner, 2026-09-28:** amend D5's absolute routing bar to held-out matchable top-1 ≥ 90 %; the original top-3 ≥ 90 % bar is historical | OA10 | Quality target resolved; C24/C26 apply the top-1 gate and report both metrics; C28 verifies the amended bar before final quality acceptance |

Record blocked/not-run when any input or external proof is absent. Do not
create a repository, change organization settings, publish, enable workflows,
install a client or send private material as a side effect of a coding task.
