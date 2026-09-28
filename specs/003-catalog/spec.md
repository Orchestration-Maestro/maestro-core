# Feature Specification: Catalog

**Feature Branch**: `docs/s3-analyze-fixes`, from `feat/s3-integration` at `0be954b`

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

## Rules touched

The [plan's holders](plan.md#constitution-check) name each gate, test or review;
planned holders are obligations, not implementation evidence.

| Rule IDs | S3 obligation |
| --- | --- |
| C-001, C-005 | Maintain both repository rule maps and durable acceptance evidence |
| FND-002, FND-003, P-001, P-004, P-005 | Required content only; reuse existing seams and small replaceable adapters |
| P-011, P-013, SEC-003 | Strict typed parsing and bounded hostile-input refusal |
| P-012, P-014 | One scoped admission path; least-privilege credentials and effects |
| ENF-001, ENF-003, SEC-001 | Runtime-derived paths, English artifacts/logs and synthetic public data |
| ENF-002, ENF-005, ENF-006 | Three-platform checks, red-first tests and unchanged quality bars |
| ENF-008, ENF-012 | Commit hooks, layered gates and pinned inputs |
| ENF-009 | Exact, current mutation exclusions and dependency/architecture exceptions |
| ENF-011, SEC-002, SEC-004, SEC-005 | Input grants no authority; trusted identity and scoped human approval |
| ENF-013 | No secrets in files, logs or history |
| SEC-006 | Verify the pinned executable before every bounded subprocess launch |
| SEC-008 | Separate declared, observed, unsupported and not-run evidence |
| SEC-011 | Attestation, per-asset checksums, SPDX JSON SBOM and verification instructions |
| ADR-0005, ADR-0007, ADR-0015 | Host probe decides formats; real Cedar; authenticated freshness/revocation |
| ADR-0018, ADR-0020 | One held-handle filesystem implementation; measured approved libraries |

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
- **D5 Quality — original decision (history):** a hybrid win is required only to enable hybrid
  routing. Keep top-3 accuracy at least 90 %, report top-1 and correct no-match
  separately, and include synthetic distractors so two workflows cannot make
  top-3 trivial. Record the change explicitly in architecture 06; do not
  silently reinterpret its older unconditional "beats the baseline" wording.
  OA10, approved by the owner on 2026-09-28, amends this absolute bar to
  held-out matchable top-1 ≥ 90 %; the conditional hybrid rule is unchanged.

The exact host pin, measured dependency versions and publisher bindings are
execution inputs and qualification evidence, not unresolved product choices.
[Needs owner action](plan.md#needs-owner-action) lists the external steps.

### Owner decision, 2026-09-28 08:12

"Finish S1 in parallel and start S2 and S3." C00 waits only for the decided
D1–D5, not M1. Integrated T034/T035 and T038 live evidence gate C08 and C28;
the M1 release remains an M3 exit dependency, not an implementation-start gate.

### Owner decision, 2026-09-28 11:25

Init stores workspace settings in `.maestro/config.toml` at the init root,
asks the conversational language and tone, and offers a polished, branded
interactive configuration menu plus flags and `--yes` for scripts. Every
Maestro session discovers the nearest workspace file upward from its working
directory within the safe discovery boundary below. Free preferences use
**explicit flags > workspace file > user-level config > built-in defaults**;
this supersedes the older five-layer preference order. No layer grants authority.
Code, commits, file names, identifiers, logs and documentation always stay
English; language and tone affect conversational prose only.

The supervisor's schema ruling keeps user preferences in `preferences.toml`
inside the existing platform configuration directory, separate from the
kernel's authority-bearing `config.toml`. UI/init defaults are `en` and `normal`;
without an explicit session language, answers retain S1's question language.
Display "Very detailed", store `detailed`.
Presets seed explicit init choices, not an additional session precedence layer.
OA9 subsequently approved `ratatui` with `crossterm` on 2026-09-28 under ADR-0020;
minimum-feature measurement and visual acceptance remain required.

### Owner clarification, 2026-09-28 11:50

The owner requested languages beyond en/fr. The supervisor's review ruling
bounds S3 to a well-formed **BCP 47 subset**: 2–3 ASCII-letter language,
optional 4-letter script and optional 2-letter or 3-digit region, with canonical
casing. No other subtag is accepted and no parser dependency is needed.
Explicit session language governs generated conversation; without one, use the
question's language. Evaluation runs always keep question-language behavior
and ignore session preferences. Built-in interface strings ship in `en`, `fr` and `es`;
other languages keep English interface text with one visible fallback note,
while conversation still uses the chosen language. Tone changes only prose
length/detail, never code or document content. Every session reads the settings;
MCP uses explicit registered `--workspace` for workspace overrides, otherwise
user preferences, and exposes them to Pi, Claude Code, Codex and Copilot.
Maestro-launched
agents receive them in their instructions. S3 proves instruction construction
and delivery; the named S4 launch-adapter obligation proves actual launch use.

### Owner requirement, 2026-09-28 11:55

At session startup, check for newer verified production releases, rate-limited
to once per day and safe offline. The review ruling adds `updates = "off"`
(no startup discovery), keeps `"propose"` as default and allows `"auto"` only
as user-level catalog consent; workspace settings/flags may only narrow it.
Runtime releases are propose-only in S3. MCP never applies updates and exposes
only a fixed path-free ID/target/version notice through the client-delivery port,
never release-note text.
Permission-widening or hook-changing updates always require explicit human
approval, even in auto mode. Every applied update has a receipt and a rollback
path; no update runs mid-task. Reuse ADR-0015 and the existing verification,
freshness, revocation and activation path, not a second updater.

The supervisor's inventory ruling covers the released Maestro runtime (its
separate publisher identity) and every installed catalog with its pinned
component closure. Cargo resolution, host clients, models, pinned `gh` and
Qdrant setup are not S3 update targets. The named later follow-up is
"notify-only update checks for owner-managed components (Qdrant, router runtime,
host clients, models)": propose-only after approved sources exist, never apply.

### Owner requirements, 2026-09-28 12:00 and 12:05

Init asks whether to trust its canonical working directory (normally a clone,
or an explicitly chosen larger workspace). The review ruling keeps the answer, canonical folders and approval receipt
entirely in user-local kernel authority records keyed by canonical path, not in
workspace config. Only `maestro trust add/list/remove` manages these records;
cloned/edited config, catalogs and updates cannot grant trust. Add asks default-no
on a terminal, or requires an exact canonical `--confirm-path DIR` without one;
never `--yes`, environment variables or MCP text. Filesystem/drive/mount roots,
HOME itself and kernel-internal directories cannot be trusted.

Trust permits non-secret reads/writes inside approved folders under other
mandatory controls. Writes outside are blocked; reads outside are allowed
except for the immutable secret-location deny list. Secrets remain denied even
inside trust; ADR-0018 canonical/held-handle checks prevent link and `..` escapes.
S3 enforces this on its own file effects and Copilot's hook; Pi/Codex/Claude
hooks remain the named S4 obligation. Kernel-internal XDG config/data/state
writes retain kernel authority and are never granted to agents/tools by trust.
Declining trust permits only an explicitly confirmed config/approval-metadata
write, not templates or installation. External host installation directories
need their own once-only user grant; never auto-trust HOME.

Keep workspace configuration, path trust, updates and instruction delivery as
small modules with clear ports. Secret denies are checked data in the default
trust adapter; runtime/catalog releases use update-source adapters; MCP and
native projections use client-delivery adapters. Another client or approved
source must not require changes in callers. No plugin runtime is implied.

### Review boundaries, 2026-09-28

Discovery inside home stops at home. Outside home no workspace config is read
until a containing root is journal-trusted; discovery then stays inside it.
Require current-user ownership and no group/world write access to the config
and its directory on held handles. Skip foreign-owned/unsafe candidates with a
warning, including planted invalid files; malformed selected safe files still
refuse. Never select drive/mount roots. No preference becomes authority.

MCP initialization uses explicit `--workspace` or user preferences, never the
client's spawn directory or roots. Report the source without paths. Roots-based
workspace detection waits for a client-qualified channel (06/08). Plain init
ships before the optional TUI; C08 does not wait for OA9. Runtime auto-apply with
the installer is also a named follow-up. These review rulings supersede the
broader discovery, in-file trust, language grammar and runtime-apply wording of
the initial amendment.

### Owner approvals, 2026-09-28

- **OA10 approved (owner, 2026-09-28):** amend D5's absolute routing bar to
  held-out matchable top-1 ≥ 90 % (correct first selection), replacing the
  original top-3 ≥ 90 % bar retained as history above. Report both metrics.
- **OA9 approved:** ratatui + crossterm for the TUI under ADR-0020; measure the
  minimum adoption footprint in C05f. Branded visual acceptance is still pending.
- **OA2 approved, bounded scope:** probe/test the already-installed Copilot CLI,
  Pi, Claude Code and Codex, each in an isolated temporary home. C01 records
  every exact installed version as its pin. No installs/upgrades, changes to
  the owner's real configurations or enterprise policy. Anything beyond this
  scope needs fresh OA2 approval; no private-data/provider approval is implied.

### Execution rulings, 2026-09-28

The 12:00 analyze ruling keeps ADR-0005. C01's integrated
[host report](research/hosts.md) at `0be954b` and the supervisor's subsequent
ruling confirm agent `<stem>.maestro.toml` sidecars, with each catalog agent's
file stem equal to its `name:`. Copilot CLI 1.0.88 warns
`unknown field ignored: metadata` on agents. Skills use `metadata:` under the
[Agent Skills specification](https://agentskills.io/specification): C01's
unknown-key control also loads silently, so silence proves no field support.
A host warning on skill metadata reopens the ADR-0005 sidecar decision;
there is no automatic sidecar default. C03 consumes this integrated outcome.
Reload inside a running session remains not run, an explicit C06/C07 input.
CORE fixture implementation does not depend on MAN content or publication;
C08/C28 retain the live evidence gates.
The workspace-config amendment already resolves the old settings-layer finding:
`.maestro/project.toml` is a descriptor, `.maestro/config.toml` holds workspace
preferences, and platform-derived user `preferences.toml` supplies the lower
layer. No five-layer resolver or settings fields in the descriptor are restored.

**D5 metric amendment — OA10 approved (owner, 2026-09-28):** the absolute
routing bar is **held-out matchable top-1 ≥ 90 %**, requiring a correct first
selection, not merely shortlist inclusion. This is the owner's dated amendment
to D5, not a clarification of its original top-3 ≥ 90 % wording retained above
as history. Report both metrics; top-3 is diagnostic, not the acceptance bar.
D5's seeded paired top-1 gain protocol still governs whether hybrid improves
on the baseline; it is unchanged by the amended absolute target.

The supervisor approved a pure move of ADR-0018's existing filesystem code and
tests into `maestro-filesystem` (C04a), and the S2 G27 public typed-edge port for
S3's separate catalog dependency projection (C27a). S3 supplies the edge schema
and adapters; these are not evidence-span claims. C00 records these boundaries
and the hook disposition below in architecture 03/06/08.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Start a project and ask a cited question (Priority: P1)

As the owner, I select the knowledge preset, choose Maestro's conversational
language and tone in a polished configuration menu, preview its files, project
the Maestro profile into Copilot or Pi, and ask a question through the existing
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
6. **Given** a terminal, **When** I run `maestro init`, **Then** a branded,
   keyboard-accessible menu shows workspace/preset, language, tone and allowed
   overrides and the canonical workspace trust decision, followed by a complete
   file preview. Confirmed `--apply` with explicit user trust writes
   `.maestro/config.toml` at the displayed init root; Back, cancel or preview
   alone writes nothing, including no configuration file.
7. **Given** a script without a terminal, **When** I run init with
   `--language fr --tone detailed --yes --apply` and an explicit preset,
   **Then**, after `maestro trust add DIR --confirm-path DIR` for that exact
   canonical root (including in a fresh CI home), it never prompts,
   validates the same schema and produces the same files as those choices. `--yes` alone does not imply `--apply`
   or waive collisions, policy, trust or validation; missing required choices
   without `--yes` produce a usage error rather than a hang.
8. **Given** no color, a small terminal or a screen reader, **When** I configure
   Maestro, **Then** a plain sequential mode exposes the same choices, errors
   and preview; focus and status never depend only on color or animation.
9. **Given** malformed/duplicate/unknown settings or a malformed language tag,
   **When** init validates them, **Then** it names the offending path/key or tag
   and refuses before writes; no arbitrary SDK option or authority-bearing table
   passes as an override. A supported well-formed tag outside the shipped interface translations
   is accepted, with a visible English-interface note, not a changed conversation
   language.
10. **Given** an untrusted clone, **When** I apply init, **Then** it asks about
    the displayed canonical root and records the answer/paths/receipt only in
    user-local kernel authority. Decline can separately confirm a preferences-only
    config write, never a template or projection. `--yes` cannot grant trust;
    adding a folder after editing preferences leaves the config byte-identical.

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
   entry or bundle, **When** a bundle is installed/updated or an installed bundle
   is used by init, resolve, search, route, explain, impact or projection,
   **Then** the same admission check refuses it, caches included. Labelled
   authoring-source convenience is separate, never an unsigned installation.
4. **Given** still-valid offline records, **When** resolving from the cache,
   **Then** the remaining validity window is shown; at expiry it is refused.
5. **Given** interrupted installation or an old backup, **When** recovered,
   **Then** durable ownership and pins remain consistent, and restored catalog
   use requires a fresh authenticated trust check rather than a restored floor.
6. **Given** `updates = "propose"` and a newer verified production release,
   **When** a daily startup check is due, **Then** Maestro identifies the runtime
   or installed catalog, summarizes verified changes and gives one exact apply
   command. It changes no installed version; unverified metadata cannot become
   a trusted proposal. Repeated same-day startups reuse the recorded result.
7. **Given** user-level `updates = "auto"` not narrowed by another layer and a
   verified compatible catalog update without wider
   permissions or hook changes, **When** the session is idle before its first
   task, **Then** the existing updater activates it atomically and records the
   old/new identities, trust evidence, policy and rollback target in a receipt.
8. **Given** a permission-widening or hook-changing update, **When** auto would
   apply it, **Then** it requests explicit approval bound to the exact change;
   without an interactive human or recorded scoped approval it only proposes.
   `--yes` and agent text cannot approve the change.
9. **Given** an unverified, revoked, expired or incompatible update, a partial
   download, or offline startup, **When** checked, **Then** no unsafe update is
   applied. Offline check failure is visible but does not block otherwise valid
   current work; existing trust expiry still refuses invalid catalog use.
10. **Given** a recorded applied catalog update, **When** rollback is requested,
    **Then** retained prior artifacts and pins are restored atomically with a
    linked receipt only if current trust still admits them. A revoked target is
    refused; an active task defers both update and rollback until a safe boundary.
11. **Given** startup updates off, a runtime proposal or an MCP session, **When**
    startup runs, **Then** off makes zero discovery network calls, runtime never
    auto-applies, and MCP never activates any target. Explicit CLI check still
    works with off; MCP instructions contain no release notes or install command.

---

### User Story 3 - Change preferences without widening permissions (Priority: P1)

As a developer, I change language, tone or an allowed model profile and
can see where each effective value came from, without disabling mandatory
checks, weakening permissions or changing another role's settings.

**Why this priority**: customization must not create an authorization bypass.

**Independent Test**: resolve the same project under each override layer;
check allowed neighbours and denied changes through the real resolver and
Cedar evaluator, with zero executor calls on denial.

**Acceptance Scenarios**:

1. **Given** a setting, **When** checked or loaded, **Then** it has exactly one
   class: free, bounded, additive or locked; unknown or duplicate classes fail.
2. **Given** conflicting free preferences, **When** resolved, **Then** precedence is
   explicit flags > workspace file > user-level config > built-in defaults,
   per key, with every source explained. Only explicitly supplied flags win;
   parser defaults never hide the workspace or user value.
3. **Given** a wider permission, removed check or larger budget, **When** proposed
   as an override, **Then** permissions still intersect, checks accumulate and
   budgets narrow; locked changes are refused.
4. **Given** a Copilot native hook, **When** it checks an operation, **Then** the
   same Cedar rules allow or deny it; missing facts, unknown tools, opaque shell,
   evaluator errors and timeouts deny. An absent hook is reported as unprotected.
5. **Given** no S4 qualification or observed run, **When** explained, **Then** the
   value is unsupported or not observed, never an invented qualification receipt.
6. **Given** nested directories/workspaces, **When** CLI or explicitly registered
   MCP workspace discovery runs, **Then** it selects only the nearest safe,
   user-owned config within home or an approved external root. Foreign/unsafe
   candidates warn and are skipped; malformed selected files refuse. No selected
   file means user/default preferences. MCP without `--workspace` always uses
   user preferences, regardless of cwd/roots. A new session sees edits.
7. **Given** `fr` with any tone, **When** Maestro answers or displays human CLI/TUI
   messages, **Then** its conversational prose follows that choice, but generated
   code/comments, commits, file names, identifiers, logs and documentation remain
   English. JSON keys/status codes, command names, citation IDs and quoted source
   bytes are unchanged; a brief answer still contains required evidence/refusals.
8. **Given** workspace settings that name `[access]`, identity, hooks, secrets or
   a locked parameter, **When** loaded or explained, **Then** they are refused.
   User preferences cannot change the kernel authority config; allowed bounded
   overrides still cannot widen permissions or budgets.
9. **Given** a language such as `ja` and any tone, **When** each of Pi, Claude
   Code, Codex and Copilot initializes an MCP connection, **Then** server
   instructions expose that validated conversational preference and the English
   artifact/log rule. Interface text remains English with a visible note; the
   selected conversation language is unchanged. S3 tests delivery, not obedience
   by an arbitrary client; S4 must test every launched/resumed/delegated agent's
   actual instruction payload.
10. **Given** a trusted folder, **When** an operation writes outside it, reads
    `.env` or SSH/cloud credentials inside it, or follows a link outside it,
    **Then** Maestro's own effect path and the installed Copilot hook deny before
    effects; ordinary non-secret inside writes and outside reads remain eligible.
11. **Given** another folder, **When** it is added to trust, **Then** only an
    explicit user `maestro trust add DIR` action can record the canonical path
    and matching journal approval in user-local state only. A catalog/update/
    config edit or model-supplied approval cannot add it. Non-terminal add needs
    exact `--confirm-path`; omitted confirmation exits 2. Root/HOME/internal
    directory additions refuse. Kernel-internal directories are not tool grants.

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
   or a placeholder/authored/retired resource in a compiled closure is refused.
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
- A parent and child each have a workspace config: the child wins as a whole
  file; omitted child keys use user/default values, not the parent file.
- A discovered config is a link or foreign-writable: warn and skip it without
  parsing. A planned file changes after preview: refuse the write. Never modify
  a parent workspace as a side effect of init.
- EOF, Ctrl-C, resize or terminal failure interrupts init: no unconfirmed write;
  restore terminal state and preserve the same safe recovery contract as C04.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-S3-001**: Author the catalog from zero, without opening an earlier catalog
  before M3. Every resource MUST serve a named architecture 08 row and workflow.
- **FR-S3-002**: Parse Copilot-native Markdown/frontmatter, TOML and JSON as
  bounded, strict typed data. Reject duplicate/unknown keys, duplicate IDs,
  unsupported kinds, missing body sections, dangling references and dependency
  cycles. Enforce [D2's numeric limits](plan.md#security-limits) before unbounded
  allocation. Follow C01's evidence-backed ADR-0005 decision: agent sidecars,
  with `<stem>.agent.md`'s `name:` equal to `<stem>` so each
  `<stem>.maestro.toml` pairs with exactly one agent. Skills use the Agent
  Skills specification's `metadata` field, not inferred support from C01's
  silent unknown-key control. A host warning on skill metadata reopens the
  ADR-0005 sidecar decision; no automatic sidecar default or hidden diagnostic.
- **FR-S3-003**: Check owner and maturity evidence; use only `placeholder`,
  `authored`, `reviewed`, `qualified`, `retired`. S3 compiles declared closures
  whose members are `reviewed`, the highest pre-S4 stage: the declared stage
  plus a named owner on content admitted through OA1's protected-branch
  CODEOWNERS review. C03 checks stage and nonempty owner; C15 enforces protected
  publication. A local authoring check validates a declaration, not completion
  of remote review; no new review-reference field is required. Record each stage
  and owner in the lock and show them in preview/explain. `qualified` requires
  S4 evidence, never a changed label.
  Placeholder/authored resources may be discovered, not compiled into a closure;
  retired resources cannot enter one. A **projectable** reviewed resource has a
  supported native mapping; a **route-eligible** executable closure additionally
  requires every member's S4 qualification and all caller/runtime/trust checks.
  Projecting instructions in convenience mode is not workflow execution.
- **FR-S3-004**: `maestro init` MUST inspect without scripts, compose base plus
  Rust, preview all files including dotfiles, require explicit apply, validate
  composed output and write only the small descriptor, workspace config, lock
  and owned files.
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
  Compilation MUST NOT execute catalog content or publish a bundle exceeding
  any C11 archive entry/count/size/stream/nesting limit. Source validity alone
  does not guarantee the closure fits; writer and reader share D2's `Limits`.
- **FR-S3-009**: Revalidate the bundle schema, every byte digest and compatibility
  on load. Reject undeclared, duplicate, unsafe, oversized or truncated entries
  before any installation becomes current.
- **FR-S3-010**: Verify with pinned `gh`, fixed arguments and bounded time/output;
  bind exact bytes to the expected repository, workflow, issuer, source and
  digest. Trust-root bindings and the version/digest/executable pin are kernel
  authority records, not settings, catalog metadata or PATH discovery. Check
  the pinned executable's digest before every verification launch, even after
  doctor succeeds. `maestro catalog authority set` provisions/rotates these
  records only with D2's terminal or exact `--confirm` proposal approval and a
  journal receipt, never `--yes`, preferences or MCP. Missing, substituted,
  failing or hung verifiers MUST fail closed.
- **FR-S3-011**: Authenticate timestamp, revocation and version-floor records;
  refresh within five minutes during use, expire offline use within 24 hours,
  apply records atomically and resist clock rollback and replay. Offline
  validity ends at `min(signed expiry, issued_at + 24 hours)`; cap longer signed
  expiry and never restart the clock at download/verification. Consult records
  before every install/update and every installed-bundle init, resolve, search,
  route, explain, impact and projection. D1's explicitly labelled authoring mode is
  separate, never an unsigned install.
- **FR-S3-012**: Store scoped installs, components, closures, pins and trust
  state in the kernel with content-addressed artifacts. Installation and update
  MUST switch atomically; a failed update retains the prior valid install.
  Backup/restore MUST NOT restore authority to use old trust records. Keep the
  destination's current publisher roots, gh pin and approval receipt; if absent,
  refuse until explicitly reprovisioned. A pre-rotation backup never replaces
  current authority or reinstates a rotated-out root.
- **FR-S3-013**: Publish from protected manifests CI using the released,
  checksum-pinned compiler, policy tests, per-asset checksums and attestation.
  Every release MUST include an SPDX JSON SBOM generated from the pinned
  component closure and digests, plus instructions to verify all release assets;
  use checksum-pinned toolbelt jaq 3.1.1, with no new library. D2 and the
  release-assets contract fix asset names, tag mapping, checksums/subjects and
  deterministic SBOM time/namespace from source commit time and bundle digest.
  Separate catalog/runtime publisher identities, rotation and emergency
  revocation MUST be tested. The catalog publisher's scheduled trust workflow
  MUST re-issue attested records every six hours independently of content tags;
  an independent hourly freshness check alerts the maintainer/backup on a missed
  run before expiry. Online refresh requires authenticated pinned `gh` (login or
  `GH_TOKEN`); credentials never enter catalog content. `maestro doctor` checks
  the verifier pin and authentication readiness.
- **FR-S3-014**: Every setting MUST have exactly one override class. Preferences
  use explicit flags > workspace file > user-level config > built-in defaults
  for free values only; update consent and budgets narrow across all layers.
  Permissions intersect, prohibitions and checks accumulate,
  secrets remain references, and capability values stay local to their role or
  step. Precedence never makes a locked or widening override acceptable.
- **FR-S3-015**: The project lock MUST pin bundle/component/runtime/host/model
  identities and supported execution profiles. Updates require an explicit
  command or FR-S3-033's safe-boundary policy; session pins never change mid-task. The lock
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
  Provide `fast`, `balanced` and `deep` profiles for `copilot` and `llamacpp`; only
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
- **FR-S3-027**: Init MUST preview and, only on explicit apply, persist
  `.maestro/config.toml` at its displayed root, leaving ancestors untouched.
  The strict versioned TOML schema stores `language` (the well-formed BCP 47
  subset: 2–3 ASCII-letter language, optional 4-letter script, optional 2-letter
  or 3-digit region), `tone` (`brief`, `normal`, `detailed`), `updates`
  (`off`, `propose`, user-only `auto`) and documented typed `[overrides]`.
  Canonicalize tag casing; refuse all other subtags, unknown/duplicate keys,
  wrong types, unsupported versions and authority fields. Only the canonical
  tag reaches model instructions, quoted as data. Neither preference file holds
  trust, paths or receipts or replaces user authority `config.toml`. Init always
  writes a language; other files may omit it to preserve question-language answers.
- **FR-S3-028**: CLI discovery MUST select the nearest safe current-user-owned
  config, with directory/file ownership and write permissions checked on held
  handles. Inside home stop at home; outside home select nothing until a
  containing root is journal-trusted, then never climb above it. Never select
  drive/mount roots. Foreign-owned, writable-by-others, unreadable or otherwise
  unsafe candidates warn and are skipped without parsing; malformed selected
  safe files refuse. Select one file, never merge ancestors. MCP uses registered
  `--workspace` through these checks, otherwise user preferences, never cwd or
  client roots. Report selected/fallback source without paths in instructions.
  Retain provenance and one fixed snapshot until the next session.
- **FR-S3-029**: Language/tone MUST govern only conversational prose: every
  generated reply, agent reply, ask answer and explanation. Code (including
  comments), commits, file names, identifiers, logs and documentation MUST
  always be English and unaffected by tone. Built-in CLI/TUI interface strings
  ship in `en`, `fr`, `es`; for other languages show one visible note that the
  interface is English while conversation retains the selected language. Keep
  machine contracts, evidence/citation identities and source quotations unchanged.
  An explicit layer language wins; absent one, answer in the question's language.
  Evaluation always ignores session preferences. Unsupported language detection
  is `unchecked`, never a fabricated pass or automatic refusal. Built-in messages
  have one wording per language, independent of tone; `--help`/clap reference
  remain English documentation. Tone never suppresses a warning, refusal,
  citation or mandatory check; changing
  presentation never claims an unmeasured answer profile is calibrated.
- **FR-S3-030**: On a terminal, init MUST provide the owner's "very nice looking,
  AAA, pristine" configuration flow: consistent Maestro brand palette, clear
  hierarchy, visible progress/focus, language/tone choices, allowed overrides,
  complete preview and explicit confirmation. Provide keyboard-only operation,
  screen-reader-friendly plain mode, resize handling, cancel without writes and
  a no-color fallback. Flags plus `--yes` MUST provide the same validation and
  deterministic plan without prompts for CI; `--apply` remains required to write.
  OA9 approved ratatui + crossterm on 2026-09-28 under ADR-0020; adoption still
  requires C05f's minimum-feature/dependency measurements and normal vet gates.
- **FR-S3-031**: The MCP server MUST expose the effective language/tone and the
  English-artifact/log rule in its initialization instructions to Pi, Claude
  Code, Codex and Copilot. Native projections contain only the shared fixed
  English artifact/log rule and the instruction to follow Maestro MCP session
  language/tone, never copied preference values. Maestro-launched agents MUST receive it on start, resume and
  delegation: S3 tests construction/delivery and hands off the real launch test
  as `S4 session-preferences launch-adapter obligation`, not an S3 engine.
- **FR-S3-032**: Unless startup updates are `off`, check for newer verified
  runtime and installed catalog/closure production releases at most once per
  24 hours per installation. Bound network time/output and cache attempts across
  sessions; offline discovery never blocks otherwise valid work. `off` makes
  zero startup discovery network calls; explicit `maestro update check` still
  works. Mandatory trust refresh/expiry is unchanged. MCP never applies updates;
  its client-delivery port provides only a fixed one-line proposal ID/target/
  version notice, never release-note text or commands in model instructions.
- **FR-S3-033**: Resolve updates by narrowing (`off < propose < auto`), not
  last-writer-wins. Default `propose`; only user `preferences.toml` can enable
  `auto`. Workspace/flags can only tighten the user/default ceiling; diagnose
  ignored widenings, including workspace auto over user propose. Budget-like
  keys also narrow across all layers. Propose shows verified changes and one
  exact command; S3 runtime proposals never apply and use the approved installer
  command, not a new updater. Auto applies verified compatible catalogs only at
  idle CLI boundaries. Widened permissions or changed hooks require exact human
  consent; uncertainty/missing consent stays a proposal, never generic `--yes`.
- **FR-S3-034**: Catalog update/rollback MUST reuse C13/C14/C16 verification,
  admission and atomic activation. Every apply retains a rollback target and
  English receipt of old/new identities, trust, policy, approval and result.
  Rollback revalidates current trust, cannot revive revocation or lower floors,
  and records a linked receipt. No activation or pin replacement runs mid-task;
  interruption preserves the prior valid install. Runtime discovery reuses
  verification with its separate publisher; runtime activation/rollback and
  installer handoff remain a named follow-up, not an S3 implementation.
- **FR-S3-035**: Init MUST ask about trust in its canonical root; store answers,
  canonical paths and receipts entirely in user-local kernel authority keyed by
  canonical path. No trust table belongs in workspace or user preferences.
  `maestro trust add/list/remove` manages authority without rewriting preferences.
  Add displays its canonical path and asks default-no on a terminal; without one,
  require exact `--confirm-path DIR`, else exit 2 with the exact command.
  Reject filesystem/drive/mount roots, HOME itself and kernel-internal directories.
  `--yes`, environment variables, MCP/model text, catalogs and updates cannot
  grant trust. Decline allows only a separately confirmed preferences-config
  write with kernel-local metadata, never templates or other effects.
- **FR-S3-036**: A shared workspace-trust policy port MUST check Maestro-owned
  filesystem effects (init, projection, install, update and rollback) and Copilot
  `preToolUse`. Permit non-secret inside reads/writes and outside reads subject
  to other controls; deny outside writes and all built-in secret locations,
  even inside trust. Cover SSH/GPG, cloud/CLI credentials, password/keyring stores
  and `.env` files with immutable checked deny data. Use ADR-0018 canonical
  ancestry/held handles for links, `..`, new-file parents and race resistance.
  Kernel-internal XDG config/data/state writes follow kernel rules and grant
  agents/tools no access. External host targets require explicit folder trust;
  Pi/Codex/Claude tool enforcement waits for the named S4 hook qualification.

### Key Entities

- **Resource**: stable ID, kind, version, named owner, declared maturity, content
  digest and exact references. S3 review assurance is protected-branch CODEOWNERS
  review as defined in FR-S3-003, not a synthetic evidence field or authority grant.
- **Bundle**: immutable compiled entries and closures, compatibility contract and
  source identity, verified independently of authoring checks.
- **Install and trust snapshot**: scoped kernel records tying artifacts and pins
  to authenticated freshness, revocations and a monotonic version floor.
- **Project and ownership manifest**: descriptor and explicit lock plus the
  paths/digests Maestro wrote, including owned portions of shared host settings.
- **Workspace preferences**: strict `.maestro/config.toml` with conversational
  language/tone, update policy and documented overrides; user `preferences.toml`
  supplies lower precedence values. Neither is a grant, lock or secret store.
- **Workspace trust approval**: canonical roots, answer and receipt in user-local
  kernel authority only, never editable preferences or a secret-deny exception.
- **Update proposal and receipt**: exact managed release identities, verified
  change/permission/hook diff, decision and rollback artifacts; never authority
  to skip current trust or replace an active session's pins.
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
- **SC-S3-006**: On C23's frozen 100+ intent suite and digest-pinned synthetic
  bundle/eligibility fixture, the absolute bar is held-out matchable top-1
  ≥ 90 % (OA10 approved by the owner, 2026-09-28, amending D5). The original
  top-3 ≥ 90 % bar is historical; report top-3 as a diagnostic, not acceptance.
  Freeze at least 20 tuning and 80 held-out cases, including at least
  60 held-out matchable cases and ten route-eligible synthetic workflow
  candidates. Record exact cohort sizes and all input digests before scoring.
  Report top-1/top-3, correct no-match, clarification, exact dependency completeness,
  unnecessary context, distractors and per-route p50/p95 latency separately.
  Required closure completeness is 100 %. Under the [D5 protocol](plan.md#d5-baseline-routing-and-measured-hybrid),
  hybrid ships only if the seeded 95 % paired-bootstrap interval for its
  held-out top-1 difference over baseline has a lower bound strictly above zero;
  otherwise ship the passing baseline and retain the comparison. C28 also
  verifies a real M3 install returns `incompatible` ("not qualified until S4");
  synthetic results never qualify it.
- **SC-S3-007**: Catalog discovery and impact rebuilds preserve scoped results
  and snapshot identity; no catalog/knowledge scope leakage is observed.
- **SC-S3-008**: Final CI passes on Linux, macOS and Windows, with line coverage
  at least 90 % overall and 95 % on changed lines, zero missed mutants and zero
  mutation timeouts. Mutation and coverage run in CI only, not on the workstation.
- **SC-S3-009**: M3 evidence names every required traceability row and delivered
  portion, with no missing, duplicate or extra keys; private receipts stay
  private. Missing host, release, S2 or CI evidence blocks the corresponding exit.
- **SC-S3-010**: Automated cases cover `en`/`fr`/`es`/`ja` in the well-formed
  BCP 47 subset × all three tones, all four
  precedence layers, nearest/nested/missing/invalid-file discovery, strict-schema
  refusals, permission non-widening and identical interactive/scripted plans.
  Verify every generated workspace/host deliverable is byte-identical except
  config language/tone values. Kernel/internal ownership metadata is checked
  structurally for the exact corresponding config digest, not byte identity;
  no other deliverable exception exists. Receipts/logs normalize only timestamps
  and IDs. Verify successful
  trusted apply at the root, an untouched ancestor and zero writes on rerun.
  Preserve machine/evidence fields and MCP delivery/source reporting for all
  four clients with/without explicit workspace. Terminal
  snapshots and a keyboard/plain-mode walkthrough cover preview, confirmation,
  cancel, resize, no-color and screen-reader use; all three OS test suites pass.
  Owner visual acceptance is recorded before calling the branded menu complete.
- **SC-S3-011**: Startup-update tests cover off (zero discovery calls), propose,
  user-only catalog auto, runtime propose-only, MCP zero activation, daily throttling,
  concurrent sessions, unverified/expired/revoked releases, mandatory approval
  for widened permissions or changed hooks, offline use, interrupted activation,
  receipts and trusted rollback for catalog updates. An active-task spy
  observes zero activation calls; S3 manages no owner-controlled component.
- **SC-S3-012**: Real file-effect and Copilot-hook tests deny an outside write,
  a secret read inside trust and a symlink/reparse escape with zero effects.
  A trusted-folder addition succeeds only with user approval bound to its
  canonical path; copied configs, updates and model text fail. Fresh-home CI
  uses exact confirm-path; missing confirmation and root/HOME additions refuse.
  A trust addition leaves manually edited preferences untouched. Test a
  declined-trust config-only write, outside non-secret reads, kernel-internal
  isolation and all built-in deny data on Linux/macOS/Windows. Shared port
  contract tests cover every default adapter; changing adapters cannot weaken
  the policy floor.

## Out of Scope

- Workflow execution, durable engine, daemon, broker enforcement, sandbox,
  runtime acceptance and general provider/role qualification: S4. Native hooks
  and filesystem safety are not substitutes for those controls.
- A new Pi extension, provider fallback, a separate catalog service, remote MCP,
  GUI, team deployment or multi-user operations.
- Extra roles, programming-language overlays beyond Rust, built-in interface
  translations beyond `en`/`fr`/`es`, three-way generated-file merge and bulk import
  of an earlier catalog. Tags outside the defined BCP 47 subset are not supported.
  Comparison is a separately gated post-M3 task.
- Automatic runtime activation/rollback and roots-based MCP workspace detection:
  named follow-ups in 06/08; no unqualified handoff or instructions notification.
- Updates to Cargo dependencies, host clients, models, pinned `gh`, Qdrant setup
  or the router runtime. Owner-managed components have the named notify-only
  follow-up in architecture 06/08, not an S3 automatic installation path.
- S1 cleanup, answer calibration, corpus deduplication, Qdrant Edge, unrelated
  gate changes and the post-M1 backlog. Reuse approved privacy checks when they
  land; do not fold their implementation into S3.

## Traceability

Architecture 08 statuses are design dispositions, not proof of delivery.
C00's [machine-checked inventory](traceability.json) records the 85 exact keys
below, their S3 portions, tasks and named remaining slices, plus six explicit
S5-only exclusions. All included portions are planned; C28 adds integrated
evidence. Combined keys remain verbatim. The frozen exact inventory at
`746df13` is approved by the owner, 2026-09-28 (C00 inventory approval).
This ruling refines portions, never the approved key set. The conventions check
derives candidates from 08's catalog/S3 links and S3 statuses, rejects an
unmapped candidate, and checks both this table and the JSON with negative cases.

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
| `chat.M006 release` | Trusted catalog lifecycle | C13, C13a, C14, C15, C16, C16b |
| `chat.M019 maturity, M023 step 4` | Evidence checks; role qualification stays S4 | C03, C21, C21b, C24a |
| `chat.M019 descriptor` | C01 agent sidecars and specification-backed skill metadata | C01, C03 |
| `chat.M019 ownership` | Required owners/CODEOWNERS; dual-approval operation stays S5 | C02, C21, C21b |
| `chat.M027 detached` | No checkout or toolchain on the laptop | C16, C28 |
| `chat.M027 validation split` | Author/load validation; governed enforcement stays S4 | C03, C11, C13, C13a |
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
| `chat.M048 real controls` | Real verification/Cedar and allow/deny neighbours | C09, C13, C13a, C19, C28 |
| `chat.M057 CI` | Zero relevant tests cannot pass; honest statuses | C15, C28 |
| `chat.M059 provenance` | Observed/unsupported/not-run kept distinct | C08, C18, C28 |
| `delivery.U02` | Catalog consistency; translation comparison after M3 | C03, C21, C21b, C29 |
| `delivery.U05` | Consumption, compiler, closure and overrides | C10, C11, C17, C22a, C22b |
| `delivery.U09` | Deterministic bootstrap and projection | C04a, C04, C05, C06, C07 |
| `delivery.U13` | Catalog routing; S1 retains retrieval/model qualification | C23, C24a, C24, C25, C26 |
| `delivery.U17` | Bundle trust/lifecycle; general InnerSource stays S5 | C09, C15, C16, C16b, C28 |
| `core storage` | Scoped kernel authority, rebuildable projections | C12, C25, C27a |
| `core verification` | Organization gates, no S3 exception | C28 |
| `delivery.§1.5` | Exact S3 delivery evidence; other slices retain their portions | C00, C28 |
| `chat.M006 CLI` | Bootstrap, projection, doctor, catalog lifecycle, settings and policy CLI; other commands stay S4/S5 | C05, C06, C07, C13, C13a, C16, C16b, C17, C18, C19 |
| `chat.M036 objects` | Catalog resource kinds and declarative references; runtime objects stay S4 | C03, C21, C21b |
| `delivery.§2.2` | Catalog-release contract; runtime envelopes stay S4 and ingestion policy stays S6 | C10, C11 |
| `chat.M019 roles` | Earlier roles deferred to C29 comparison after M3, not seeded | C29 |
| `chat.M023 step 13` | Stop-on-collision owned writes; three-way merge remains separately specified | C04, C05 |
| `delivery.C01 transparent platform, minimal plumbing` | Bootstrap and preferences remove plumbing, not human judgement | C05, C17, C18 |
| `delivery.C02 detached two-repository releases` | Separate catalog/runtime publishers and detached install | C09, C15, C16, C28 |
| `delivery.C04 readiness, owners, dependencies without invalid frontmatter` | Strict descriptors, owners and readiness evidence; role qualification stays S4 | C01, C02, C03, C21, C21b |
| `delivery.C06 closure, merge, explanation, one override class` | Exact closure, restrictive resolution and one explained override class | C03, C17, C18, C22a, C22b |
| `delivery.C08 explicit instructions and skills with provenance` | Required skills/instructions and native projection provenance; session composition stays S4 | C02, C06, C07, C21, C21b |
| `delivery.C12 preview/apply bootstrap` | Safe deterministic preview/apply bootstrap | C04a, C04, C05 |
| `delivery.C13 native projection` | Copilot/Pi convenience projection and Copilot policy hook | C06, C07, C20 |
| `delivery.C14–C16 catalog MCP, shared Qdrant, separate knowledge ACLs` | Local catalog MCP and scoped cards through the shared knowledge pipeline | C24a, C24, C25, C26 |
| `delivery.C17 signed releases, freshness, revocation, transparency` | Verified catalog lifecycle, current trust and honest transparency | C09, C13, C13a, C14, C15, C16, C16b, C18 |
| `delivery.R01–R11 corrections` | R05 workflow-first routing and R09 trust; other corrections stay S4/S5/S7 | C14, C16b, C24a, C24 |
| `product.GD1–GD5` | Four-client local MCP and Copilot hook; GD1 registry keeps its existing §6/05 ownership | C00, C06, C07, C08, C20, C28 |

Architecture [08 §21](../../docs/architecture/08-traceability.md#21-s3-contract-inventory-c00-not-delivery-evidence)
records D1/D5, concurrent S1/S2/S3 starts, the S3/S4 compiler boundary, the
`product.GD2, GD4, GD5` hook disposition and C27a's S2 G27 seam, consistent with
03/06. This is contract evidence only; the inventory is approved by the owner,
2026-09-28 (C00 inventory approval). C09 owns closure of 08 §17's "Publisher
identities, trust roots, key rotation procedure" row with D2 and OA4 evidence;
without that evidence
C28 remains blocked.
Runtime enforcement, general qualification and InnerSource keep their named
later slice. No similarity or unapproved closure fallback replaces C27a/C27.

### Owner amendment coverage (2026-09-28 11:25–12:05)

The frozen C00 rows above stay unchanged. These requirements refine existing
rows, so no new traceability JSON row or inventory-test change is needed:

| Requirement | Existing row family | Tasks |
| --- | --- | --- |
| FR-S3-027 | Bootstrap/settings: `owner.m001.cli`, `owner.m001.load`, `chat.M019 bootstrap, M023 step 13` | C17, C05a, C05j, C05g, C08 |
| FR-S3-028 | Settings: `chat.M036 classes, M039 policy`, `chat.M006 explain` | C05b, C08 |
| FR-S3-029 | Preferences: `owner.m001.load`, `chat.M036 classes, M039 policy` | C05c, C05l, C05d, C08 |
| FR-S3-030 | Bootstrap: `owner.m001.cli`, `owner.m001.load` | C05g, C08, C05f, C05k |
| FR-S3-031 | Instructions: `delivery.C08 explicit instructions and skills with provenance`, `product.GD2, GD4, GD5` | C05e, C06, C07, C08 |
| FR-S3-032 | Releases: `chat.M006 release`, `chat.M027 TUF`, `owner.m024` | C16c, C16e, C16g, C16f |
| FR-S3-033 | Settings/releases: `chat.M036 classes, M039 policy`, `delivery.C17 signed releases, freshness, revocation, transparency` | C17, C05a, C16c, C16e, C16g, C16f |
| FR-S3-034 | Lifecycle: `chat.M006 release`, `delivery.C17 signed releases, freshness, revocation, transparency`, `owner.m024` | C16d, C16e, C16f |
| FR-S3-035 | User authority: `owner.m001.guardrails`, `owner.m032` | C05h, C05j, C05g, C08 |
| FR-S3-036 | Guardrails: `owner.m001.guardrails`, `chat.M036 classes, M039 policy`, `chat.M006 destructive` | C05h, C05i, C05j, C05e, C06, C07, C08, C16d, C16f, C20 |
| SC-S3-010 | Delivery proof: `delivery.§1.5` | C05a, C05b, C05j, C05c, C05l, C05d, C05e, C05g, C06, C07, C08, C05f, C05k, C16d, C28 |
| SC-S3-011 | Delivery proof: `delivery.§1.5` | C16c, C16d, C16e, C16g, C16f, C28 |
| SC-S3-012 | Delivery proof: `delivery.§1.5` | C05h, C05i, C05j, C05g, C06, C07, C08, C16d, C16f, C20, C28 |

C28 must include these supplemental portions in its existing-row evidence.
They do not claim delivery or S4 launch qualification. The named S4 handoff and
owner-managed notify-only follow-up are recorded in architecture 06/08.

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
