# Feature Specification: Catalog

**Feature Branch**: `docs/s3-manifest-v4`, from `feat/s3-integration` at `815ff33`

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
**explicit flags > workspace file > user-level config > pinned manifest defaults**;
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
S3 enforces this on its own file effects; all live host hooks, including
Copilot, remain the named S4 obligation after the v4 amendment. Kernel-internal XDG config/data/state
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

### Owner amendments, 2026-09-28 evening

The manifest must grow by adding kind descriptors and fixtures, not by editing
checker/compiler/installer branches. C03 owns the data-driven registry (6 h):
serde-able built-in descriptors with a loader seam, not file-loaded kinds yet.
Existing input/resource limits remain security bounds, not an unlimited-memory
promise. Model cards are the only new production kind in this amendment.

Under [ADR-0011](../../docs/adr/0011-models-chosen-by-bake-off.md), bake-off winners
arrive as versioned manifest changes approved by the owner. The manifest declares
the model and job; the kernel retains immutable cards, evaluations and selections.
Each answer carries its registry card ID, which resolves to an immutable card;
the kernel stores no answers. Declaration, registration, explicit selection
records and caller-observed answers are different states. M059's agent-session model
profiles remain separate; neither a profile preference nor installing a catalog
selects a kernel model.

S1's `feat/s1-settings` work owns the shared settings registry, strict user/project
parsing, `config get/set/unset/list/explain` and journalled changes. C05a, C05b,
C05d, C17 and C18 extend that registry rather than build another. C05h trust,
catalog classes, locks and session delivery sit above it. The supervisor confirmed
that the landed S3 rules prevail: user `preferences.toml` stays separate from
authority `config.toml`, workspace discovery is safely bounded, MCP uses explicit
`--workspace` or user preferences, tags/tones keep their current grammar, and
evaluation ignores preferences. No default-workspace setting is introduced.
**Supervisor ruling, 21:02: S1 registry names are canonical**; for example,
`ask.output_tokens` replaces the catalog's `max_output_tokens` spelling. C17
supplies only missing catalog descriptors through that shared registry, not a
second known-key list. The supervisor synchronizes the required S1 commits into
S3 before C17; the older S3 base is not evidence those APIs are integrated.

**Owner additions, 20:48 and 20:50.** `maestro init` opens the ratatui menu;
`maestro config` without arguments opens the same settings editor. Its entries
come from the S1 registry: every setting shows its current value, allowed values,
one-line description and source layer. New descriptors appear without per-setting
screen code. Editable overrides come from descriptors with type, range, class
and permitted layers; `model_profile` and `routing_candidates` are examples,
not a second allowlist (owner's 20:48 amendment). Editing remains subject to
class, scope and authority restrictions; locked or authority-only entries explain
their restriction, not a preference bypass. C05g provides the shared plain flow
first; C05k adds the renderer.

After the glossary spike shows value, the glossary and source-class table become
reviewed, versioned catalog kinds per collection, not only kernel bindings. They
are named later work, not additional production kinds or M3 tasks in this amendment.

### Manifest v4 amendment, 2026-09-30

The owner approved the final manifest design, minimal Phase 1 and the gap
additions. This supersedes the earlier owner-first tree, single-owner rule,
post-M3-only recovery and S3 live-Copilot-hook requirement. Durable decisions
are in [ADR-0022](../../docs/adr/0022-manifest-layout-v4-and-language-neutral-extensions.md).
The detailed approved records are `manifest-design-final.md` (§§2–8) and
`manifest-gap-analysis.md` (§§3, 6–7), dated 2026-09-30; the public implementation
contract is [D13](plan.md#d13-manifest-v4-source-contract) and [tasks.md](tasks.md).

- **Phase 1/M3: 149 amendment hours**, comprising minimal design 132 h,
  gaps 16 h and the 20:45 source-policy amendment 1 h. Keep signed catalog bundles, compatibility, exact versions/digests,
  existing init presets, Cedar, local trust, safe removal and all M3 quality bars.
- **Phase 2 immediately after M3: 121 h**: minimal deferrals 39 h, original
  Phase 2 40 h, gaps 42 h. ST1 switches standards authority; A0 adds authoring;
  R1 recovers deferred content; L1 adds independent package lifecycle; X1 adds
  extensions before S4; E1 adds package evals; A1 adds catalog-wide explain.
- **Supervisor correction, 2026-09-30:** gap G07–G11 guides/guard evidence are
  Phase 1 (12 h); G12 per-host context accounting is Phase 2 (6 h). Its typed
  contract is specified now; no fabricated ceiling or Phase 1 pass is allowed.
- **C20 moves to S4 with its 4 h**, not a saving. S3 checks ten hook points and
  host mappings only. Any-language extensions run out of process; Maestro's
  own implementation stays Rust. MCP carries actions, not durable event delivery.
- **One pending `/2` cutover:** common, core, standard, language and team areas
  use `package.toml`; obsolete capability IDs/paths refuse rather than rebind.
  Mandatory framework availability never means injecting every persona into
  knowledge-only context. Root standards are non-removable; languages are
  reusable declarations, not seven newly qualified non-Rust gates.

Earlier scope decisions remain historical evidence only where this amendment
explicitly replaces them. Old-content recovery is owner-approved before M3,
but no private Control-M material is authorized. Landed C05a/C05b/C05f/C19
stay closed; C46/C47/C61 carry new wiring rather than reopening that work.

### Manifest-owned URL rules, owner decision 2026-09-30 20:45

The manifest is the one editable home for URL rules and their definitions.
A knowledge source carries strict JSON policy/decision/promotion/migration
records beside `source.toml`; collections hold only their declaration and exact
source-policy reference. Reviewed rule admission is the signed digest-pinned
catalog release plus the owner/maintainer approvals recorded by the ownership
rules. No policy JSON self-assertion supplies approval or local access rights.

C66 grows from 3 to 4 h; C52a/b, C68 and C41/C43 retain their hours by reusing
existing typed schema/checkpoint/collection consumers. Phase 1 is **149 h**;
Phase 2 remains **121 h**. S6 alone owns the catalog-backed `PolicySource` /
`ResourceSource` runtime adapter; its separate task is not an M3 prerequisite.
C66 needs the existing core wire types synchronized, including N07's
`IdentityMigration` after its review fixes land. Private URL inventories use
C69's admitted private manifest packages; C42's restricted mount is not widened.
See [D13's source-rule contract](plan.md#manifest-owned-source-rules).

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
2. **Given** core and Rust templates, **When** each composition is applied,
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
   keyboard-accessible menu shows workspace/preset, every registry setting with
   its value, allowed values, description and source, and the canonical workspace
   trust decision, followed by a complete
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
   explicit flags > workspace file > user-level config > pinned manifest defaults,
   per key, with every source explained. Only explicitly supplied flags win;
   parser defaults never hide the workspace or user value.
3. **Given** a wider permission, removed check or larger budget, **When** proposed
   as an override, **Then** permissions still intersect, checks accumulate and
   budgets narrow; locked changes are refused.
4. **Given** a checked hook subscription, **When** S3 validates it, **Then**
   its point, tool references, timeout and mapped/unsupported/unqualified host
   state are explicit. Offline Cedar fixtures deny missing facts and evaluator
   errors. Actual Copilot allow/deny/error-to-deny effects are S4 evidence.
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
    **Then** Maestro's own effect path denies before effects; live host-hook
    enforcement is S4. Ordinary non-secret inside writes and outside reads
    remain eligible under the other mandatory controls.
11. **Given** another folder, **When** it is added to trust, **Then** only an
    explicit user `maestro trust add DIR` action can record the canonical path
    and matching journal approval in user-local state only. A catalog/update/
    config edit or model-supplied approval cannot add it. Non-terminal add needs
    exact `--confirm-path`; omitted confirmation exits 2. Root/HOME/internal
    directory additions refuse. Kernel-internal directories are not tool grants.
12. **Given** a new setting descriptor, **When** I open `maestro config` with no
    arguments or init's settings menu, **Then** it appears automatically in both
    plain and terminal views with its current value, allowed values, description
    and source layer. Allowed edits use S1 validation/journalling; locked entries
    stay visible but cannot be changed through preferences.

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
2. **Given** `feature-delivery` and `workload-question`, **When** compiled,
   **Then** all twelve architecture 03 §2.3 rules hold for supported constructs;
   the builder is a deterministic step and reviews remain independent.
3. **Given** skill scripts, hooks or templates, **When** compiled,
   **Then** they remain data; compilation never runs them.
4. **Given** an untrusted pull request, **When** CI checks it, **Then** it has no
   signing or production credentials; only the protected release path attests.
5. **Given** a synthetic new kind descriptor, **When** the unchanged generic
   machinery checks, compiles, reads and installs it, **Then** its fields,
   references and digests survive; removing the descriptor refuses the kind.
6. **Given** an owner-approved model-card declaration, **When** explicitly
   registered from an admitted catalog on its qualifying machine with all
   evidence already local, **Then** the kernel records the exact immutable
   identity without fabricating evaluation/selection records. Each returned
   answer carries a registry card ID; the kernel stores no answers. S1's
   latest-registration answerer lookup may use the new card on a later ask for
   the same router entry; registering A, then B, then A again still resolves B.
   Manifest changes cannot rewrite the immutable card named by an earlier answer.

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

- **FR-S3-001**: Author only approved required content, using the owner-approved recovery
  inventory before M3. Preserve provenance, attribution and current authority
  boundaries; do not import placeholders or private material. Every resource
  MUST serve named architecture 08 rows and a declared selection; workflow
  labels are usage metadata, not reverse dependency or authority edges.
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
  C03's source formats are Markdown/frontmatter and TOML; JSON resource-format
  support arrives later with its contract consumer, not in C03 or C03a.
- **FR-S3-003**: Check area-derived ownership and maturity evidence; use only `placeholder`,
  `authored`, `reviewed`, `qualified`, `retired`. A compiled S3 closure requires
  reviewed members and protected owner/maintainer review under FR-S3-041/058.
  Offline checking validates shape, not completed remote approval. Record each
  member's maturity and owning area in bundle, lock, preview and explain.
  Placeholder/authored/retired members cannot enter a compiled closure. A
  supported native mapping makes reviewed content projectable, not executable;
  route eligibility also needs S4 qualification and caller/runtime/trust checks.
  Package `active`/`deprecated`/`retired` status is separate from maturity.
- **FR-S3-004**: `maestro init` MUST inspect without scripts and compose mandatory
  common/core/standards with the selected language/package closure. The first
  proof covers the base and base-plus-Rust compositions. Preview all files
  including dotfiles, require explicit apply, validate composed output and
  write only the small descriptor, workspace config, lock and owned files.
- **FR-S3-005**: A shared owned-file writer MUST resist traversal, links, races
  and changed previews, stop on collision, recover interrupted operations and
  remove only unchanged owned content on all three supported operating systems.
- **FR-S3-006**: Project to Copilot and Pi with explicit tools, dependencies and
  provider loading, no model fallback, collision/shadow checks, preserved
  unrelated registrations and honest registered/observed/stale/failed states.
  Native projection MUST rewrite area-qualified names, paths and MCP
  references together, reserve native agent alias `maestro` for core, and refuse
  host length/normalization collisions. Source names remain local; live probes
  qualify projected aliases rather than assume hosts accept slashes.
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
- **FR-S3-014**: Every setting MUST have exactly one override class. Catalog
  checking MUST consume S1 registry keys through a port, not a catalog-private
  `KNOWN_SETTINGS` list. S1 names are canonical under the 21:02 supervisor ruling;
  C17 adds missing catalog descriptors without duplicating overlapping keys.
  Preferences
  use explicit flags > workspace file > user-level config > pinned manifest defaults
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
- **FR-S3-017**: S3 MUST validate subscriptions at exactly ten points: session-start,
  session-end, prompt-submit, pre-tool, post-tool, agent-stop, subagent-stop,
  pre-compact, post-compact and notification. Each point implicitly binds an
  engine/checker-owned platform event protocol, never a catalog event-contract
  dependency. Validate target/tool references, limits and failure behavior;
  every approved host mapping records mapped, unsupported or unqualified for
  every point, with adapter/native event and evidence for a mapped point.
  S3 launches no hook. C20's Copilot normalization, live allow/deny/error-to-deny
  proof and 4 h move to S4 alongside trusted identity, ordering, replay,
  acknowledgments, timeouts and other host qualification. Unknown facts and
  unsupported mediation deny; an absent hook is unprotected.
- **FR-S3-018**: Declare `workload-question` in the application-workflow package and
  `feature-delivery` in core. Phase 1 has ten core personas: maestro, planner,
  researcher, worker, tester, reviewer, builder, releaser, steward, bootstrapper.
  Phase 2 adds scaffolder and the team-owned Translator; no `coder` alias or
  placeholder recovery. Builder remains a deterministic workflow step; persona
  prose grants neither LLM execution nor build/release authority. Keep
  fast/balanced/deep agent-session profiles distinct from quality profiles and
  kernel cards; unsupported provider/role combinations remain ineligible.
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
- **FR-S3-026**: Before M3, C29 MUST audit all 395 authorized legacy manifest/template
  files exactly once by relative path, digest, attribution, destination,
  disposition and rationale. Recovery is limited to named bounded tasks;
  minimal Phase 1 explicitly identifies Phase 2 deferrals rather than claiming
  them shipped. Preserve rule IDs and approval obligations; no bulk unchecked
  copying, private data access or competing normative authority is authorized.
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
  hierarchy, visible progress/focus, complete preview and explicit confirmation.
  The same settings editor MUST open on `maestro config` without arguments and
  cover every S1 registry setting: current value, allowed values, one-line
  description and source layer. Generate entries from descriptors; adding a
  setting MUST require no per-setting screen code. Allow every authorized edit
  through S1 validation/journalling, while locked/authority-only entries show why
  editing is refused; no menu bypasses classes, scope, trust or consent.
  Provide keyboard-only operation,
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
- **FR-S3-036**: A shared workspace-trust policy port MUST check Maestro-owned filesystem
  effects (init, projection, install, update and rollback). Permit non-secret
  inside reads/writes and outside reads subject to other controls; deny outside
  writes and immutable secret locations, even inside trust. Cover SSH/GPG,
  cloud/CLI credentials, password/keyring stores and `.env` files with checked
  deny data. ADR-0018 canonical ancestry/held handles protect links, traversal,
  new-file parents and races. Kernel-internal XDG writes use kernel authority
  and grant tools no access; external host targets require explicit trust.
  C20 transfers live Copilot use of this port to S4 with all other host hooks;
  S3 file-effect evidence is not host-process containment.
- **FR-S3-037**: A resource kind expressible with the supported formats and
  operations MUST require only one declarative descriptor/registry entry plus
  positive and refusal fixtures. Shared checker, compiler, reader and installer
  code MUST NOT change. The descriptor names its version, source shape, strict
  fields, references and constraints. A built-in `Registration` pairs a descriptor
  with an existing hook, named in the descriptor and resolved against the fixed
  hook table; unknown hook names refuse. C03 supplies finite TOML floats
  (non-finite values refuse) and a structured-table field type passed whole to
  `KindRules`, so nested model identities need no generic parser change in C03a.
  Descriptors never supply executable code. New special semantics belong in an isolated,
  separately tested hook/consumer, not kind switches in generic machinery.
  [D12](plan.md#d12-kind-extensibility-and-model-cards) lists permitted and forbidden
  changes. Unknown kinds/versions still refuse; bounds, trust and ownership apply
  to every kind. Built-in serde-able descriptors and a loader seam suffice in S3;
  loading descriptors from files or executing plugins is not required. JSON is a
  later source-format addition; the guarantee applies to supported formats.
- **FR-S3-038**: Add `model-card` as reviewed, versioned catalog content with
  common resource metadata, a model-card `version` field, and the exact kernel
  v2 `CardIdentity`, including role, logical router entry, model-file fingerprint,
  sampling, output limit,
  chat-template digest/explicit absence and qualification provenance. Preserve
  every v2 field and unsupported/unavailable state, not a reduced parallel schema.
  Author human-facing declarations in area-relative
  `llm/models/<role>/<name>.toml`, starting in `core/llm/models/`; require the
  path role to match the kernel identity and refuse unregistered roles;
  the kernel owns canonical card JSON and its fingerprint. No secrets or machine
  paths are permitted; bindings stay local. Intended jobs are embedder, reranker,
  answerer, extractor and query_expander, but unsupported kernel roles refuse,
  never alias/fallback. Use the kernel's own unknown-role refusal, not a hard-coded
  planned-role table. Extractor support depends on S2 G17, integrated before M3
  with S2; query_expander remains unsupported until S1's named role follow-up. M059 owner-relative `profiles/models/` and `model_profile`
  continue to configure agent sessions; they neither replace these cards nor
  qualify or select a kernel job.
- **FR-S3-039**: Maestro MUST check the declaration before explicit, scoped
  registration through the existing kernel model registry. Installed registration
  uses the same current trust/compatibility admission as other catalog consumers;
  missing qualification artifacts, invalid identity, unsupported role or denied
  scope refuses without a registration/selection change. Registration succeeds
  only where all identity-referenced evidence is already in the local kernel;
  rely on the existing transactional pin refusal if an artifact is absent.
  Each machine qualifies its own card: backend, runtime-binary digest and reference
  hardware are identity fields. M3 has no cross-machine evidence import; a local,
  digest-matched `--evidence DIR` import is named post-M1 work.
  Registration is idempotent and never implicit in check, compile, install or
  update. Each answer carries its registry card ID, which resolves to an immutable
  card; the kernel stores no answers. Manifest edits, updates and removal cannot
  rewrite that card. Preserve S1's current answerer lookup: explicit registration
  under an existing router entry can change which card a later ask resolves,
  without a selection record. Re-registering an earlier card does not restore it.
  S1's named post-M1 explicit-answerer-selection follow-up owns that fix, not S3.
  New bake-off winners require an owner-approved manifest change; existing
  selection-record APIs still require exact-card real evaluation. A reviewed
  declaration is neither a real evaluation nor S4 qualification. No model
  download, router reconfiguration or fabricated evaluation/selection is added.
- **FR-S3-040**: Discover only the bounded registered v4 areas: common at root, core,
  `standards/<domain>`, `languages/<language>` and
  `capabilities/<group>/<name>`, plus registered shared support directories.
  Instantiate only nonempty registered content. Common has no agents and no
  root instructions/policies; universal normative content lives in standards.
  Reject nested areas, unsupported nonempty trees, overlapping placements,
  symlinks and traversal. Every asset is explicitly inventoried, owner-local
  and counted under aggregate limits across all sources. Nonempty collections
  remain unsupported until S6; extension trees until Phase 2/X1.
- **FR-S3-041**: Each area MUST have exactly one `package.toml`, including root common,
  core, every language, standard and team package. Require kind/name, exact
  SemVer version, description, nonempty `owners`, optional `maintainers`
  (default empty), maturity/rows, explicit requirements and lifecycle status.
  Owners and maintainers are GitHub users or organization-team slugs. Resource
  ownership derives from its area, never another editable owner list. Reject
  malformed/duplicate principals, duplicate namespaces and owner-reference
  mismatch; groups confer no inherited approval authority. Owners approve
  descriptor/delegation changes; maintainers can review content only.
- **FR-S3-042**: All dependencies MUST use declared typed qualified IDs. Global loads
  every pinned standard, common and required languages; framework adds core;
  selected packages add explicit closures in stable topological order. Common
  and standards cannot require core or optional packages; core cannot require
  team packages; languages may require global areas or other languages, never
  team packages. Cycles, undeclared/native references and cross-root includes
  refuse. Common, core and all admitted standards appear exactly once in every
  preset's lock/forward closure. Usage labels confer no dependency or grant;
  core labels can name only core workflows. Framework delivery declarations
  are available in every install, but knowledge-only native context exposes
  only Maestro, its required instructions/skills and selected MCP tools.
  Package removal preserves immutable common/core and unrelated selections.
- **FR-S3-043**: A selected package MUST be removable through one safe-boundary
  transaction with its lock, selection, receipt and unchanged owned projections.
  Refuse mandatory-root removal, live reverse dependencies and edited-file
  collisions. Preserve referenced shared/transitive dependencies, consumer
  data, model cards and kernel evidence. Removing its source folder makes
  surviving dangling references fail with source and missing qualified ID;
  no basename fallback or silent deletion changes another selection.
- **FR-S3-044**: The S6 private overlay MUST be explicit, pinned and additive
  only, through the same `SourceTree` checking seam. Public checking runs alone
  first, followed by a combined check with aggregate limits and source-aware
  diagnostics/locks. Refuse duplicate IDs/paths even with identical bytes,
  public/core shadowing, owner/trust overrides, symlinks and self-authorized
  publishers. The CTM mount is restricted to its approved collection subtree
  and private preset; it cannot supply a second owner manifest. Public presets
  and their transitive closures MUST never require private IDs. Missing or
  unauthorized private sources disable only private selection, not public use.
  Public CI/releases MUST never fetch, package or index private inputs.
- **FR-S3-045**: Generate `.github/CODEOWNERS` from area descriptors only. Content rules
  name owners plus maintainers; later descriptor/exception rules name owners
  only. Root owners protect the generator, CI, ownership policy and generated
  file. Exact last-match tests MUST prevent a broad rule undoing protection.
  CI refuses missing, extra, changed and stale rules. Standard exceptions also
  require central root-owner approval; the generated file does not establish
  independent quorums, identity validity or repository protection.
- **FR-S3-046**: Keep one pending `maestro-source/2` cutover with resource IDs
  `kind:namespace/local-name`; area IDs are `package:common`, `package:core`,
  `package:<name>`, `language:<name>`, `standard:<name>`, and presets
  `preset:<name>`. A language/standard has no package alias. All area namespaces
  are globally unique; local names are unique within `(kind, namespace)`,
  including across that kind's hook-point/model-role folders. Segments retain
  the lowercase-hyphen grammar and 64-character maximum. Same stem in different
  kinds is valid. Refuse duplicate IDs/paths/placements and old `/1`, mixed
  layouts, `capability.toml`, `capability:` IDs and old locks with migration or
  fresh-preview diagnostics. Increment changed descriptor versions and keep
  check/project/authoring-lock `/2`; kernel fingerprints and S1 preferences
  do not change.
- **FR-S3-047**: Presets and explicit package selection MUST resolve the same checked
  mandatory roots and package/language closures. Preset `templates` selects
  area/inventory names, never arbitrary paths. Each area-local bootstrap
  inventory lists exact source-to-output files, binding/tool requirements and
  digests. Include common and selected language inventories once; refuse even
  identical-byte collisions from distinct sources. Lock every area descriptor,
  resource, sidecar, config/default, rule/exception, inventory and declared asset
  with source identity/revision/digest. Changed input requires fresh preview.
  Preserve C04 held-handle safety, strict generated JSON, zero writes on cancel,
  owned removal and zero content execution/tool installation.
- **FR-S3-048**: The deferred S6 collection descriptor MUST carry strict
  `knowledge/collections/<name>/collection.json` with its catalog metadata
  sidecar, using ADR-0014 and the core collection declaration. It references
  source rules by exact identity/digest; no `[approved_urls]`, copied exclusion
  registry or collection-local policy overrides. C41/C43 check referenced-source
  ownership, visibility and exact rule/decision/migration closure, not another
  URL-rule language. Source policies govern seeds, discovered URLs, redirects,
  denial/promotion/expiry and identity migration. S6's separately tasked adapter
  rechecks current signed/reviewed admission and local access before effects.
  Preserve private provenance, exclusion precedence and original retention;
  check/install fetch nothing. No crawler, private URL inventory or new local
  entitlement is authorized by this document.
- **FR-S3-049**: Use functional path/file/ID names; products are typed values. A single
  checker-owned exception table permits only exact approved host directories
  `core/hosts/{pi,claude-code,copilot}` and registered host/tool-required native
  filenames at exact placements (including inert starter outputs). Reject
  registered product names/aliases elsewhere using the shared adapter/host
  registry; no wildcard or package-authored exceptions. New spellings need
  reviewed registry coverage, not a claim to recognize every trademark.
- **FR-S3-050**: Core backend bases live at `core/backends/{graphdb,vectordb,mcp}/config.toml`.
  Each has a type and fully checked supported-type tables; defaults and common
  `settings/defaults.toml` fill one lowest S1 defaults slot, one producer per
  key. Keep canonical `graph.engine`, functional `graphdb.*` knobs, explicit
  `none` zero-call mode and unavailable-adapter refusal before native calls.
  Preserve approved graph bounds/locked handles/checkpoint behavior in D14,
  qualified compiled metadata and Qdrant identity/lifecycle. Package role
  configs may only add namespaced bindings or descriptor-approved narrowings,
  selected exactly by `backend_extensions`; type/endpoint/base replacement,
  collisions, wider grants and aggregate-limit resets refuse. MCP records are
  config entries, not `mcp:` resources; retire old files/edges with diagnostics.
  The effective MCP view derives from immutable core plus selected records;
  install/removal never rewrites signed core or launches a server.
- **FR-S3-051**: Export schemas/editor associations from the pinned descriptor/settings
  registry and owning semantic validators, not copied validators. Cover every
  registered kind and checked config shape with authored valid/refusal fixtures
  and expected diagnostic/allowed neighbour. Missing, zero, extra or stale
  coverage fails. Four checkpoints (editor, pre-commit, exact-commit CI,
  verified install) share one checker; schema success alone is not semantic,
  review, trust or runtime evidence. Drift checks cover schemas, ownership and
  generated index/navigation. CI uses trusted base code for identity checks.
- **FR-S3-052**: Phase 1 MUST keep exact package/dependency versions, runtime/descriptor/
  host/tool compatibility and signed whole-catalog releases. Add/remove selects
  components of an admitted pinned bundle, never an unsigned independent source.
  Phase 2/L1 adds immutable independent/private releases, stable exact or bounded
  interval resolution, keep-compatible-lock then highest-admitted-stable choice,
  and explicit deprecation. No latest/wildcard/caret or republished bytes.
  Ordinary removal waits at least one released minor and 90 days from signed
  deprecation, in a new major; authenticated security revocation can act now.
  Reuse five-minute refresh/24-hour offline expiry, current trust, consent,
  rollback, checksums, attestation and SBOM. Owners do not self-authorize
  publishers; public CI never fetches or discloses private metadata.
- **FR-S3-053**: Phase 2/X1 extensions MAY use any implementation language, but run only
  out of process under ADR-0013; Maestro's own code stays Rust. Require type,
  tools or hook-subscriber kind, runtime versions, verified relative entry,
  MCP requirements, typed tool/config contracts, secret references, egress
  ceiling and tests/evals. Code is exactly one explicit local inventory or
  immutable signed release with digest/platform assets/expected signer.
  Refuse mutable releases, shell strings, install/build scripts and entries
  escaping verified artifacts. MCP carries every action; durable subscriptions,
  cursors, supervision, limits and launch qualification remain S4. Each extension
  has a separately scoped trusted Cedar grant bound to principal/package/code
  digest/destinations. Declaration is a request, never permission; missing
  facts, invalid consent, evaluator errors or unsupported mediation deny.
- **FR-S3-054**: Registered secret-bearing fields MUST accept only a typed one-of
  environment-variable or keychain-service/account reference. Refuse literals,
  credential-bearing URLs, environment-value maps and argument/default channels.
  Check/index/install/explain never resolve secrets or place them in locks,
  receipts or indexes. Runtime missing binding refuses, not empty credentials.
  Scanning remains mandatory but cannot prove arbitrary prose secret-free.
- **FR-S3-055**: Register prompt, handoff, contract and eval/source kinds with strict
  formats in D13. Common prompt input/output contracts live in root `contracts/`;
  framework contracts stay in core. JSON Schema 2020-12 references are local,
  declared and digest-locked; external HTTP references refuse. Prompt bodies,
  handoff sections/sender/recipient and source provenance/bindings are checked.
  A knowledge source owns the admitted URL rules under FR-S3-068, but is not
  an ingested collection. Check/install performs zero fetching; signed review
  admission never substitutes for current local access/processing grants. Hook-event protocols remain engine-owned,
  not catalog contract resources or dependency edges.
- **FR-S3-056**: Root language areas MUST expose typed quality profiles/instructions and
  small inert CI/starter declarations for Rust, Python, TypeScript, JavaScript,
  Java, Go, configuration-management and infrastructure-provisioning. Profiles
  name format/lint/types/security/secrets/mutation/property/coverage gates,
  applicability, reports, thresholds and manager default/alternatives. One default
  per choice; alternatives cannot weaken mandatory standards. Bind the released
  pinned Rust gate once. Other seven full gates remain separately priced later
  projects (24–40 h each), with unresolved bindings explicit, never passing stubs.
  Team packages require language IDs instead of duplicating profiles.
- **FR-S3-057**: Every install MUST embed all admitted standards with no preset opt-out,
  removal or weakening override. Checks/prohibitions accumulate, ceilings take
  minima, floors maxima and permissions intersect; conflicts refuse. Exceptions
  require exact rule/scope/rationale/expiry/evidence and standard plus central
  owner approval. Non-negotiable rules accept none. Phase 1 imports digest-pinned
  canonical organization and four rust-workflows standards documents read-only,
  drift-checked, preserving rule IDs and normative/local-observation distinction.
  Phase 2/ST1 freezes source edits, makes manifests the one editable authority
  and renders verified-pinned mirrors before independent standards edits.
  Never invent Cedar enforcement for prose or maintain two editable authorities.
- **FR-S3-058**: Trusted CI MUST verify owner/maintainer principal existence and team
  readiness through authenticated read-only lookups, using trusted base code
  with no credential exposed to PR code. Missing/unverifiable access blocks
  release. Descriptor/delegation changes need base-revision owners' approval
  of the exact proposed head; new owners cannot approve themselves. Quorums and
  separation, where required, are protected CI/ruleset checks, not CODEOWNERS
  claims. Offline syntax checks confer no review/runtime authority.
- **FR-S3-059**: Phase 2/A0 MUST provide descriptor-led templates, `maestro package new`
  preview/apply and an ordinary unprivileged scaffolder persona. Create only
  requested supported kinds in the selected owner's area, with explicit owners,
  authored maturity and valid/refusal/eval cases. Use C04; no occupied overwrite,
  invented owner, automatic reviewed status, dependency installation, standard
  exception or runtime grant. Language/standard kinds use root placements;
  standards need central approval. Unsupported future tooling refuses honestly.
- **FR-S3-060**: Phase 2/E1 MUST run bounded registered offline package eval drivers in
  isolated CI with denied egress and synthetic inputs. Zero/missing cases,
  unsupported drivers, wrong outputs and unauthorized egress fail publication.
  Reports/attestations bind exact source and closure; no live provider, LLM judge
  or arbitrary source-provided runner is a default. Check/compile/install never
  run evals; independent package/extension publication requires this checkpoint.
- **FR-S3-061**: Phase 1 starter index MUST list every eligible declared inventory once,
  with input/output contracts, exact closure and parameter validation; stale,
  missing, extra/private entries and unknown selectors refuse. Phase 2 adds an
  optional out-of-process renderer contract and isolated synthetic output
  verification, not a renderer implementation. Rendering is an explicit tool,
  never checker interpolation, install execution or bootstrap copying with
  scripts. Unavailable renderer bindings remain unsupported.
- **FR-S3-062**: Phase 2 consumer setup MUST extend the existing transaction receipt,
  not create another lock/history database: inventory/template ID, source and
  generator pins, schema version, normalized nonsecret inputs, output digests,
  standard snapshot and approved operation. Changed parameters need new preview.
  Binding references never grant endpoints/tools/access. Enrollment is a typed
  portable plan and synthetic example; actual consumer records stay external.
  Package removal preserves consumer data and user-edited outputs.
- **FR-S3-063**: Phase 2 MUST supply inception/construction/operations content within the
  existing feature-delivery graph, stable phase/step IDs, blocking versus
  advisory approvals, pending prerequisites, resumable revision-bound evidence,
  artifact-to-requirement/control/test links and phase knowledge-source plans.
  Operations is readiness/handoff, not deployment. Provide one bounded synthetic
  read-only walkthrough with complete/partial/unavailable labels; missing or
  stale evidence and fabricated source IDs refuse. Standards never become
  optional and a report does not prove the reported action occurred.
- **FR-S3-064**: Phase 1 MUST ship owner/delegation/contribution/security navigation,
  installation/prerequisite/contributor/MCP-authentication/recovery guides,
  release notes bound to actual release identity, one roadmap linked to the
  canonical task ledger and a reader index joining kinds/contracts/guides.
  Check links/generation drift. Guides add no installer or normative authority;
  unavailable or unqualified features remain labelled.
- **FR-S3-065**: Phase 1 guard-evidence output contract MUST distinguish refusal,
  warning and confirmation-required results attributable to rule, tool and
  session, with redacted synthetic cases. Exclude raw secrets/commands and
  distinguish observed warnings from enforced denial. This is an ordinary
  framework output schema, not a new platform hook-event protocol or extra
  hook point. Trusted live attribution/publication/retention belongs to S4.
- **FR-S3-066**: Reserve typed per-host aggregate injected-context/description accounting
  now; implement it in Phase 2. Approved configuration supplies ceilings, never
  numbers copied from reference sources. Count generated descriptions and
  reviewed always-loaded selections, normalize LF/CRLF, deduplicate stable rule
  references and refuse overflow without removing standards. The standard check
  consumes neutral projection evidence, not a dependency on core host resources.
  Static counts are not tokenizer/model-latency qualification.
- **FR-S3-067**: Phase 2 code-analysis declarations MUST define portable graph node/edge
  identities, source revision/digest, extractor identity, relation/confidence/
  provenance, exclusions and freshness using existing skill/contract/source
  kinds. Unknown source subtypes remain unsupported until registered. Full graphs,
  caches and consumer repositories stay outside manifests; only synthetic cases
  ship. Checking never fetches, builds or imports a graph; product identity is
  replaceable data, not a fixed backend or product-named public path.
- **FR-S3-068**: The knowledge-source kind MUST own URL rules beside
  `knowledge/sources/<name>/source.toml`: `maestro-source-policy/1`, referenced
  `maestro-source-decisions/1` and `maestro-source-promotions/1` with expiry,
  and `maestro-url-identity-migration/1`. Use strict JSON under ADR-0014, exact
  inventories/references/digests and the core types/parsers; never duplicate
  per-site rules or invent another approval store in maestro-core. Reviewed
  admission MUST bind the signed digest-pinned catalog release and the owner
  and maintainer approvals that the protected ownership rules record. Expired,
  unreviewed, tampered, inaccessible or revoked rules refuse; proposal-only
  records cannot authorize use. Publish the type-derived JSON Schemas in the
  manifest's `schemas/`, with valid/invalid fixtures and drift checks at all
  four checkpoints. Preserve core wire identities, independent collection ACLs
  and local entitlement. S6 checks this through its separately owned adapter
  behind `PolicySource`/`ResourceSource`; S3 supplies data/contracts, not fetch
  execution. Private inventories live only in admitted private manifest packages,
  never the public repository or public indexes/fixtures.

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
- **Model-card declaration**: reviewed resource metadata plus a kernel v2 identity;
  distinct from an agent-session model profile and from a searchable discovery card.
  The kernel registration/fingerprint, not the current manifest, identifies what ran.
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
- **SC-S3-004**: Core and core-plus-Rust composition tests cover every generated
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
- **SC-S3-009**: M3 evidence names all 85 frozen architecture rows and their Phase 1
  delivered portions, without missing/duplicate/extra keys. C29 accounts for
  all 395 legacy inputs, including explicit Phase 2 recovery holders. Missing
  Phase 1 host/release/S2/CI proof blocks the affected exit; Phase 2/S6/S4 work
  cannot masquerade as M3 evidence or become an unintended M3 prerequisite.
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
  Inject a new setting descriptor and verify automatic appearance, allowed editing
  and the four displayed fields in init and no-argument config, plain and TUI;
  locked/authority-only neighbours refuse with zero unauthorized writes.
- **SC-S3-011**: Startup-update tests cover off (zero discovery calls), propose,
  user-only catalog auto, runtime propose-only, MCP zero activation, daily throttling,
  concurrent sessions, unverified/expired/revoked releases, mandatory approval
  for widened permissions or changed hooks, offline use, interrupted activation,
  receipts and trusted rollback for catalog updates. An active-task spy
  observes zero activation calls; S3 manages no owner-controlled component.
- **SC-S3-012**: Real S3 file-effect tests deny outside writes, secret reads inside
  trust and symlink/reparse escapes with zero effects. User-only canonical
  trust approval, copied-config/model-text refusal, fresh-home exact confirmation,
  root/HOME refusal, untouched preferences, declined-trust config-only writes,
  outside nonsecret reads and kernel isolation pass on all three platforms.
  Every adapter retains the policy floor. Actual Copilot hook allow/deny/error
  receipts are C20/S4, not an M3 gate or a synthetic execution claim.
- **SC-S3-013**: Inject test-only kind descriptors and fixtures without editing
  production checker/compiler/reader/installer logic. Include a glossary fixture
  with a finite float and nested table, and a model-card-like fixture whose
  versioned nested identity and f64 sampling fields reach the named hook whole.
  Non-finite floats refuse. Check → compile twice →
  read → install → resolve preserves its kind, fields, references and digests;
  identical inputs yield identical bundles. Missing required field, unknown key,
  dangling reference and absent/unsupported descriptor each refuse before
  activation. C03 proves source extension; C10/C11/C12/C16 extend the same fixture
  through the existing lifecycle. No production synthetic kind is shipped.
- **SC-S3-014**: Round-trip a synthetic v2 model declaration through the catalog
  and kernel with identical canonical card digest. Re-registering is a no-op;
  changing weights, template, sampling or output limit creates a different card.
  Reject secrets/paths and invalid identities. C03a uses the kernel's unknown-role
  refusal; read and install also refuse that invalid card before registration.
  Explicit installed registration refuses absent local evidence, denied scopes
  and revoked/expired snapshots without writes. Check/compile/install/update
  make zero model-registration/selection calls; changing `model_profile` makes
  zero selection calls. Hold an answer's returned registry card ID in the test,
  register a replacement and remove its catalog: that ID still resolves to the
  same immutable card. The kernel stores no answer history. Register A, then B,
  then A again under one router entry and expect the later ask to use B, without
  a selection receipt. Mark this test as a record of the known S1 post-M1
  explicit-answerer-selection gap, to change deliberately with that fix.
  C16h owns the CLI/history suite. Retain legacy-v1 reads.
- **SC-S3-015**: C30–C35/C79–C81 have positive/refusal neighbours for v4 placement,
  area-derived owners, delegated CODEOWNERS precedence, naming exceptions,
  mandatory standards, layer directions, duplicate namespaces and removable
  packages. A common prompt/common-contract neighbour passes while common →
  core refuses. C42's additive private collection proofs stay S6; public checks
  run without a private checkout.
- **SC-S3-016**: Qualified IDs and all `/2` outputs round-trip; same names in different
  kinds/namespaces pass, duplicate names within one kind/namespace refuse.
  Mandatory common/core/standards are pinned once; all framework personas remain
  available, but knowledge-only native projection stays thin. Old/mixed layouts,
  legacy MCP edges, stale locks, inventory collisions and native alias/user
  shadows refuse. Copilot/Pi probes use pinned real hosts; missing live format
  evidence is blocked, not substituted with synthetic proof.
- **SC-S3-017**: C41–C43 (S6) check strict collection JSON placement and exact
  references to source-owned rules, signed-review admission, URL allow/exclude
  precedence, redirect/access refusal and provenance using synthetic data only.
  Prove zero network calls during check/install; duplicate private/public inputs,
  public-to-private dependencies, missing/unauthorized mounts and aggregate-limit
  overflow refuse without disabling public presets. No crawler, private-content
  seed or corpus migration is claimed by these catalog-contract tests.
- **SC-S3-018**: At M3, all admitted standards are non-removable, eight language profiles
  retain every gate category and honest missing bindings, and base-owner/head-
  bound delegation CI rejects unknown identities/self-approval. ST1 separately
  proves the single-authority render switch after M3.
- **SC-S3-019**: At M3, backend/settings/package tests prove one default producer, every
  class's narrowing/locked refusal, graph bounds/qualified-native and featureless
  behavior, vector identity, MCP namespace/alias checks, immutable core bytes,
  current admission and atomic add/remove/rollback with edited data preserved.
- **SC-S3-020**: Phase 2 X1/E1/L1 tests reject mutable/tampered extensions, missing grants,
  bad secret channels, unavailable drivers, zero evals, self-authorized private
  publishers and public/private leakage. Allow neighbours use exact pins and
  revision-bound CI evidence; install launches zero processes. S4 supplies live
  extension and durable-event proof, not S3.
- **SC-S3-021**: At M3, schema/index/CODEOWNERS generation is deterministic and every
  registered shape has authored valid/refusal fixtures; each stale/extra/missing
  row fails. Phase 2 A0 adds package-new/scaffolder unsupported-kind, occupied-
  output and self-approval refusals; A1 explains exact locked contributors and
  redacts secrets/private identifiers without re-resolving changed files.
- **SC-S3-022**: At M3, starter-index tests enumerate eligible inventories exactly once,
  reject unknown parameters/stale outputs/missing standards, and observe zero
  generator calls during check/install. Guide link/drift checks and redacted
  guard-outcome fixtures pass without a live enforcement claim.
- **SC-S3-023**: Phase 2 synthetic tests bind normalized consumer inputs and exact
  source/generator pins to reproducible output/receipt digests, require new
  preview after change and preserve edited consumer files. Lifecycle/walkthrough
  cases refuse missing prerequisite/approval evidence, stale trace links and
  invented source IDs; partial phases cannot count as qualified results.
- **SC-S3-024**: Phase 2 host-context tests include generated descriptions, LF/CRLF
  equivalence and stable-rule deduplication, refusing overflow without weakened
  standards. Code-graph cases refuse changed revisions, missing freshness/
  provenance, malformed relations and unknown configuration; check performs
  zero graph fetch/build/import. These are static proofs, not runtime ingestion
  or tokenizer/model qualification.
- **SC-S3-025**: At M3, synthetic source-rule fixtures cover all four strict
  JSON families: valid policies/decisions/promotions/migrations, unknown or
  duplicate keys, bad versions, dangling or altered digests, expired decisions,
  fabricated review and public/private leakage. Type-derived published schemas
  and valid/invalid fixtures pass all four checkpoint checks; changing a core
  type without regenerated output fails. Collection-local rule duplication
  refuses in C41/C43's S6 contract tests. Check/compile/install launch no source
  transport. Live catalog-backed resolution is S6 evidence, never a static M3
  claim; private publication uses C69 after M3.

## Out of Scope

- Workflow execution, durable engine, daemon, broker enforcement, sandbox,
  runtime acceptance and general provider/role qualification: S4. Native hooks
  and filesystem safety are not substitutes for those controls.
- An unqualified Pi extension, provider fallback, a separate catalog service, remote MCP,
  GUI, team deployment or multi-user operations.
- Full non-Rust language gates (seven later 24–40 h projects), real product
  connectors, renderer implementation, fleet enrollment, deployed operators,
  interactive training and code-graph ingestion. Eight language declarations
  and bounded synthetic kits are in scope at their stated phases. No bulk
  unchecked legacy import, three-way merge or interface translations beyond
  `en`/`fr`/`es`; conversational tags retain the defined BCP 47 subset.
- Automatic runtime activation/rollback and roots-based MCP workspace detection:
  named follow-ups in 06/08; no unqualified handoff or instructions notification.
- Updates to Cargo dependencies, host clients, models, pinned `gh`, Qdrant setup
  or the router runtime. Owner-managed components have the named notify-only
  follow-up in architecture 06/08, not an S3 automatic installation path.
- File-loaded kind descriptors, plugin execution, new kernel model roles, model
  downloads and automatic winner selection. Declaring model cards does not add
  agent execution or replace M059 profiles. S1's query-expander role follow-up
  owns that currently unsupported role; S2 G17 owns extractor support.
- Cross-machine model-card evidence import: post-M1 local, digest-matched
  `--evidence DIR`, not an implicit download or portable qualification claim.
  Per-answer kernel storage is not provided by this amendment.
- Glossary and source-class table production catalog kinds: after the glossary
  spike shows value, each is reviewed, versioned and per collection. C03's
  synthetic glossary tests are not delivery of that later feature.
- Actual collection crawling/ingestion, corpus migration, provider/storage
  provisioning and admitting any real private URL/content without its required
  manifest review and local authorization. Source-rule representation and
  admission contracts are in scope; C41–C43 are deferred S6
  catalog contracts only, not S3/M3 delivery or permission to read private data.
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
| `owner.catalog` | Approved minimal recovery, framework and language declarations, with a pre-M3 disposition audit | C02, C02a, C21, C21b, C29, C70, C71, C72, C73, C74a, C74b, C75, C76a, C76b, C77a, C77b, C77c, C77d, C82a, C82b |
| `owner.m001.manifest` | Descriptor-extensible catalog definitions and local-evidence model-card registration; InnerSource operation stays S5 | C03, C03a, C02a, C10, C11, C12, C16, C16h, C21, C21b |
| `owner.m001.team` | Role declarations; execution stays S4 | C21b, C22a, C22b, C78 |
| `owner.m001.cli` | Project bootstrap and registry-generated settings editor | C04a, C04, C05, C05g, C05k |
| `owner.m001.load` | Bootstrap/preferences and shared every-setting editor; human approvals remain | C05, C05g, C05k, C17, C18 |
| `owner.m001.laptop` | Detached install/native projection | C06, C07, C16, C28, C91 |
| `owner.m001.guardrails` | Mandatory standards and real Cedar checks; all live host hooks stay S4 | C19, C62, C63, C58, C59, C60, C61 |
| `owner.m024` | Separate catalog and runtime releases | C15, C28 |
| `owner.m028` | Workflow discovery and exact impact | C23, C24a, C24, C25, C26, C27a, C27 |
| `owner.m032` | Restrictive settings; governed enforcement stays S4 | C17, C18, C19, C74a, C74b, C81a, C81b, C82a, C82b, C83a, C83b |
| `chat.M006 layers` | Definitions through verified install; enforcement stays S4 | C03, C10, C16 |
| `chat.M006 layout, M023 layout` | Global/framework/team layout v4, root standards/languages and functional package identities | C02, C21, C21b, C44, C51a, C80a, C30, C31, C32, C33, C34, C37, C38 |
| `chat.M006 compiler` | Descriptor-driven strict definitions and static graph checks | C03, C03a, C10, C11, C22a, C22b, C45a, C52a, C52b, C52c, C58, C60, C62, C63, C64, C65, C66, C79a, C81a |
| `chat.M006 explain` | Every effective setting's provenance | C17, C18, C46, C47a, C49a, C52b, C57, C65 |
| `chat.M006 lockfile` | Exact pins and shared lock-bound lookup; live execution qualification stays S4 | C18, C16h |
| `chat.M006 project file` | Small non-authoritative descriptor | C05, C86, C87 |
| `chat.M006 transparency` | Declared/effective/observed; external export stays S5 | C18, C93 |
| `chat.M006 release` | Trusted catalog lifecycle | C13, C13a, C14, C15, C16, C16b, C51b, C53a, C53b, C54, C58, C69, C92 |
| `chat.M019 maturity, M023 step 4` | Evidence checks; role qualification stays S4 | C03, C21, C21b, C24a |
| `chat.M019 descriptor` | C01 agent sidecars and specification-backed skill metadata | C01, C03 |
| `chat.M019 ownership` | Area owners and delegated maintainers with protected descriptors and trusted identity checks | C02, C21, C21b, C80a, C80b, C90, C32, C34, C35, C38 |
| `chat.M027 detached` | No checkout or toolchain on the laptop | C16, C28 |
| `chat.M027 validation split` | Author/load validation; governed enforcement stays S4 | C03, C11, C13, C13a |
| `chat.M027 invariants` | No verification bypass or fake receipts; broker stays S4 | C14, C16b, C18 |
| `chat.M027 contract crate` | In-workspace schemas, no separate contract crate | C03, C10, C11 |
| `chat.M027 versions` | Runtime/bundle/tool compatibility | C10, C11, C18, C51b, C53a, C53b, C54, C58, C69 |
| `chat.M027 pipelines` | Separate publishers, unprivileged PR checks | C09, C15 |
| `chat.M027 TUF` | Freshness, revocation, floors and offline expiry | C09, C14, C15, C16b |
| `chat.M036 classes, M039 policy` | Restrictive override classes over canonical S1 registry keys, not a second key list | C03, C17, C74a, C74b, C81a, C81b, C82a, C82b, C83a, C83b |
| `chat.M036 defaults` | Authored defaults; runtime application stays S4 | C17, C21, C21b, C45a, C45b, C46, C47a, C47b, C48, C49a, C49b |
| `chat.M006 roles, M023 step 12` | Required v1 roles only; later capabilities stay S5 | C21, C21b |
| `chat.M006 change workflow` | Declarative development workflow; execution stays S4 | C21b, C22a, C22b, C88a, C88b, C88c, C89a, C89b |
| `chat.M019 machine contracts` | Static schema validation; acceptance stays S4 | C19, C21, C21b, C22b, C94 |
| `chat.M039 superpowers` | Explicit lifecycle stages and preserved approval obligations | C21b, C29 |
| `delivery.R08` | Independent review declarations; runtime contexts stay S4 | C21b, C22a |
| `chat.M036 models` | Kernel model-card declarations/local-evidence registration and separate agent profiles; provider qualification stays S4 | C03a, C02a, C16h, C18, C21, C21b |
| `chat.M059 model profile` | Agent-session profile identities and pins, distinct from kernel model cards; live evidence stays S4 | C03a, C02a, C18, C21, C21b |
| `chat.M006 broker` | Real offline policy checks and declared hook mappings; authoritative broker stays S4 | C19, C62, C63 |
| `chat.M006, M023 step 8` | Checked hook action references and host capability states; native execution stays S4 | C62, C63 |
| `chat.M006 destructive` | Real policy fixtures by effect/scope | C19, C21 |
| `chat.M006 approvals` | No source-supplied grants; offline policy and guard-evidence contracts | C19, C62, C63 |
| `chat.M023 step 7` | Real Cedar and fail-closed facts/errors | C19 |
| `chat.M031 principle` | Definitions remain authority, evidence gates eligibility | C24a, C24 |
| `chat.M031 workflow first` | Exact declared closure after workflow selection | C24a, C24 |
| `chat.M031 API` | Typed local CLI/MCP routes and caller context | C24a, C24 |
| `chat.M031 cards` | One replaceable card per resource | C25 |
| `chat.M031 hybrid` | Pre-limit eligibility and measured conditional hybrid | C26 |
| `chat.M031 optimal` | Smallest qualified workflow; no automatic feedback rewrite | C24, C26 |
| `chat.M031 separation` | Catalog/knowledge collection and scope separation | C25, C96a, C96b |
| `chat.M031 offline` | Valid exact-ID/local lexical fallback | C14, C24 |
| `chat.M031 publication` | Verified generation/snapshot identity | C25 |
| `chat.M031 security` | Local caller context/cache isolation; HTTP remains out of scope | C24a, C24, C26 |
| `chat.M031 delivery` | Frozen baseline before hybrid | C23, C24, C26 |
| `chat.M031 service` | Existing server, no separate catalog service | C24a, C24 |
| `chat.M019 bootstrap, M023 step 13` | Inspect/preview/apply/validate/ownership | C04, C05, C45b, C47b, C49b, C50, C55, C77a, C77b, C77c, C77d, C81b, C84, C85a, C85b |
| `chat.M019 overlays` | Core plus Rust composed output | C02, C05, C79a, C79b, C82b |
| `chat.M006 native` | Convenience projection, checked host maps and owned removal; live hooks stay S4 | C06, C07, C62, C63 |
| `product.GD2, GD4, GD5` | Four-client local MCP and checked hook mappings; all live hooks stay S4 | C00, C06, C07, C08, C28, C62, C63, C75 |
| `chat.M048 real controls` | Real verification/Cedar and allow/deny neighbours | C09, C13, C13a, C19, C28 |
| `chat.M057 CI` | Zero relevant tests cannot pass; honest statuses | C15, C28, C51a, C52a, C52b, C67, C68, C69, C80b, C83a, C83b |
| `chat.M059 provenance` | Declared model cards, registrations and returned answer card IDs kept distinct; no stored-answer history | C03a, C08, C16h, C18, C28, C46, C47a, C49a, C52b, C57, C65 |
| `delivery.U02` | Catalog consistency and provenance; bounded translation recovery after M3 | C03, C21, C21b, C29 |
| `delivery.U05` | Consumption, compiler, closure and overrides | C10, C11, C17, C22a, C22b |
| `delivery.U09` | Deterministic bootstrap and projection | C04a, C04, C05, C06, C07 |
| `delivery.U13` | Catalog routing; S1 retains retrieval/model qualification | C23, C24a, C24, C25, C26 |
| `delivery.U17` | Bundle trust/lifecycle; general InnerSource stays S5 | C09, C15, C16, C16b, C28 |
| `core storage` | Scoped kernel authority including immutable model cards, rebuildable projections | C03a, C12, C16, C16h, C25, C27a |
| `core verification` | Organization gates, no S3 exception | C28 |
| `delivery.§1.5` | Exact S3 delivery evidence; other slices retain their portions | C00, C28, C44 |
| `chat.M006 CLI` | Bootstrap, projection, doctor, catalog lifecycle, explicit local-evidence model-card registration, registry-generated settings editor and policy CLI; other commands stay S4/S5 | C05, C05g, C05k, C06, C07, C13, C13a, C16, C16h, C16b, C17, C18, C19, C51a, C51b, C52c, C54, C55, C56, C57, C59, C78 |
| `chat.M036 objects` | Descriptor-extensible resource kinds including model cards; runtime objects stay S4 | C03, C03a, C02a, C10, C11, C12, C16, C16h, C21, C21b, C45a, C52a, C52b, C52c, C58, C60, C62, C63, C64, C65, C66, C79a, C81a |
| `delivery.§2.2` | Separate signed catalog, manifest-owned source-rule JSON and completion contracts; live runtime envelopes stay S4 and acquisition stays S6 | C10, C11, C28, C41, C43, C44, C52a, C52b, C66, C68, C69, C80b |
| `chat.M019 roles` | Ten core personas now, scaffolder and Translator in named Phase 2 tasks | C29, C70, C71, C72, C73, C74a, C74b, C75, C76a, C76b, C77a, C77b, C77c, C77d, C82a, C82b |
| `chat.M023 step 13` | Stop-on-collision owned writes; three-way merge remains separately specified | C04, C05 |
| `delivery.C01 transparent platform, minimal plumbing` | Bootstrap and registry-generated preferences remove plumbing, not human judgement | C05, C05g, C05k, C17, C18 |
| `delivery.C02 detached two-repository releases` | Separate catalog/runtime publishers and detached install | C09, C15, C16, C28 |
| `delivery.C04 readiness, owners, dependencies without invalid frontmatter` | Strict descriptors, owners and readiness evidence; role qualification stays S4 | C01, C02, C03, C21, C21b |
| `delivery.C06 closure, merge, explanation, one override class` | Exact closure, restrictive resolution and one explained override class | C03, C17, C18, C22a, C22b, C45b, C47b, C49b, C50, C55, C77a, C77b, C77c, C77d, C81b |
| `delivery.C08 explicit instructions and skills with provenance` | Required skills/instructions and native projection provenance; session composition stays S4 | C02, C06, C07, C21, C21b, C95a, C95b |
| `delivery.C12 preview/apply bootstrap` | Safe deterministic preview/apply bootstrap with the shared settings editor | C04a, C04, C05, C05g, C05k |
| `delivery.C13 native projection` | Copilot/Pi convenience projection; live policy hooks stay S4 | C06, C07, C62, C63 |
| `delivery.C14–C16 catalog MCP, shared Qdrant, separate knowledge ACLs` | Local catalog MCP and scoped cards through the shared knowledge pipeline | C24a, C24, C25, C26 |
| `delivery.C17 signed releases, freshness, revocation, transparency` | Verified catalog lifecycle, current trust and honest transparency | C09, C13, C13a, C14, C15, C16, C16b, C18 |
| `delivery.R01–R11 corrections` | R05 workflow-first routing and R09 trust; other corrections stay S4/S5/S7 | C14, C16b, C24a, C24 |
| `product.GD1–GD5` | Four-client local MCP and checked hook maps; live hooks stay S4 and GD1 retains its existing ownership | C00, C06, C07, C08, C28, C62, C63 |

Architecture [08 §21](../../docs/architecture/08-traceability.md#21-s3-contract-inventory-c00-not-delivery-evidence)
records D1/D5, concurrent S1/S2/S3 starts, the S3/S4 compiler boundary, the
`product.GD2, GD4, GD5` hook disposition and C27a's S2 G27 seam, as amended by v4 in 03/06. This is contract evidence only; the inventory is approved by the owner,
2026-09-28 (C00 inventory approval). C09 owns closure of 08 §17's "Publisher
identities, trust roots, key rotation procedure" row with D2 and OA4 evidence;
without that evidence
C28 remains blocked.
Runtime enforcement, general qualification and InnerSource keep their named
later slice. No similarity or unapproved closure fallback replaces C27a/C27.

### Exact requirement coverage after v4

The [task Requirements inversion](tasks.md#requirements-coverage) and the
`requirements` section of traceability.json contain every FR/SC and its exact
holders, including existing IDs. The JSON `design_coverage` maps all sixteen
approved MD design keys; `gap_obligations` maps G01–G13 to disjoint bounded tasks.
These supplement, never expand, the fixed 85 architecture rows and six exclusions.
There are **93 requirements: 68 FR and 25 SC**. C44's documentation mapping is
not implementation evidence; phase-specific outcomes remain at their milestones.

Phase 1 includes G01 and G07–G11 (16 h); G12 is Phase 2 (6 h), as corrected by
the supervisor. The task ledger records all 57 design tasks exactly once and all
13 gap obligations through 19 bounded tasks. C29 now gates M3; C20 is a 4 h S4
transfer. C41–C43/S6 and the 35 Phase 2 tasks never gate M3. No mapping marks
unimplemented content, live hooks or generic evals as delivered.

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
