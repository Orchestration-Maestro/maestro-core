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
startup updates, user-approved path trust and small replaceable ports. Research,
data model and contracts are in the plan.

**Format:** `Cnn [USn] title (hours)`, with red/green/check steps. IDs preserve
the approved draft, with explicit review splits. Physical order below is
dependency order, not numeric order. **53 tasks, 177 lane-hours**, each at most
four hours including local checks; review/CI reserve is separate. All checkboxes
start open: a plan is not implementation evidence. Only the supervisor ticks
integrated work.

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
  secret. Keep private receipts at the owner-approved private location. Never
  open the earlier catalog before C29. No task may expand its ≤4 h budget by
  weakening scope, validation or tests; split a discovered overrun explicitly.

### Paths and checks

`CORE` is maestro-core. `MAN` means the separate, owner-created
maestro-manifests checkout; a task marked MAN commits there only. All other
paths are relative to CORE. Brace lists name exact files, not directory-wide
permission to refactor. Tests live beside source or in the existing `it`
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
dispatch, migration registration and the generated Copilot guide. Do not create
a new command/module before its first working behaviour.

## Phase 1: First useful content, preferences and owner loop [US1, US3, US5]

### C00 Contract and traceability [US1, US2, US3, US4, US5] (2 h)

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

- [ ] **Step 1: Red.** Derive S3 candidates from 08 source tables; fail if a
  candidate is neither included nor explicitly excluded with a reason. Reject
  missing, duplicate or extra exact keys in the JSON and spec, missing/different
  portions or tasks, and lost/unnamed remaining slices; prove each mutation
  fails its own check. Task IDs must match actual C-number task headings.
- [x] **Step 2: Green.** Freeze exact keys/S3 portions, approved by the owner,
  2026-09-28 (OA7). In 03/06/08 record D1's authoring boundary, D5's synthetic
  conditional hybrid result, the 08:12 parallel start and S3 static/S4 runtime split. For
  `product.GD2, GD4, GD5`, keep four-client MCP and Copilot `preToolUse` in S3;
  defer Pi/Codex/Claude Code hooks to S4 host-adapter qualification because their
  trusted event/identity adapters are unqualified. Name C27a's catalog schema/
  adapters and S2 G27's public port. Coordinate next-free migrations above all
  landed/reserved numbers across main and S1/S2/S3, never a fixed/gapped block.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-conventions catalog_traceability`,
  `rumdl check specs/003-catalog`, and offline links for the edited Markdown.

**Acceptance:** every exact row has a disposition, no false delivered status,
no open D1–D5 choice, and architecture agrees on routing, hook scope, impact
ownership and checkpoints. The frozen inventory is approved by the owner,
2026-09-28 (OA7). External operations are not run.

**C00 evidence boundary:** [traceability.json](traceability.json) and the
conventions check contain 85 included exact rows with planned portions and
named remainders, plus six explicit S5-only exclusions. This inventory is
approved by the owner, 2026-09-28 (OA7); fixture checks are not M3 delivery
evidence. Architecture 03/06/08 records the decided contract.

### C01 Real host format probe [US1, US5] (3 h)

**After:** C00; OA2 approved on 2026-09-28 for already-installed tools in isolated
temporary homes only. No installs/upgrades, real-configuration or enterprise
policy changes; anything broader needs fresh approval.
**Files:** `crates/maestro/tests/it/catalog_host_probe.rs`,
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
  skill mapping. If C03 already fixed sidecars for v1, qualify that shape without
  silently switching it; a later metadata migration is a separate reviewed task.
  Do not mask unsupported fields.
- [ ] **Step 3: Check.** Run the explicit live `catalog_host_probe` cases:

  ```sh
  ~/.local/bin/capped cargo test -p maestro --test it catalog_host_probe -- \
    --ignored --nocapture
  ```

  Keep synthetic receipts in the research note, with unsupported/not-run distinct.

**Acceptance:** one evidence-backed authoring shape and both host mappings;
a missing client/provider/account blocks the probe, not the rest of the plan.
No new extension and no implicit model fallback.

### C03 Strict source checker [US1, US5] (4 h)

**After:** C00's recorded contract; no host-access prerequisite.
**Files:** `crates/maestro-catalog/{Cargo.toml,src/lib.rs}`,
`crates/maestro-catalog/src/source/{mod.rs,types.rs,parse.rs,check.rs,tests.rs}`,
`crates/maestro/src/cli/catalog/{mod.rs,check.rs}`,
`crates/maestro/tests/it/catalog_check.rs`,
`tests/fixtures/catalog/source/{valid.agent.md,valid.maestro.toml,invalid.agent.md,preset.toml}`,
`specs/003-catalog/research/hosts.md`;
workspace/CLI registration files from the shared list.
**Requirements:** FR-S3-002, FR-S3-003, SC-S3-005.

- [ ] **Step 1: Red.** Add passing-neighbour and refusal fixtures for duplicate
  IDs/keys, unknown keys/types, oversized/deep input, missing body sections,
  dangling references, dependency cycles and executable draft closures. Reject
  unknown, unclassified and doubly classified setting keys at this boundary.
- [ ] **Step 2: Green.** Freeze the authoring shape before coding: if C01 is
  unavailable, ADR-0005 sidecars become final for v1. Record that decision; a
  later metadata switch needs its own task. Add the first checker/CLI using
  installed parsers, measured byte/depth/resource limits and precise path/key
  diagnostics. Unimplemented graph/policy features are unsupported, never
  silently accepted as fully checked.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog source::tests` and
  `~/.local/bin/capped cargo nextest run -p maestro catalog_check`.

**Acceptance:** valid sources round-trip; every invalid neighbour fails;
checking executes no template/script. Fixed agent sections are Purpose,
Responsibilities, Inputs, Working sequence, Outputs and Boundaries.

### C02 Minimal catalog seed [US1, US5] (2 h)

**After:** C00, C03; OA1 repository and ownership established by the owner.
**Files (MAN):** `agents/base/maestro.agent.md`,
`skills/knowledge-evidence/SKILL.md`, `instructions/knowledge.instructions.md`,
`mcp/maestro.toml`, `presets/{knowledge-client.toml,rust-service.toml}`,
`bootstrap/base/.github/copilot-instructions.md`,
`bootstrap/rust/.github/instructions/rust.instructions.md`,
`bootstrap/rust/.maestro/recipes.json`, `settings/classes.toml`, `CODEOWNERS`,
`README.md`. If C03's frozen v1 format uses sidecars, add only the corresponding
`agents/base/maestro.maestro.toml` and
`skills/knowledge-evidence/knowledge-evidence.maestro.toml` metadata files.
**Requirements:** FR-S3-001, FR-S3-002, FR-S3-003, FR-S3-007.

- [ ] **Step 1: Red.** Run C03's check against a seed missing its required
  knowledge skill/reference and an unclassified setting; retain the refusals.
- [ ] **Step 2: Green.** Write only the knowledge preset, Maestro and the
  knowledge resources `ctm-question` will need, then the Rust overlay. Attach
  owners, honest maturity, source-row keys and explicit MCP tools; generate
  CODEOWNERS from the resource ownership model, not a competing owner list.
- [ ] **Step 3: Check.** Run
  `"$MAESTRO_BIN" catalog check --catalog-dir "$MANIFESTS"`; validate
  `recipes.json` as strict JSON. C08 runs C01's real probes on this seed; their
  unavailability does not block checked content preparation.

**Acceptance:** a useful generic seed passes without reading earlier content;
no vendor text, credentials, empty roles, executable workflow activation or
unqualified-role claim. Release CI arrives in C15 with a released compiler.

### C04a Share the existing ADR-0018 filesystem [US1] (2 h)

**After:** C00.
**Files:** `crates/maestro-filesystem/{Cargo.toml,src/lib.rs}`,
`crates/maestro-filesystem/src/{root.rs,unix.rs,windows.rs}`;
move from `crates/maestro-canonicalization/src/filesystem/{mod.rs,root.rs,unix.rs,windows.rs}`;
`crates/maestro-canonicalization/{Cargo.toml,src/lib.rs,src/store.rs}`,
`crates/maestro-canonicalization/src/tokenizer/artifacts.rs`;
workspace manifest/lock and guide registration from the shared list.
**Requirements:** FR-S3-005, SC-S3-004.

- [ ] **Step 1: Red.** Run the existing filesystem/store/tokenizer tests as a
  baseline; change consumer imports to the intended shared crate and capture
  the missing-crate failure before moving implementation. Keep existing
  assertions and fixture bytes unchanged.
- [ ] **Step 2: Green.** Move the module and its tests to `maestro-filesystem`,
  adapting only imports, visibility and workspace wiring. Move existing
  target-specific dependencies, not new libraries. Preserve store/tokenizer
  integration regressions and all ADR-0018 behavior; no catalog logic here.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-filesystem` and
  `~/.local/bin/capped cargo nextest run -p maestro-canonicalization`;
  compare test assertions/fixtures and run three-target Clippy.

**Acceptance:** one shared implementation, unchanged security behavior and
regression assertions; canonicalization no longer owns a private copy.

### C04 Shared owned-file operations [US1] (4 h)

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

**After:** C03.
**Files:** `crates/maestro-catalog/src/settings/{mod.rs,classes.rs,resolve.rs,tests.rs}`,
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
  only in kernel authority, never preferences.
- [ ] **Step 2: Green.** Reuse C03's checked class declarations and add
  resolution/provenance: explicit flags > workspace file > user-level config > built-in defaults.
  Apply update-consent ceilings and budget minima before free-value precedence,
  explaining ignored widenings. Presets are init seeds only; permissions
  intersect, checks accumulate and values stay role-local. Reject unsupported
  profile effort. The small resolver consumes typed layers, not storage or UI.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog settings::tests`.

**Acceptance:** every effective value has one class/source/requester; ordering
search results cannot change configuration or grant a permission.

### C05 Preview and apply project bootstrap [US1] (3 h)

**After:** C03, C04; CORE fixtures, not the owner-created seed.
**Files:**
`crates/maestro-catalog/src/bootstrap/{mod.rs,inspect.rs,compose.rs,project.rs,tests.rs}`,
`crates/maestro/src/cli/init.rs`, `crates/maestro/tests/it/catalog_init.rs`,
`tests/fixtures/catalog/bootstrap/{knowledge-client.toml,rust-service.toml}`,
`tests/fixtures/catalog/bootstrap/base/.github/copilot-instructions.md`,
`tests/fixtures/catalog/bootstrap/rust/.maestro/recipes.json`;
CLI registration files from the shared list.
**Requirements:** FR-S3-004, FR-S3-005, SC-S3-004.

- [ ] **Step 1: Red.** Test base and base-plus-Rust composed outputs, every
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

**After:** C05, C17; D6's complete BCP 47 subset needs no dependency or OA9.
**Files:** `crates/maestro-catalog/src/settings/{preferences.rs,preference_files.rs}`,
`crates/maestro-catalog/src/settings/tests/preferences.rs`,
`crates/maestro/src/cli/init.rs`, `crates/maestro/tests/it/catalog_preferences.rs`,
`tests/fixtures/catalog/settings/{workspace.toml,user-preferences.toml}`;
necessary module/CLI registrations only.
**Requirements:** FR-S3-027, FR-S3-033, SC-S3-010.

- [ ] **Step 1: Red.** Test full/partial files, absent language, canonical
  en/fr/es/ja, zh-Hant-TW, es-419 and sr-Latn; reject variants, extensions,
  private-use, grandfathered forms, malformed tags and non-ASCII input. Test all
  tones/update modes, duplicate/unknown keys, types, versions, size and forbidden
  access/identity/hooks/secrets. Both preference files reject trust, paths and
  receipts; workspace auto is an ignored widening. Preview/cancel create no
  files and changed config bytes survive rerun.
- [ ] **Step 2: Green.** Implement D6's strict schema and file adapter behind
  `WorkspacePreferences`. Both files hold preferences only. Parse the exact
  bounded tag grammar without a new library; pass only canonical tags onward.
  Add language, tone, narrowing update choices and documented overrides to
  init's side-effect-free draft; absence of language remains distinct from en.
  Do not enable config persistence before C05h/C05j supply the real trust guard;
  this task produces the validated C04 write plan, not an approval stub. Keep
  user authority config untouched; the draft contains no machine-local paths.
- [ ] **Step 3: Check.** Run capped nextest filters `settings::tests::preferences`
  in maestro-catalog and `catalog_preferences` in maestro; parse the generated
  TOML and compare scripted and draft-plan bytes.

**Acceptance:** one strict schema/port and file adapter, no raw option escape
hatch or authority field. Init plans selected preferences at its own
root; C05j owns guarded persistence, never into an ancestor/user authority file.
New storage adapters do
not change configuration consumers.

### C05b Every-session discovery and precedence [US1, US3] (3 h)

**After:** C05a, C17; reads existing kernel journal records, not config claims.
**Files:** `crates/maestro-catalog/src/settings/{discovery.rs,resolve.rs}`,
`crates/maestro-filesystem/src/{unix.rs,windows.rs}` (held-handle owner/write
metadata only, keeping ADR-0018's module layout after C04a),
`crates/maestro-catalog/src/settings/tests/discovery.rs`,
`crates/maestro/src/cli/{args.rs,run.rs}`, `crates/maestro/src/mcp/run.rs`,
`crates/maestro/tests/it/catalog_session_preferences.rs`.
**Requirements:** FR-S3-014, FR-S3-028, SC-S3-010.

- [ ] **Step 1: Red.** Test home-bounded and explicitly trusted external-root
  discovery, nested configs, missing config, links and ancestor swaps. Plant
  valid and invalid foreign-owned/other-writable files, unreadable candidates,
  a /tmp parent and a /mnt/c-style parent, with safe user-owned neighbours.
  Skips warn, never parse or block startup; malformed selected safe files refuse.
  Outside home without journal trust reads no config. Cover four-layer conflicts,
  absent language, masked invalid keys, explicit/default flags, ignored ancestors,
  workspace auto over user propose, user off and budget-only narrowing. No grant
  reconciliation runs; explicit MCP --workspace uses these same checks.
- [ ] **Step 2: Green.** Resolve one session snapshot through C05a's port and
  C17, including provenance. Wire every CLI invocation and MCP startup before
  effects. Reuse ADR-0018 reads with held-handle uid/mode or owner SID/DACL checks
  on directory and file; never select mount/drive roots. MCP without --workspace
  uses only user preferences, never cwd/roots. Preserve platform config-home
  resolution; snapshots do not watch files and tool arguments cannot replace them.
- [ ] **Step 3: Check.** Run capped nextest filters `settings::tests::discovery`
  and `catalog_session_preferences`; restart the process after an edit and
  assert that only the new session sees the changed preference. Run planted-file
  and owner/write-check cases on all three CI hosts, not Unix-only mocks.

**Acceptance:** free values use explicit flags > workspace file > user-level
config > built-in defaults; update consent/budgets only narrow. No hidden preset
layer, ancestor merge or policy widening. CLI/MCP
consume the same port; replacing file storage does not change their resolution.

### C05h User-approved workspace trust records [US1, US3] (3 h)

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
Pi/Codex/Claude tools; C20 covers Copilot and the named S4 obligation covers the rest.

### C05c Interface message port and English logs [US1, US3] (4 h)

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

**After:** C05b, C05c; existing S1 answer ports, not live calibration approval.
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

**After:** C05a, C05b, C05h, C05i, C05j, C05c; no TUI or OA9 dependency.
**Files:** `crates/maestro/src/cli/init/{mod.rs,flow.rs,plain.rs}`
(move C05's adapter without duplicating its planner),
`crates/maestro/src/cli/init/tests/{mod.rs,flow.rs}`,
`crates/maestro/tests/it/catalog_init_menu.rs`, `docs/how-to/catalog.md`.
**Requirements:** FR-S3-027, FR-S3-030, FR-S3-035, SC-S3-010, SC-S3-012.

- [ ] **Step 1: Red.** Test D6's five steps as sequential labelled prompts,
  language/tag/tone, narrowing update choice, trust yes/no, Back, errors and
  Ctrl-C/EOF. Assert flags/plain plan parity, zero preview/cancel writes, --yes
  without prompts and fresh-home explicit trust setup; --yes never grants trust.
- [ ] **Step 2: Green.** Implement the small flow port and plain adapter over
  one draft/validator/planner. No raw mode, alternate screen or new library.
  Reuse localized messages and fallback note. --apply remains necessary;
  missing scripted approval gives the exact user trust command, not a prompt.
- [ ] **Step 3: Check.** Run capped nextest `catalog_init_menu` and flow tests on
  three hosts; retain a screen-reader/plain/no-color walkthrough and script parity.

**Acceptance:** the first owner loop has a complete accessible flow without waiting on TUI evidence.
C05k later replaces the renderer, not preferences, trust or owned-file planning.

### C06 Copilot projection and shared JSON ownership [US1] (4 h)

**After:** C01, C04, C05e, C05j; OA2 host-operation approval.
**Files:** `crates/maestro-catalog/src/hosts/{mod.rs,copilot.rs,shared_json.rs}`,
`crates/maestro-catalog/src/hosts/tests/{mod.rs,copilot.rs,shared_json.rs}`,
`crates/maestro/src/cli/catalog/project.rs`,
`crates/maestro/tests/it/catalog_copilot.rs`.
**Requirements:** FR-S3-006, FR-S3-007, FR-S3-031, FR-S3-036,
SC-S3-001, SC-S3-004, SC-S3-010, SC-S3-012.

- [ ] **Step 1: Red.** In an isolated home, test native discovery, exact tools,
  same-name shadowing, unrelated MCP entries, edited owned entries, rerun,
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
  `~/.local/bin/capped cargo nextest run -p maestro catalog_copilot` and repeat
  the C01 real parser/MCP call on the resulting isolated projection.

**Acceptance:** idempotent real discovery, preference-fragment delivery and
owned-only removal; user MCP entries survive and no missing tool/model is hidden.
The native delivery adapter shares the fragment/WorkspaceTrust ports, and tests
prove an outside-trust write is refused rather than silently granting a folder.

### C07 Pi projection [US1] (3 h)

**After:** C01, C04, C05e, C05j, C06; OA2 host-operation approval.
**Files:** `crates/maestro-catalog/src/hosts/pi.rs`,
`crates/maestro-catalog/src/hosts/tests/pi.rs`,
`crates/maestro/tests/it/catalog_pi.rs`; register Pi in the existing projector.
**Requirements:** FR-S3-006, FR-S3-007, FR-S3-031, FR-S3-036,
SC-S3-001, SC-S3-004, SC-S3-010, SC-S3-012.

- [ ] **Step 1: Red.** Test exact tools and skills, missing MCP adapter/provider,
  unsupported fields, restart/reload, shadowing, edited content and removal in
  an isolated Pi home; prove tool names alone cannot pass the MCP-call check.
  Assert the shared fixed English/MCP-guidance fragment and path-policy port.
  Across en/fr/es/ja × three tones, all projected files are byte-identical and
  contain no stored language/tone values.
- [ ] **Step 2: Green.** Project only C01's supported profile with explicit
  provider loading; reuse C05e's static rule/native delivery port,
  C05j's controlled write gate, the installed adapter and C04. No extension install,
  silently dropped field or inherited/fallback model.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro catalog_pi` and the C01 real
  Pi MCP probe after restarting the projected host.

**Acceptance:** provider-backed tools work, preference instructions are delivered
through the shared port, missing support is named, and unrelated/edited resources
survive. Maestro's projection writes enforce trust; this does not claim Pi tool
containment, whose hook obligation remains S4.

### C08 First owner loop [US1] (2 h)

**After:** C02, C05a, C05b, C05c, C05d, C05e, C05g, C05h, C05i, C05j, C05l,
C06, C07; integrated T034/T035 and completed T038 live evidence;
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
  project preferences. Preserve unavailable/uncalibrated markers and private
  receipts; fix only integration defects within this flow. When C05k is available,
  repeat the walkthrough for OA9 visual acceptance, never gate this first loop on it.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro catalog_owner_loop` and the
  authorized live flow; record client/provider versions and all observed states.

**Acceptance:** first useful checkpoint, not M3. A client-like test alone is
insufficient. Re-estimate remaining work from the actual 70-hour-checkpoint results.

## Phase 2: Menu, settings, policy and required content [US1, US3, US5]

### C05f Terminal dependency measurements [US1] (2 h)

**After:** C05; independent of the first owner loop. OA9 approved ratatui +
crossterm on 2026-09-28; ADR-0020 measurements still precede adoption.
**Files:** `specs/003-catalog/research/interface.md`; scratch manifests only.
**Requirements:** FR-S3-030, SC-S3-010.

- [ ] **Step 1: Red.** Define a terminal probe for keyboard/focus, resize,
  Ctrl-C cleanup, plain output and no-color behavior on the three platforms.
- [ ] **Step 2: Green.** Measure minimum ratatui/crossterm features, added
  crates, duplicates, native links, licences and vet needs. Keep the prototype
  disposable; no parser work or production dependency adoption in this task.
- [ ] **Step 3: Check.** Run the probe and capped `cargo tree -e features` /
  `cargo tree -d` on scratch manifests; record results and exact OA9 decision.

**Acceptance:** reproducible measurement; only the optional renderer waits for
adoption evidence. The plain flow and preference parser have no new dependency.

### C05k Branded terminal renderer [US1, US3] (4 h)

**After:** C05g, C05f, C05c; OA9 ratatui/crossterm approval recorded 2026-09-28,
not a C08 prerequisite. Measurement/vet requirements remain.
**Files:** `crates/maestro/src/cli/init/{terminal.rs,mod.rs}`,
`crates/maestro/src/cli/init/tests/terminal.rs`,
`crates/maestro/tests/it/catalog_init_menu.rs`, `docs/how-to/catalog.md`;
approved dependency/feature/vet registrations only.
**Requirements:** FR-S3-030, SC-S3-010.

- [ ] **Step 1: Red.** Add five-screen snapshots/key sequences, Tab/arrows,
  Back, invalid-field focus, resize, Ctrl-C/EOF, cleanup and no-color cases.
  Assert renderer/plain/script plan parity and zero unconfirmed writes.
- [ ] **Step 2: Green.** Plug the approved renderer into C05g's flow port.
  Apply D6's palette, hierarchy, focus and accessible fallback; never duplicate
  draft validation or file effects. Restore the terminal on every exit.
- [ ] **Step 3: Check.** Run terminal/menu tests on all three hosts, measure
  contrast and retain keyboard/plain/no-color walkthrough plus OA9 visual
  acceptance during the repeated C08 flow; screenshot-only proof is insufficient.

**Acceptance:** the owner's polished menu target is met before C28. A renderer
can be replaced without changing preferences, trust or planning; plain C08 does
not wait for this task or its library decision.

### C09 Dependency and trust measurements [US2, US3, US5] (3 h)

**After:** C00; D4 already approves the required libraries. Existing public
attested artifacts supply real verification fixtures; no publisher-setup gate.
**Files:** `specs/003-catalog/research/{dependencies.md,trust.md}`,
`docs/architecture/08-traceability.md`,
`tests/fixtures/catalog/trust/{valid.json,wrong-signer.json}`,
`maestro-quality.toml`, `supply-chain/audits.toml` as approved;
use scratch manifests for measurements, not speculative production dependencies.
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

**After:** C03, C09's measured dependencies under the decided D4.
**Files:** `crates/maestro-catalog/src/policy/{mod.rs,check.rs,schema.rs}`,
`crates/maestro-catalog/src/policy/tests/{mod.rs,check.rs}`,
`crates/maestro/src/cli/policy.rs`, `crates/maestro/tests/it/catalog_policy.rs`,
`tests/fixtures/catalog/policy/{schema.json,rules.cedar,cases.json}`;
dependency/CLI registrations from the shared list.
**Requirements:** FR-S3-016, SC-S3-005.

- [ ] **Step 1: Red.** Write allow/deny fixtures before policies for default
  deny, destructive effects, protected paths, egress and MCP tools. Include
  schema errors, missing trusted facts, evaluation errors and zero discovered
  tests; a spy must see zero executor calls on refusal.
- [ ] **Step 2: Green.** Wire the real Cedar evaluator/schema and policy
  check/test CLI. Inspect all diagnostics, deny on errors and keep approval-needed
  distinct from permission granted. Checking never executes the requested effect.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog policy::tests` and
  `~/.local/bin/capped cargo nextest run -p maestro catalog_policy`.

**Acceptance:** every rule's stimulus reaches real Cedar; allowed neighbours
pass, denied/error neighbours deny, and model text cannot supply trusted facts.

### C22a Graph topology [US5] (3 h)

**After:** C03, C17, C19.
**Files:** `crates/maestro-catalog/src/graph/{mod.rs,types.rs,topology.rs}`,
`crates/maestro-catalog/src/graph/tests/{mod.rs,topology.rs}`,
`tests/fixtures/catalog/graphs/{topology-valid.md,topology-invalid.md}`.
**Requirements:** FR-S3-003, FR-S3-019, SC-S3-005.

- [ ] **Step 1: Red.** Cover missing/ineligible references, unreachable nodes,
  paths without terminals, unbounded cycles/maps/subgraph depth and impossible
  reviewer independence, including a mandatory reviewer omitted from closure.
- [ ] **Step 2: Green.** Build exact dependency closures and topology checks
  for rules 1, 2, 3, 6 and 9 of architecture 03 §2.3. Reject unknown constructs;
  do not build a scheduler, engine or general plugin graph framework.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog graph::tests::topology`.

**Acceptance:** each covered rule has a valid/invalid pair; bounded repair
loops and independent reviewers pass without treating source labels as evidence.

### C22b Graph contracts [US5] (4 h)

**After:** C22a, C19; C09's measured JSON Schema validator approval.
**Files:** `crates/maestro-catalog/src/graph/{contracts.rs,conditions.rs,state.rs}`,
`crates/maestro-catalog/src/graph/tests/contracts.rs`,
`tests/fixtures/catalog/graphs/{contracts-valid.md,contracts-invalid.md}`;
wire full checks into `source/check.rs`.
**Requirements:** FR-S3-016, FR-S3-019, SC-S3-005.

- [ ] **Step 1: Red.** Test condition types, exact router edges, tool-policy
  coverage, required sandbox, budgets, read-before-write and a successful path
  missing its output, plus valid neighbours and hostile condition text.
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

**After:** C02, C17, C19, C22b.
**Files (MAN):** `workflows/ctm-question/workflow.md`,
`contracts/answer.schema.json`, `profiles/models/maestro.toml`,
`policies/{default-deny,destructive-operations,protected-paths}.cedar`,
`policies/{egress-deny-by-default,mcp-allowlist}.cedar`,
`policies/schema.cedarschema.json`, `hooks/pre-tool-use.json`,
`settings/classes.toml`, `evals/scenarios/{ctm-question,policy-neighbours}.yaml`,
`CODEOWNERS`. Discovery cards are metadata of the exact resources, not authority.
**Requirements:** FR-S3-001, FR-S3-003, FR-S3-016, FR-S3-018, FR-S3-019.

- [ ] **Step 1: Red.** Write missing knowledge-skill/answer-contract cases and
  all five production policies' allowed/denied neighbours; require real checker
  and evaluator refusals before completing the content.
- [ ] **Step 2: Green.** Author `ctm-question`, its answer contract and shared
  policies/hook. Declare Maestro fast/balanced/deep profiles for both providers;
  retain unsupported/unqualified status without S4 evidence. Reuse C02's skill,
  instructions and MCP descriptor rather than duplicating them.
- [ ] **Step 3: Check.** Run `"$MAESTRO_BIN" catalog check --catalog-dir "$MANIFESTS"`
  and `"$MAESTRO_BIN" policy test --catalog-dir "$MANIFESTS"` on production content.

**Acceptance:** the knowledge workflow's exact references, ownership/08 rows,
policy neighbours and graph checks pass; no invented qualification or vendor text.

### C21b Feature-delivery workflow [US3, US5] (3 h)

**After:** C21.
**Files (MAN):** `workflows/feature-delivery/workflow.md`,
`agents/base/{planner,coder,tester,reviewer}.agent.md`,
`contracts/{plan,patch,test-report,review,delivery}.schema.json`,
`profiles/models/{planner,coder,tester,reviewer}.toml`,
`skills/{spec-compliance,security-review}/SKILL.md`,
`evals/scenarios/feature-delivery.yaml`, `CODEOWNERS`.
For C03's sidecar format add only corresponding
`agents/base/{planner,coder,tester,reviewer}.maestro.toml` and
`skills/{spec-compliance/spec-compliance,security-review/security-review}.maestro.toml`.
**Requirements:** FR-S3-001, FR-S3-003, FR-S3-018, FR-S3-019.

- [ ] **Step 1: Red.** Require refusal for a missing role/contract, removed
  mandatory reviewer, self-review, missing approval gate and invented role
  qualification, with a valid complete feature-delivery neighbour.
- [ ] **Step 2: Green.** Author only the four required roles, two skills,
  contracts and declarative workflow; builder is a deterministic step. Declare
  fast/balanced/deep for both providers, marking absent S4 support unsupported.
  Reuse C21's policies and preserve any imported approval obligations.
- [ ] **Step 3: Check.** Run `"$MAESTRO_BIN" catalog check --catalog-dir "$MANIFESTS"`
  and the feature-delivery scenarios through its graph/contract checks.

**Acceptance:** both v1 workflows have exact complete closures, required
independent reviews and honest qualification states; no unused role is added.

## Phase 3: Trusted distribution and explanation [US2, US3]

### C10 Deterministic bundle compiler [US2, US5] (3 h)

**After:** C03, C09, C22b; compile CORE fixtures, not owner-published content.
**Files:** `crates/maestro-catalog/src/bundle/{mod.rs,manifest.rs,write.rs}`,
`crates/maestro-catalog/src/bundle/tests/{mod.rs,write.rs}`,
`crates/maestro/src/cli/catalog/compile.rs`,
`crates/maestro/tests/it/catalog_compile.rs`;
measured tar dependency registration from the shared list.
**Requirements:** FR-S3-008, SC-S3-002.

- [ ] **Step 1: Red.** Reorder inputs and change mtimes/permissions; require
  identical bytes. Change one definition and require a new digest. Include
  executable-looking templates/hooks/scripts that would leave a marker if run.
- [ ] **Step 2: Green.** Compile checked sources into sorted tar with fixed
  metadata and `bundle.json`, exact closures, owners/maturity, source commit,
  policy digest, runtime/features/tool contracts and entry points.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog bundle::tests::write`
  and the `catalog_compile` process suite; compare two produced bundle digests.

**Acceptance:** deterministic bytes, changed-input sensitivity and no script
marker; authoring and bundle schemas are separate and versioned.

### C11 Hostile bundle reader [US2] (4 h)

**After:** C10.
**Files:** `crates/maestro-catalog/src/bundle/{read.rs,compatibility.rs}`,
`crates/maestro-catalog/src/bundle/tests/read.rs`.
**Requirements:** FR-S3-009, SC-S3-003.

- [ ] **Step 1: Red.** Build hostile archive fixtures for duplicate, traversal,
  link, device, undeclared and oversized entries, bad schema/digest, truncation,
  unknown features, incompatible runtime and unsupported tool-contract versions.
- [ ] **Step 2: Green.** Revalidate the normalized schema and every entry under
  bounded reads before returning a verified shape. Do not extract arbitrary
  paths first or treat the author's successful compile as validation.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog bundle::tests::read`.

**Acceptance:** every invalid bundle is refused with no partial installation
or writes outside staging; compatibility fails before activation.

### C12 Scoped install records [US2] (4 h)

**After:** C11; migration numbers above every migration landed or reserved
on `main`, `feat/s1-integration`, `feat/s2-integration` and
`feat/s3-integration`, assigned next free at landing by the supervisor in
coordination with the other slices.
**Files:** `crates/maestro-kernel/src/catalog/{mod.rs,records.rs,store.rs}`,
`crates/maestro-kernel/src/catalog/tests/{mod.rs,installs.rs,pins.rs}`,
`crates/maestro-kernel/migrations/NNNN_catalog.sql`,
`crates/maestro-kernel/src/store/migration.rs`,
`crates/maestro-catalog/src/install/{mod.rs,record.rs}`,
`crates/maestro-catalog/src/install/tests/{mod.rs,records.rs}`.
`NNNN` is the next free number assigned at landing, the only intentional path
binding awaiting integration coordination; do not edit an applied migration.
**Requirements:** FR-S3-012, SC-S3-003.

- [ ] **Step 1: Red.** Test scoped invisibility, reopen, concurrent installs,
  atomic activation/rollback, exact component closure, artifact pin accounting
  and backup inclusion; corrupt references must prevent activation.
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

**After:** C09, C11; use C09's real public artifact/identity fixtures.
**Files:** `crates/maestro-catalog/src/trust/{mod.rs,attestation.rs,verifier.rs}`,
`crates/maestro-catalog/src/trust/tests/{mod.rs,attestation.rs,verifier.rs}`,
`crates/maestro/src/cli/health/doctor.rs`,
`crates/maestro/src/cli/health/tests/catalog_verifier.rs`,
`docs/how-to/catalog.md`.
**Requirements:** FR-S3-010, SC-S3-003.

- [ ] **Step 1: Red.** Test byte tamper, wrong signer/repository/workflow/issuer/
  source, forged subject digest, missing verifier, output overflow, hang and
  nonzero exit; pair these with real valid signature evidence from C09. Make
  doctor fail readiness on absent/wrong-version/wrong-digest `gh` or missing
  online authentication, without downloading a binary or starting a login.
- [ ] **Step 2: Green.** Invoke pinned `gh` with fixed argv and bounded
  time/stdout/stderr; kill/reap on timeout and bind authenticated claims to the
  locally hashed exact bytes. Never trust exit zero without matching claims.
  Extend doctor with pin/auth diagnostics and document the owner's standalone
  checksum-verified `gh` setup plus login/`GH_TOKEN` requirement.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog trust::tests` and
  C09's real verification commands on the approved positive/negative artifacts.

**Acceptance:** any missing or mismatched verification evidence refuses use;
subprocess doubles prove bounds only, not cryptographic authenticity.

### C14 Shared freshness and revocation admission [US2, US4] (4 h)

**After:** C12, C13.
**Files:** `crates/maestro-catalog/src/trust/{records.rs,admission.rs,refresh.rs}`,
`crates/maestro-catalog/src/trust/tests/{freshness.rs,revocation.rs}`,
`crates/maestro-kernel/src/catalog/{trust.rs,tests/trust.rs}`.
**Requirements:** FR-S3-011, FR-S3-022, SC-S3-003.

- [ ] **Step 1: Red.** Test five-minute refresh and exact 24-hour/record-expiry
  boundaries, clock rollback, record/floor replay, entry and bundle revocation,
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

**After:** C14, C17, C05b.
**Files:** `crates/maestro-catalog/src/resolve/{mod.rs,lock.rs,explain.rs,tests.rs}`,
`crates/maestro/src/cli/config.rs`,
`crates/maestro/src/cli/catalog/explain.rs`,
`crates/maestro-catalog/src/bootstrap/project.rs`,
`crates/maestro/tests/it/catalog_explain.rs`.
**Requirements:** FR-S3-011, FR-S3-015, SC-S3-005.

- [ ] **Step 1: Red.** Test precedence provenance, immutable session pins,
  explicit versus implicit update, component/runtime/host/model identity drift,
  authoring versus installed locks and absent S4 qualification/observations.
  Explain a revoked bundle and an expired bundle: each must refuse through C14,
  even with a previously cached explanation or a valid-looking old lock.
- [ ] **Step 2: Green.** Pin the declared identities and supported profiles;
  explain each class/source/requester and declared/effective/observed state.
  Unsupported is not false, zero or qualified; installed explanations enter
  C14's shared admission and the lock cannot override trust.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro-catalog resolve::tests` and
  `~/.local/bin/capped cargo nextest run -p maestro catalog_explain`.

**Acceptance:** every effective value is explainable; updates never silently
change a pinned project and no S4 receipt is fabricated.

### C15 Release and trust-record workflow code [US2, US5] (3 h)

**After:** C10, C13, C22b; owner-created MAN checkout for landing, not live
publisher setup or compiler publication.
**Files (MAN):** `.github/workflows/{check.yml,release.yml,trust.yml,trust-watch.yml}`,
`scripts/tests/trust-workflow.sh`,
`docs/{release.md,rotation.md,emergency-revocation.md}`, `README.md`.
**Requirements:** FR-S3-013, SC-S3-002, SC-S3-003.

- [ ] **Step 1: Red.** A runnable workflow contract check must fail for an
  absent six-hour schedule, missing hourly missed-run alert, refresh tied only
  to tags, unpinned compiler/checksum, PR signing permission, omitted catalog/
  policy tests or wrong publisher binding. Mutate each case to prove refusal.
- [ ] **Step 2: Green.** Write protected release/check/trust workflows with
  checksum-pinned release inputs and SHA-pinned actions; refuse absent/unverified
  compiler inputs, never invent a release digest. The catalog publisher re-issues
  attested records every six hours. Independent hourly trust-watch alerts the
  maintainer/backup after eight hours without a verified issue. Document minimal
  attestation permissions, auth, rotation and emergency revocation; no privileged
  PR checkout. Implement against fixture release metadata until publication.
- [ ] **Step 3: Check.** Run `bash scripts/tests/trust-workflow.sh`, `actionlint`
  and `zizmor --offline .github/workflows`. Save the exact owner-activation and
  live-verification checklist for C28; do not enable workflows or publish.

**Acceptance:** workflow code and schedule/refusal tests pass without live
publisher setup. C28, not C15, owns the real compiler/tag, periodic refresh,
missed-run alert, withdrawal and rotation proofs after OA4/OA5.

### C16 Verified install and update [US2] (4 h)

**After:** C12, C13, C14, C15, C18, C05j.
**Files:** `crates/maestro-catalog/src/install/{download.rs,activate.rs,update.rs}`,
`crates/maestro-catalog/src/install/tests/{updates.rs,recovery.rs}`,
`crates/maestro/src/cli/catalog/{install.rs,update.rs}`,
`crates/maestro/tests/it/catalog_install.rs`.
**Requirements:** FR-S3-009, FR-S3-010, FR-S3-011, FR-S3-012, SC-S3-002, SC-S3-003.

- [ ] **Step 1: Red.** Interrupt download/verification/activation/lock update;
  require the previous valid install to survive. Test incompatible updates,
  expired offline cache, revoked rollback and attempted authoring promotion,
  using C09/C13's real verified fixture artifacts and controlled downloads.
- [ ] **Step 2: Green.** Compose bounded download, C11/C13 validation, C14
  admission, C05j's path-policy port and C12 atomic records. Update the lock
  only by explicit command here and refuse unsigned installs; C16f later adds
  the approved pre-task auto policy without a second update mechanism. Do not wait for a live catalog release to implement these
  paths; C28 tests the owner-published release.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro catalog_install` and
  `~/.local/bin/capped cargo nextest run -p maestro-catalog install::tests`.

**Acceptance:** fixture installs/updates are atomic and authenticated; failure
cannot replace a valid install or revive a revoked snapshot. No source checkout
or compiler toolchain is needed by the implemented consumption path.

### C16b Restore safety and installed-consumer admission [US1, US2] (3 h)

**After:** C16; C06/C07 only for their host-entry-point wiring.
**Files:** `crates/maestro-catalog/src/install/restore.rs`,
`crates/maestro-catalog/src/install/tests/restore.rs`,
`crates/maestro-catalog/src/bootstrap/project.rs`,
`crates/maestro-catalog/src/hosts/{copilot.rs,pi.rs}`,
`crates/maestro/src/cli/backup/{manifest.rs,restore.rs}`,
`crates/maestro/tests/it/{catalog_restore.rs,catalog_admission.rs}`.
**Requirements:** FR-S3-011, FR-S3-012, SC-S3-003.

- [ ] **Step 1: Red.** Restore an older backup with a revoked digest or lower
  floor; require refusal until fresh authenticated records arrive. Exercise
  installed init and both projections with revoked/expired caches and require
  the same refusal, without mutating user files.
- [ ] **Step 2: Green.** Include catalog authority/pins in backup, mark restored
  trust unready and require authenticated refresh before use. Wire C14's shared
  admission at installed bootstrap/projection entry points; authoring mode
  remains explicitly separate and cannot produce verified install records.
- [ ] **Step 3: Check.** Run capped nextest filters `catalog_restore` and
  `catalog_admission` in maestro and `install::tests::restore` in maestro-catalog.

**Acceptance:** backup restores data, never old trust authority; every installed
init/project path uses admission and preserves user files on refusal.

### C16c Verified release discovery and proposals [US2, US3] (3 h)

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

### C20 Native Copilot policy hook [US1, US3] (4 h)

**After:** C06, C19, C21, C05i, C05j; OA2 authorizes the real host test.
**Files:** `crates/maestro-catalog/src/hosts/copilot.rs`,
`crates/maestro-catalog/src/policy/{native.rs,normalize.rs}`,
`crates/maestro-catalog/src/policy/tests/native.rs`,
`crates/maestro/tests/it/catalog_hook.rs`, `docs/how-to/catalog.md`.
**Requirements:** FR-S3-017, FR-S3-036, SC-S3-005, SC-S3-012.

- [ ] **Step 1: Red.** Drive a real pinned preToolUse hook through allow,
  deny, malformed request, unknown tool, opaque shell, missing fact, error and
  timeout. Try model-supplied identity/approval, outside-trust writes, secret
  reads even inside trust, link escapes and agent-shell trust administration,
  including trust-add with a correct --confirm-path;
  denied cases record no effect, with ordinary allowed neighbours.
- [ ] **Step 2: Green.** Normalize only supported operations into C19's real
  evaluator and the same `WorkspaceTrust` port as Maestro-owned effects; load
  hooks only through the explicit projection. Report absent hook
  as unprotected; an approval-needed result cannot be manufactured into allow.
- [ ] **Step 3: Check.** Run
  `~/.local/bin/capped cargo nextest run -p maestro catalog_hook` and the
  authorized real-host hook probe, including its allowed neighbour.

**Acceptance:** real policy denial, not merely hidden tools; errors/timeouts
fail closed when the hook is installed. Documentation makes no broker,
acceptance, journal or sandbox claim for native mode. Extending secret-path
rules changes checked data, not this hook; other clients' trust hooks stay the
named S4 qualification obligation.

## Phase 4: Measured routing and exact impact [US4]

### C23 Reviewed intent labels [US4] (3 h)

**After:** C21, C21b; independent review of every label arranged by the supervisor.
**Files (MAN):** `evals/intents/{routing.jsonl,review.json,split.json,eligibility.json,check.jq}`,
`evals/intents/fixtures/distractors.md`, `evals/intents/README.md`.
**Requirements:** FR-S3-020, SC-S3-006.

- [ ] **Step 1: Red.** Define checks for fewer than 100 cases, duplicate IDs,
  missing alternatives/review, conflicting no-match/clarification labels,
  tuning/held-out overlap, empty matchable denominator, absent adversarial/
  distractor coverage or missing/mismatched synthetic eligibility snapshot.
- [ ] **Step 2: Green.** Draft public/synthetic intents, independently review
  each, fix confirmed errors and freeze digests/splits plus `eligibility.json`.
  Record suite and synthetic eligibility digests together; include only explicit
  test role/profile grants, never live qualification records. Include forged authority,
  revoked resources, contradictory skills, low-similarity required reviewers,
  stale indexes and failed embedders. Do not copy private questions.
- [ ] **Step 3: Check.** Run the fixture check through the installed toolbelt:
  `jaq -e -s -f evals/intents/check.jq evals/intents/routing.jsonl`;
  check review/split IDs and workflow references against the frozen source
  inventory and synthetic eligibility snapshot; retain both digests and the
  matchable denominator. C24 adds the workflow-ID parser and scorer.

**Acceptance:** at least 100 reviewed cases with alternatives, honest negative
labels and a nontrivial distractor cohort; no outcome tuned on the held-out set.

### C24a Exact resolve/search and eligibility [US4] (3 h)

**After:** C14, C18, C22b, C23; no release or real-host prerequisite.
**Files:** `crates/maestro-catalog/src/resolve/{lookup.rs,search.rs}`,
`crates/maestro-catalog/src/route/{mod.rs,request.rs,eligibility.rs,result.rs}`,
`crates/maestro-catalog/src/route/tests/{mod.rs,eligibility.rs}`,
`crates/maestro/src/cli/catalog/{resolve.rs,search.rs}`,
`crates/maestro/src/mcp/catalog.rs`,
`crates/maestro/src/mcp/server/{handler.rs,operations.rs}`,
`crates/maestro/tests/it/catalog_resolve.rs`.
**Requirements:** FR-S3-021, FR-S3-022, SC-S3-006.

- [ ] **Step 1: Red.** Test exact IDs/closure, scope/revocation/qualification
  exclusion, caller-bound cache partitioning, expired snapshots and still-valid
  offline use. Synthetic eligibility must work only in the test harness; a real
  unqualified executable workflow returns `incompatible`.
- [ ] **Step 2: Green.** Implement shared pre-limit eligibility and exact
  definition/closure lookup with bounded catalog browsing. Add resolve/search
  to the existing CLI/MCP, binding caller, reasons and snapshot; no raw storage
  filter or separate server. Consume C14 records directly in fixture tests.
- [ ] **Step 3: Check.** Run capped nextest filters `route::tests::eligibility`
  in maestro-catalog and `catalog_resolve` in maestro, on C23's frozen snapshot.

**Acceptance:** exact authorized results, mandatory closure complete, no
synthetic qualification accepted by normal installed consumers.

### C24 Exact-ID and lexical baseline [US4] (4 h)

**After:** C14, C18, C22b, C23, C24a; no release-setup prerequisite.
**Files:** `crates/maestro-catalog/src/route/{lexical.rs,tests/lexical.rs}`,
`crates/maestro-catalog/src/eval/{mod.rs,intents.rs,score.rs}`,
`crates/maestro-catalog/src/eval/tests/{mod.rs,intents.rs,score.rs}`,
`crates/maestro/src/cli/catalog/route.rs`,
`crates/maestro/src/mcp/catalog.rs`,
`crates/maestro/tests/it/catalog_route.rs`.
**Requirements:** FR-S3-020, FR-S3-021, FR-S3-022, SC-S3-006.

- [ ] **Step 1: Red.** Hand-score workflow-ID alternatives/negative labels;
  test all five statuses, deterministic ties, low-similarity required reviewers,
  lexical limits and offline behavior. Verify suite/eligibility digests and a
  nonempty matchable denominator before scoring.
- [ ] **Step 2: Green.** Route exact IDs and structured lexical cards through
  C24a's eligibility/closure; add only the route CLI/MCP adapter. Report reasons,
  snapshot and explicit no-index state. Reuse S1 statistical methods, not its
  section-ID labels, for the workflow scorer.
- [ ] **Step 3: Check.** Run `route::tests` and `eval::tests` in maestro-catalog
  and `catalog_route` in maestro through capped nextest. Record baseline metrics
  on C23's frozen synthetic eligibility, with negative/distractor cohorts separate.

**Acceptance:** top-3 ≥90 % on the synthetic matchable cohort, closure 100 %;
scores are not probabilities. Real unqualified workflows stay `incompatible`;
no failed or empty-denominator comparison is called a passing baseline.

### C25 Rebuildable catalog cards [US4] (3 h)

**After:** C10, C12, C14; verified CORE fixture records, not release setup.
**Files:** `crates/maestro-catalog/src/discovery/{mod.rs,cards.rs,publish.rs,tests.rs}`,
`crates/maestro-kernel/src/catalog/{discovery.rs,tests/discovery.rs}`,
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

**After:** C23, C24, C25; OA6 only if a model/provider needs new access.
**Files:** `crates/maestro-catalog/src/route/{hybrid.rs,tests/hybrid.rs}`,
`crates/maestro-catalog/src/eval/{compare.rs,tests/compare.rs}`,
`crates/maestro-knowledge/src/search/{filter.rs,request.rs}`,
`crates/maestro-knowledge/src/search/routes/{dense.rs,lexical.rs,identifier.rs,structured.rs}`,
`crates/maestro-knowledge/src/search/tests/catalog_eligibility.rs`,
`crates/maestro-knowledge/tests/it/qdrant_projection/catalog_eligibility.rs`,
`specs/003-catalog/research/routing.md`.
**Requirements:** FR-S3-022, FR-S3-024, SC-S3-006, SC-S3-007.

- [ ] **Step 1: Red.** Put ineligible high-scoring IDs ahead of eligible IDs
  beyond each branch's top-k; require eligible results without post-filter
  starvation. Test empty eligibility, stale index, failed embedder and cache
  invalidation, and paired-comparison scoring against hand-computed neighbours.
- [ ] **Step 2: Green.** Carry eligible IDs into every contributing S1 branch
  before limits, preserving existing knowledge semantics; compare dense/BM25/
  fusion with C24's baseline on the same held-out snapshot and profiles. Keep
  lexical fallback and S1 free-room admission; never unload a chat model.
- [ ] **Step 3: Check.** Run capped nextest filters `catalog_eligibility`,
  `hybrid` and `compare` in their respective crates; record paired intervals,
  top-1/top-3, no-match, clarification, context and distractor results.

**Acceptance:** enable hybrid only for demonstrated paired gain; otherwise
ship the passing baseline. Top-3 ≥90 % and closure 100 % still apply. A failed
comparison is retained; no benchmark result is invented to enable hybrid.

### C27a Scoped catalog dependency projection [US4] (4 h)

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

**After:** C22b, C24, C27a.
**Files:** `crates/maestro-catalog/src/impact/{traverse.rs,tests/traverse.rs}`,
`crates/maestro/src/cli/catalog/impact.rs`,
`crates/maestro/src/mcp/catalog.rs`,
`crates/maestro/tests/it/catalog_impact.rs`,
`docs/architecture/08-traceability.md`.
**Requirements:** FR-S3-025, SC-S3-007.

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

## Phase 5: M3 exit and the later comparison [US1–US5]

### C28 Tagged M3 proof [US1, US2, US3, US4, US5] (3 h)

**After:** C08, C05f, C05k and OA9 visual acceptance, C15, C16, C16b,
C16c, C16d, C16e, C16f, C16g, C17, C18, C19, C20, C21, C21b, C22b, C23,
C24, C25, C26, C27; integrated T034/T035, T038 live registrations, M1 release,
final CI and OA4/OA5/OA7 live trust/release/evidence actions.
**Files:** `scripts/tests/catalog-m3.sh`,
`docs/how-to/catalog.md`, `docs/how-to/knowledge-mcp.md`,
`docs/architecture/08-traceability.md`,
`specs/003-catalog/traceability.json`,
`specs/003-catalog/research/m3-evidence.md`; private receipts remain private.
**Requirements:** SC-S3-001, SC-S3-002, SC-S3-003, SC-S3-004, SC-S3-005,
SC-S3-006, SC-S3-007, SC-S3-008, SC-S3-009, SC-S3-010, SC-S3-011, SC-S3-012.

- [ ] **Step 1: Red.** Write the shell acceptance harness; fail its preflight
  if `command -v cargo`, `command -v rustc` or `command -v python3` succeeds,
  a source checkout is present or doctor rejects the released `maestro`/pinned
  `gh`. It must also fail missing tag/attestation, any exit/key/client evidence,
  M1 release, 08 §17 publisher closure, missed trust-refresh alert or current CI.
- [ ] **Step 2: Green.** Copy only the script into OA5's disposable clean WSL
  user/container with released binaries, basic shell utilities and approved
  read authentication. Run install → init → project → update → remove; collect
  real Copilot/Pi and T038 Codex/Claude MCP receipts in approved host homes.
  Verify OA4/OA5 release, six-hour re-issue, missed-run alert, withdrawal and
  rotation evidence; finish C09's owned publisher-row evidence handoff. Record
  expiry/revocation/offline/restore cases, C23's synthetic suite/eligibility
  digests and metrics, plus real installed `incompatible` ("not qualified until
  S4") results. Include preference/trust/client-delivery receipts and verified
  startup off/propose/catalog-auto/consent/rollback/offline evidence, runtime
  propose-only and MCP zero activation. Include C05k/OA9 visual acceptance;
  no mid-task effects or claims of runtime installer qualification.
  Update only delivered portions of existing traceability rows; preserve their
  S4/follow-up obligations. Document outcomes in `docs/how-to/catalog.md`.
- [ ] **Step 3: Check.** Run `bash catalog-m3.sh` inside that clean environment,
  not through cargo. In the checkout run offline links and C00's traceability
  suite. Quote final CI platform/test counts, coverage, killed/missed/timeout
  totals and commit/suite/eligibility digests in the evidence note.

**Acceptance:** every M3 criterion has current observed evidence; no skipped
live test or pending owner/S2/CI action is called passed. Only the owner accepts
M3 and authorizes publication; lanes neither release nor integrate themselves.

### C29 One post-M3 comparison [US5] (3 h)

**After:** C28 accepted as M3; OA8 access approval. This is outside M3's exit.
**Files:** `specs/003-catalog/research/{comparison.md,comparison-inventory.json}`;
no recovered source files in this task.
**Requirements:** FR-S3-026.

- [ ] **Step 1: Red.** Define a completeness check that fails for any source
  item without a provenance, disposition and reason, including an item proposed
  for recovery without a separately bounded task and licence/attribution notes.
- [ ] **Step 2: Green.** Read the approved earlier catalog once, inventory it
  and record recover/defer/reject decisions against the new requirements. Do not
  copy bulk content or silently simplify an imported approval/gate.
- [ ] **Step 3: Check.** Compare the complete authorized source inventory with
  the disposition JSON: equal key sets, no duplicates, no missing reasons;
  check report links and each proposed recovery's ≤4 h acceptance scope.

**Acceptance:** every item is accounted for; useful recoveries become separate
reviewed tasks/PRs, not an unreviewed extension of this comparison commit.

## Dependencies & Execution Order

The **After** line is authoritative, including external prerequisites. Every
internal predecessor occurs earlier above. Tasks in one phase are not all
parallel: the following are safe file-separated examples after their own
predecessors land; shared registration changes still serialize.

| Ready work | Can overlap | Do not overlap |
| --- | --- | --- |
| C00 recorded | C01 live probe, C03 fixture checker and C04a filesystem move | C03 does not wait for missing hosts; freeze sidecars if needed |
| C03 complete | C04 after C04a, C17 settings and C02 content in MAN | Two writers to the same CLI/module registration |
| C05 complete | C05a preferences and optional C05f TUI measurements | No parser dependency; C05f/OA9 never gate plain init |
| C05b complete | C05h/C05i/C05j trust; C05c/C05l/C05d/C05e presentation | C05g plain flow joins trust and C05c, not MCP delivery |
| C05e/C05j complete | C06 after C01/OA2, without C05g/C05k; optional renderer after C05g/C05f/OA9 | C07 waits for C06's shared projection command |
| C09 complete | C19 policy work and source-independent trust research | Dependency-lock updates from multiple lanes |
| C11 complete | C12 records and C13 attestation | Unassigned SQL migrations with S2 |
| C21/C21b complete | C23 MAN labels; C25 CORE cards after C12/C14 | A scoring implementation before labels are frozen |

Fixture implementation starts C00 → C03, with C04a → C04 → C05 and C17 in
parallel when their predecessors permit. Neither C01 nor the real C02 seed gates
C03/C05. C17 precedes C05a, then C05b feeds trust and presentation. C05g's plain
flow needs trust and C05c, not C05e; C06/C07 need delivery and the write gate,
not the menu. C05l owns message migration. If C01 is blocked at C03 start,
sidecars are final for v1. First live use adds C01/OA2 → C06 → C07, plain C05g,
C02/OA1 and T034/T035/T038 → C08. C05f → C05k (OA9 library approval recorded) is an independent later
renderer path, joined at C28 with visual evidence, never a first-loop gate.

Trust/compiler code: C09 → C19 → C22a → C22b → C10 → C11 → C12/C13 → C14;
C18 also needs C17. C15's workflow code can land without OA4/OA5; C16 consumes
that code and verified fixtures, then C16b adds restore/admission wiring.
C16c → C16d adds catalog receipts/rollback; C16c → C16e → C16g adds verified
runtime proposals and install-command display only. C16f joins both for startup
policy and MCP notices, consuming the existing trust/client-delivery ports.
C28 alone joins these paths with real publisher/compiler/drill evidence.

Routing: C21 → C21b → C23 plus C14/C18/C22b → C24a → C24, with C25 → C26.
There is no C15/C16 or live-host prerequisite for routing implementation.
Impact: C12/C14 plus qualified S2 G25 and the S2-owned G27 public port → C27a;
C22b/C24/C27a → C27. C28 joins all exits, including C08, M1 release, C15/C16b,
C27, C05k/OA9, OA4/OA5/OA7 and final CI. C29 starts only after accepted M3.

### Requirements coverage

| Requirement | Tasks |
| --- | --- |
| FR-S3-001 | C00, C02, C21, C21b |
| FR-S3-002 | C01, C03, C02 |
| FR-S3-003 | C03, C02, C22a, C21, C21b |
| FR-S3-004 | C05 |
| FR-S3-005 | C04a, C04, C05, C05j |
| FR-S3-006 | C01, C06, C07 |
| FR-S3-007 | C02, C06, C07, C08 |
| FR-S3-008 | C09, C10 |
| FR-S3-009 | C11, C16 |
| FR-S3-010 | C09, C13, C16 |
| FR-S3-011 | C09, C14, C18, C16, C16b |
| FR-S3-012 | C12, C16, C16b |
| FR-S3-013 | C15 |
| FR-S3-014 | C17, C05b |
| FR-S3-015 | C18 |
| FR-S3-016 | C09, C19, C22b, C21 |
| FR-S3-017 | C20 |
| FR-S3-018 | C21, C21b |
| FR-S3-019 | C00, C22a, C22b, C21, C21b |
| FR-S3-020 | C23, C24 |
| FR-S3-021 | C24a, C24 |
| FR-S3-022 | C14, C24a, C24, C26 |
| FR-S3-023 | C25 |
| FR-S3-024 | C00, C26 |
| FR-S3-025 | C27a, C27 |
| FR-S3-026 | C29 |
| FR-S3-027 | C17, C05a, C05j, C05g, C08 |
| FR-S3-028 | C05b, C08 |
| FR-S3-029 | C05c, C05l, C05d, C08 |
| FR-S3-030 | C05g, C08, C05f, C05k |
| FR-S3-031 | C05e, C06, C07, C08 |
| FR-S3-032 | C16c, C16e, C16g, C16f |
| FR-S3-033 | C17, C05a, C16c, C16e, C16g, C16f |
| FR-S3-034 | C16d, C16e, C16f |
| FR-S3-035 | C05h, C05j, C05g, C08 |
| FR-S3-036 | C05h, C05i, C05j, C05e, C06, C07, C08, C16d, C16f, C20 |
| SC-S3-001 | C01, C06, C07, C08, C28 |
| SC-S3-002 | C10, C15, C16, C28 |
| SC-S3-003 | C11, C12, C13, C14, C15, C16, C16b, C28 |
| SC-S3-004 | C04a, C04, C05, C06, C07, C08, C28 |
| SC-S3-005 | C03, C17, C19, C22a, C22b, C18, C20, C28 |
| SC-S3-006 | C23, C24a, C24, C26, C28 |
| SC-S3-007 | C25, C26, C27a, C27, C28 |
| SC-S3-008 | C28 |
| SC-S3-009 | C00, C28 |
| SC-S3-010 | C05a, C05b, C05j, C05c, C05l, C05d, C05e, C05g, C06, C07, C08, C05f, C05k, C16d, C28 |
| SC-S3-011 | C16c, C16d, C16e, C16g, C16f, C28 |
| SC-S3-012 | C05h, C05i, C05j, C05g, C06, C07, C08, C16d, C16f, C20, C28 |

## Implementation Strategy

1. Finish C08 before treating the platform as useful. It exercises one explicit
   preset and the existing M1 answerer, without waiting for routing or an engine.
2. Keep the trusted install boundary closed until verification, freshness and
   atomic records are ready. Internal tested seams are not unsigned CLI features.
3. Add policies/settings/graphs only as needed by the two workflows; finish
   declarative checks before publishing their compiled closure.
4. Measure routing against frozen labels; retain the simpler passing route.
   Do not let synthetic qualification fixtures authorize live roles.
5. Close M3 only with observed release, host, S2 and full CI evidence. Compare
   earlier content afterwards, once, without bulk recovery.

## Needs owner action

The [plan's owner-action register](plan.md#needs-owner-action) is authoritative;
only its explicitly approved scope authorizes assigned lane operations.
Remaining owner actions are blockers or handoffs, not implied permissions.
OA7's frozen keys/S3 portions and hook deferral are approved by the owner,
2026-09-28. Only its later actions remain in the table.

| Action group | Register | Tasks waiting |
| --- | --- | --- |
| Create manifests repository, owners and protections | OA1 | MAN landing: C02, C21, C21b, C23, C15; not CORE fixtures |
| **Approved 2026-09-28:** probe/test already-installed Copilot CLI, Pi, Claude Code and Codex, each in an isolated temporary home; C01 pins exact installed versions. No installs/upgrades, real owner configuration or enterprise policy changes; broader scope needs fresh approval | OA2 | Bounded C01, C06, C07, C08, C20, C28 host tests; missing access still blocks and OA6 data approval remains separate |
| Bind publishers, standalone pinned `gh`/authentication; authorized maintainer handles any unlisted licence | OA4 | C09's publisher-row evidence closure and C28, not verifier/dependency implementation |
| Publish compiler/catalog; enable six-hour trust attestations/hourly alerts; supply clean environment and drills; authorize private/model access | OA5, OA6 | C28 release proof; C08/C23/C26 only for the requested private/model access |
| Accept M3 and eventual main release; later comparison access; any S2 fallback separately | OA7, OA8 | C28, C29 |
| **Approved 2026-09-28:** ratatui + crossterm under ADR-0020; C05f measurements/vet still required, branded visual acceptance pending; no parser dependency | OA9 | C05k after measurement, C28 after visual evidence; never first plain C08 |

Record blocked/not-run when any input or external proof is absent. Do not
create a repository, change organization settings, publish, enable workflows,
install a client or send private material as a side effect of a coding task.
