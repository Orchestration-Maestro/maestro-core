# Implementation Plan: Catalog

**Branch**: `docs/s3-workspace-config` | **Date**: 2026-09-28 | **Spec**: [spec.md](spec.md)

**Input**: the approved S3 draft, owner decisions of 2026-09-28 01:56, 08:12,
11:25, 11:50, 11:55, 12:00 and 12:05,
and the supervisor's specification-review ruling,
architecture [03](../../docs/architecture/03-agent-orchestration.md),
[06](../../docs/architecture/06-roadmap.md) and
[08](../../docs/architecture/08-traceability.md). Tasks: [tasks.md](tasks.md).

## Summary

Build the smallest useful catalog first: one knowledge preset, the Maestro
profile and the dependencies of `ctm-question`, then the Rust overlay. Explicit
selection and the existing knowledge tools provide the first owner loop;
there is no engine or intent router on that path. Label reviewed-source
bootstrap and projection as authoring convenience. Init also collects and
persists conversational language/tone and documented workspace overrides in a
polished, accessible menu, alongside an explicit workspace-trust decision;
scripted init uses the same plan/validator and cannot manufacture trust approval.

Then complete M3: deterministic bundles, verified installation, current trust
records, startup update proposals/auto policy, settings, policies, graph
validation and measured routing. Reuse the
kernel, knowledge pipeline and existing CLI/MCP server. A catalog declares
workflows; S4 will execute them. Do not open the earlier catalog before M3.

## Technical Context

**Language/Version**: Rust 1.98.1, edition 2024, workspace MSRV 1.98.

**Primary Dependencies**: reuse `serde`, `serde_json`, `serde_yaml_ng`, `toml`,
`sha2`, `clap`, `schemars`, `rusqlite`, `rmcp`, `reqwest`, `tokio` and the S1
crates already in the workspace. C04a moves the existing ADR-0018 filesystem
module, its `rustix`/Windows implementation and tests to `maestro-filesystem`;
no second safety layer is written. D4 approves `tar`, JSON Schema validation
and `cedar-policy` 4.13 (ADR-0007). C09 selects/measures the JSON Schema 2020-12
crate and minimum features under ADR-0020; `schemars` generates schemas but
is not a runtime schema validator. OA9 approved `ratatui` plus `crossterm` on
2026-09-28 under ADR-0020; C05f still measures minimum features, dependencies,
licences and vet needs before adoption. No localization framework or new service
is presumed.
Pinned `gh` performs attestation verification. No custom signature code, new
Pi extension, separate service or speculative interface crate.

**Storage**: kernel SQLite and content-addressed artifacts remain authority.
The catalog collection in Qdrant and the S2 dependency projection are
rebuildable. Project descriptors, locks and ownership records are local files,
not a second authoritative catalog database. Workspace `.maestro/config.toml`
and user `preferences.toml` hold preferences, never the kernel's grants.
Trust decisions, canonical roots and approval receipts live only in user-local
kernel authority records; workspace config contains no machine paths or receipts.

**Testing**: source-named Rust tests beside implementation, process tests under
`crates/maestro/tests/it/`, public synthetic fixtures, real Cedar and schema
validators, hostile archives, filesystem fault injection and isolated host
homes. Live receipts identify exact client/provider/account and remain private
when they include private data. Mocks do not replace the control under test.

**Target Platform**: all crates build and pass tests/Clippy on Linux, macOS and
Windows. Initial live Copilot/Pi proof is on the reference WSL machine;
portability is not live host or sandbox qualification.

**Project Type**: `maestro-catalog`, consumed by the existing `maestro` binary,
and the approved pure extraction `maestro-filesystem`, shared with existing
canonicalization callers. Generic content is in `maestro-manifests`, created by
the owner with its first real content, not by this documentation task.

**Performance Goals**: trust refresh interval at most five minutes while in
use; offline expiry at most 24 hours. Bound parsing, downloads, subprocess time
and output before allocating or invoking a verifier. Report routing latency,
not an invented latency target. Preserve S1's free-room-only model loading.

**Constraints**: no private vendor content, personal paths or secrets in public
files/logs; every new library measured and approved; no executable source
content during check/compile/init; no silent loss of user files or permissions;
no inference of identity, approvals or qualification from model text.

**Scale/Scope**: two v1 workflows, their required roles/resources, base and Rust
composition, two native projections, four existing MCP registration guides,
100+ independently reviewed intents. Rust is the first programming overlay;
the well-formed BCP 47 subset in D6 selects conversational language. Built-in interface
strings start with `en`, `fr`, `es`; other languages use English interface text
with a visible note, without changing conversation language. No extra
programming languages or roles without a real workflow requirement.

## Starting point (checked 2026-09-28)

The fresh planning clone starts at
`dac543ce29aee667234821c63e3947c0d5441b34` on `feat/s3-integration`.

| Seam | Observed state and reuse |
| --- | --- |
| CLI | `crates/maestro/src/cli/args.rs` and `run.rs` have no init/catalog/config/policy commands; add thin adapters there, not a second binary. `output.rs` is the human/machine output boundary |
| MCP | `crates/maestro/src/mcp/server/handler.rs` advertises collections, get, search and ask; reuse bounded transport and server operations. The draft's older `server.rs` path no longer exists |
| Host proof | `docs/how-to/knowledge-mcp.md` and `crates/maestro/tests/it/mcp_clients.rs` exist; client-like stdio tests do not prove real Pi/Copilot loading or T038 completion |
| Retrieval | `crates/maestro-knowledge/src/search/filter.rs` filters scope/version only; C26 must add eligible-ID filtering before every branch's limit |
| Evaluation | `crates/maestro-knowledge/src/eval/` scores sections, not workflow IDs; reuse statistical methods, not section labels masquerading as intent labels |
| Preferences | `maestro-kernel/src/scope/config.rs` strictly parses user `config.toml` with only `[access]`; do not turn workspace discovery into this grant loader. Reuse `maestro_kernel::paths::config_dir` for separate user `preferences.toml` |
| Answers | `maestro-knowledge/src/answer/{prompt.rs,generate.rs,types.rs}` requests/detects the question's language, emits en/fr refusals and the public `lang` field. C05d adds explicit session language, retaining question-language fallback and evaluation isolation |
| Kernel | Scoped records, artifacts, journal, jobs and backup exist. Migrations end at `0011_exact_identifiers.sql`; catalog numbering must exceed every landed/reserved number on main and S1/S2/S3 integration branches |
| Filesystem | `crates/maestro-canonicalization/src/filesystem/{mod.rs,root.rs,unix.rs,windows.rs}` owns ADR-0018 held handles and no-follow opens; C04a moves it, not copies it |
| Hosts and content | The draft recorded Pi 0.87.1, MCP adapter 2.37.0 and subagents 0.64.0; Copilot was absent from PATH and `maestro-manifests` did not exist. These are historical observations, not refreshed approvals or qualification |

The 08:12 owner decision starts S2/S3 while S1 finishes. C00 requires only
D1–D5 decided. Integrated T034/T035 and T038 live evidence gate C08 and C28;
the M1 release gates M3 exit, not fixture-based implementation. C01 records
actual pins/capabilities rather than assuming historical versions are approved.

## Constitution Check

The shared Spec Kit constitution delegates to the golden rules and this
repository's [rule map](../../docs/standards/engineering.md). No rule is waived.

| Rule | How S3 holds it |
| --- | --- |
| FND-002, FND-003, P-001 | First content and owner-loop checkpoints; add a file only for a requirement; no engine or new framework |
| P-004, P-005 | Reuse real S1 seams; a new trait needs a real implementation variation, not a future possibility |
| P-011, P-013, SEC-003 | Strict bounded sources and bundles; reject duplicates, unknown fields, unsafe paths and incompatible requirements |
| P-012, P-014 | Scoped reads and one admission path; a lock, metadata label or search result grants nothing |
| ENF-001, SEC-001 | Bindings/environment rather than personal paths; synthetic public tests and private production receipts |
| ENF-002 | Tests and organization-configured Clippy on three targets; live qualification described separately |
| ENF-005, ENF-006 | Observe red before green; never mock Cedar/verification under test or weaken a gate to fit a task |
| ENF-008, ENF-012 | Pinned inputs, commit hooks and targeted lane checks; full CI coverage/mutation evidence before main merge |
| ENF-011, SEC-002 | Instructions are data; no model-supplied actor or approval; compilation never executes scripts |
| SEC-008 | Declared/effective/observed views; blocked, unsupported, not-run and passed remain distinct |
| ADR-0018, ADR-0020 | Handle-based filesystem safety, measured library adoption, named duplicate exceptions and vet evidence |

Rechecked at C00 (2026-09-28 review amend): no change to the constitution table.
Recheck this table after C09's measurements and at C28. A dependency
exception names the exact crate/version and removal condition in
`maestro-quality.toml`; it is not permission to relax the whole gate.

## Project Structure

### Documentation (this feature)

```text
specs/003-catalog/
├── spec.md                    # requirements and observable outcomes
├── plan.md                    # design, contracts, decisions and owner actions
├── tasks.md                   # bounded, test-first implementation tasks
└── traceability.json          # C00 inventory approved by the owner, 2026-09-28
```

C01/C09 later add measured research under `specs/003-catalog/research/`.
C00/C28 maintain architecture 03/06/08 and the exact delivery map.
`catalog_traceability` derives source candidates from architecture 08's catalog
links and S3 statuses, then checks the 85 included keys, six reasoned exclusions,
this spec's portions and the task headings, including negative mutations.
C00 fixture consistency is not delivery evidence or owner approval.
The frozen inventory is approved by the owner, 2026-09-28 (OA7); M3 acceptance
remains pending.

### Source code (planned)

```text
crates/maestro-catalog/
├── Cargo.toml
└── src/
    ├── source/                # bounded source types, references and checks
    ├── files/                 # owned write plans, apply, remove and recovery
    ├── bootstrap/             # inspect, compose, descriptor and lock
    ├── hosts/                 # Copilot and Pi projections
    ├── bundle/                # deterministic writer and hostile-input reader
    ├── install/               # shared verified update/rollback and receipts
    ├── trust/                 # verifier and shared freshness admission
    ├── settings/              # strict preferences, discovery, classes and resolution
    ├── resolve/               # exact definitions, locks and explanations
    ├── policy/                # Cedar and workspace-trust port/default adapter
    ├── graph/                 # topology and contract checks, not execution
    ├── route/                 # exact-ID, lexical and measured hybrid routes
    ├── eval/                  # intent labels and paired comparison
    ├── discovery/             # cards through S1's publication pipeline
    └── impact/                # exact dependencies through qualified S2
crates/maestro-filesystem/           # existing ADR-0018 code moved unchanged
crates/maestro-kernel/src/catalog/    # scoped install/trust/component records
crates/maestro/src/cli/              # init menu/plain flow, config and policy adapters
crates/maestro/src/presentation/     # en/fr/es interface text and agent instructions
crates/maestro/src/mcp/              # catalog tools and session preferences
crates/maestro-knowledge/src/answer/ # conversational prompt controls, not translation of evidence
crates/maestro/tests/it/             # process-level command/host tests
```

Modules arrive with their first tested behaviour, never as empty directories.
`mod.rs` files with children contain only module/use declarations; tests live
beside their subject. [Tasks](tasks.md) name leaf files and registration seams.

In `maestro-manifests`, use the architecture 03 §1.1 layout: `agents/base/`,
`skills/`, `instructions/`, `workflows/`, `contracts/`, `policies/`,
`profiles/models/`, `mcp/`, `hooks/`, `settings/`, `presets/`, `bootstrap/`,
`evals/` and `CODEOWNERS`. Seed only required directories. No earlier catalog is
read, copied or imported to populate them.

**Structure decision**: kernel records stay in the kernel; catalog processing
has no CLI/MCP types; adapters call it. Knowledge search receives only the
eligibility filter it needs. Knowledge tests use knowledge types only, with no
reverse dev-dependency on `maestro-catalog`; card assertions belong in the
catalog crate. There is no `maestro-contracts` crate or separate catalog MCP
service (architecture 08 A23/A24).

## Design

### D1 Checkpoints without a trust bypass

Approved: useful checkpoints first. C02 provides reviewed generic content;
C08 proves the owner loop. They are authoring convenience, not a signed
installation, governed run or M3 exit. `--catalog-dir` reads explicitly selected
reviewed data, validates it and records source digests in an authoring lock.
It is not accepted by install/update, cannot mint a verified install record and
cannot make a source tree appear attested. Native projection from this mode is
labelled accordingly. Workflow declarations and executable assets remain inert.

The normal M3 path consumes only verified installed bundles and passes trust
admission before init, projection or catalog consultation. Explicit resource
selection works before routing; no code depends on an engine merely to install
or remove an agent profile.

C00 records the contract in architecture 03/06/08: graph checks needed to safely
compile S3 bundles belong to C22a/C22b; S4 still owns general runtime execution.
The frozen exact row-key inventory and S3/remaining portions, including
GD4's Copilot-only S3 hook and S4 host-adapter qualification for the other
clients, are approved by the owner, 2026-09-28 (OA7). Do not quietly implement
a smaller graph language. A supported construct passes all applicable checks;
unsupported means not executable.

### D2 Trust, publication and installation

Approved: pinned `gh`, separate catalog/runtime publishers, five-minute refresh
and at most 24-hour offline validity. C09 first demonstrates actual verification
of a real public attested artifact and rejection of a wrong signer. Existing
public evidence suffices to develop the verifier: no catalog publisher setup
or release is required to start C09/C13. Freeze the verifier version/digest,
expected issuer/repository/workflow bindings and authenticated record formats.
C09 owns closure of architecture 08 §17's "Publisher identities, trust roots,
key rotation procedure" row: record D2 and the exact OA4 bindings/evidence when
supplied; pending live binding/rotation evidence is an explicit C28 gate, not
an implicit blocker for dependency measurements. No handwritten crypto or
trust-on-first-use.

Use GitHub attestations to authenticate the exact bytes of both bundles and
trust records. Timestamp records bind issued/expiry times to the current
revocation/floor record digests and monotonically ordered updates. Revocations
identify whole bundles and individual entries; version floors never decrease.
Validate a complete new record set before one kernel transaction makes it
current. Release and trust-record publication are independent: an emergency
withdrawal or freshness refresh must not require a new content bundle.

The catalog publisher runs the protected trust workflow **every six hours**,
re-issuing timestamp records and attestations even when content is unchanged.
An independent hourly trust-watch workflow alerts the named maintainer and
backup once the latest verified issue time is over eight hours old; exercise
that alert in the OA5 drill. Only protected publishing jobs have `id-token: write`
and `attestations: write`, plus the minimum publication permission. A delayed
scheduler or publisher outage never extends signed expiry. OA5 enables the
schedule and alert delivery; C15 supplies tested workflow code without waiting
for those external operations.

Online refresh requires `gh auth login` or `GH_TOKEN`. A local unauthenticated
probe of `gh` 2.98.0 exited 4 before verification, requesting authentication;
this observation is not the final pin qualification. C09 repeats it for the
selected pin and measures API calls including record download, attestation
lookup and identity verification. Five-minute polling means 12 refreshes/hour
per active trust root: report `12 × calls per refresh × active laptops` against
the actual account's rate limit, including concurrent roots and backoff on
throttling. Cache validated records, not authorization past expiry; never request
a new token or store one in content as an implicit side effect. OA4 supplies the
checksum-verified standalone `gh` pin and approved credential binding. C13 adds
`maestro doctor` checks for executable version/digest and auth readiness; missing
or mismatched prerequisites produce a diagnostic and cannot trigger downloads.

Every install/update/init/resolve/search/route/explain/impact/project consult
enters the same admission function, binding caller scopes, compatible runtime, exact
snapshot and current trust. A fresh online check is attempted when the cached
records reach the five-minute refresh interval; an unavailable network permits
only the authenticated offline window. Expiry is enforced at the boundary,
not on a best-effort timer. Refuse clock rollback below durable observations,
record replay and version-floor regression. Recheck the trust revision before
returning a consult whose admission raced with a local revocation update.
There is no promise to learn remote revocation before the bounded refresh.

The verifier receives an executable resolved from an approved pin and fixed
argv, never a shell command from content. Bound downloads, verifier stdout,
stderr and wall time; timeout kills and reaps it. Check the attestation's subject
against the locally hashed artifact, expected source commit, repository,
workflow and issuer, not merely `gh`'s exit code or a supplied checksum file.

Compile sorted tar entries with fixed archive metadata and canonical bundle
JSON; no compression is needed. The reader rejects extra/duplicate entries,
links, traversal, device entries, oversized bodies and truncation. Verify every
entry and compatibility before activation. Artifacts may be staged, but no
partial install or discovery generation becomes current. Switch install
records, component references and pins transactionally; retain the old valid
install after failure. The project lock changes only on an explicit update.

After restore, catalog trust is unready until refreshed against the configured
publisher. An older backup cannot lower the effective floor or resurrect a
revocation by presenting its own signed-but-stale records. Revocation stops new
loads; it cannot retract text already in a native session. The guide names that
limit and the restart/removal action.

### D3 Synthetic data first

Approved: all committed fixtures and public CI are synthetic or public. Generic
`ctm-question` instructions describe how to use knowledge tools; they contain no
vendor passages, question set, production counts or private configuration.

Reuse T038's exact client/provider/account/data-scope approvals for any private
proof; do not treat a logged-in client or local transport as a grant. Public
receipts cover synthetic data only; private receipts keep generation/span/digest
checks and all attempts. Denial cases use a separate no-grant kernel because
opening the normal configured kernel reconciles the local principal's grants.
No uncalibrated answer is silently presented as calibrated.

### D4 Native formats, host pins and libraries

Approved: "installed parsers plus approval for new libraries". Reuse the
installed parsers/adapters and measure only missing `tar`, JSON Schema 2020-12
and `cedar-policy` 4.13 support. C03 does not wait for C01: use its verified
frontmatter result if already available; otherwise ADR-0005's
`<name>.maestro.toml` sidecar fallback is final for v1 when C03 starts. A later
switch to `metadata:` is a separate reviewed task, never an automatic format
change. C01 records real host version/digest, effective tools, discovery order,
reload behaviour and supported fields. Missing binaries/accounts block the
probe and live exits, not fixture-based source/bootstrap/routing code. A changed
host pin needs the probe again.

Copilot output uses native agent/skill/instruction formats. Pi maps only
supported fields, explicit MCP tools and their installed provider; child tool
names without the adapter are not sufficient. Do not install a new extension
or rely on model inheritance/fallback. Unsupported fields are diagnosed rather
than dropped. Only evidence-supported host resources are projected; an
S4-unqualified executable workflow remains ineligible. S3 administers only
Copilot `preToolUse`; Pi, Codex and Claude Code hooks are deferred to S4
host-adapter qualification because their trusted event/identity adapters are
not qualified here. Four-client MCP registration remains S3.

C09 records crates added, duplicate versions, native links, feature choices,
licences and vet requirements against the current lock. D4 already approves
these libraries: the supervisor verifies measured features, DEP-001 exceptions
and selection of the JSON Schema validator under ADR-0020, without re-asking
for that approval. Any unlisted licence still needs the authorized maintainer's
organization allowlist action, recorded under OA4. No worker changes that
setting. A hand-written schema or Cedar substitute is not the minimal solution.

### D5 Baseline routing and measured hybrid

Approved: baseline first, hybrid only when it demonstrates gain. C23 freezes
100+ independently reviewed intents and the accepted alternative workflows,
clarification/no-match labels and adversarial cases. Freeze a synthetic
eligibility snapshot in `evals/intents/eligibility.json`, with its digest beside
the suite/profile digests and a nonempty matchable denominator. It enables
measurement without inventing S4 qualification. Keep tuning and held-out cases
separate; never tune on a failed held-out comparison.
Two production workflows are too few to make top-3 informative, so include
synthetic distractor workflows, report that cohort separately, and retain top-1
and no-match results even when top-3 passes.

Eligibility precedes scoring and candidate limits in exact-ID, lexical, dense
and BM25 paths. Bind scope, maturity, host/runtime support, provider/model
qualification, classification and trust once per consult, then verify its
revision has not been invalidated. A workflow is eligible only if its exact
required closure is eligible; a missing mandatory reviewer excludes it.

`catalog_route` returns at most the configured three candidates, selection
reasons, roles, skills, deterministic steps and checks, exact bundle snapshot
and the index generation used. An exact-ID/local lexical result states that no
index was used rather than inventing a generation. Statuses are `candidates`,
`no_match`, `needs_clarification`, `incompatible` and `temporarily_unavailable`.
`catalog_resolve` returns exact definitions; `catalog_search` browses kinds.
The authenticated caller context is host-bound, not a raw filter from the model.

Cards use a separate scoped catalog collection and verified generation per
bundle. A stale/missing index triggers explicit lexical fallback only when the
bundle is still authorized and valid. Every cache includes visibility,
snapshot, trust/policy revision, runtime constraints and retrieval profile.
Use S1's statistical methods for paired results; adapt scoring to workflow IDs,
not document sections. Hybrid's paired improvement must exclude zero before it
is enabled. Keep all failed comparisons and missing-route markers.

Architecture 03/06/08 now records D5's conditional rule in place of the old
unconditional hybrid-win exit. Top-3 ≥ 90 % remains the
M3 threshold on that synthetic snapshot; no-match, clarification, top-1 and
unnecessary context are reported separately. A real M3 install returns
`incompatible` ("not qualified until S4") for executable workflows. C28 and
`docs/how-to/catalog.md` show both outcomes; synthetic receipts never authorize
a live role. C24a resolve/search and C24 routing depend on fixture-based trust,
locks and graph contracts, not C15/C16 or publisher/release setup.

### D6 Owned writes and project bootstrap

C04a first moves `maestro-canonicalization/src/filesystem` and its existing
tests into the minimal `maestro-filesystem` crate, preserving behavior and
security assertions. Change only module imports, visibility and workspace
wiring; add no external library or second platform implementation. Existing
canonicalization callers and C04 use this one ADR-0018 home.

One writer serves init and both hosts. C04 owns immutable relative-path/digest
plans, removal and recovery on those held-handle operations. Resolve the caller
root once, refuse links/reparse points beneath it, use exclusive creation and
compare preconditions without reopening by unchecked path.

C05j later gates every controlled external file effect through D11's
`WorkspaceTrust` port before C04 applies it; the file safety primitive remains
independent of policy. C06/C07/C16 and update/rollback adapters reuse that gate,
not ad hoc caller checks. Kernel-internal storage keeps its own authority.

Write recovery state before effects and ownership completion last. A rerun
reconciles observed digests; it never overwrites a changed file to finish a
previous attempt. Removal checks ownership and current digest. C06, the first
shared MCP JSON consumer, separately adds entry ownership: preserve unrelated
entries, refuse a changed owned entry, and apply the resulting file through
C04's unchanged-file preconditions. Stop on collisions/shadowing. There is no
three-way merge in S3.

Init inspects files without invoking build systems or scripts, resolves the
selected preset and base/Rust overlays, previews dotfiles too, then applies only
the authorized plan. Validate composed JSON and descriptor references before
publication. Report missing prerequisites, including shell availability on
Windows. `.maestro/project.toml` contains preset, lock, capabilities and context
files only. It cannot redefine policy, hooks or authentication.

#### Workspace preference schema and ownership

C05a adds `.maestro/config.toml` at the explicit init workspace root (the current
working directory for init), separate from that descriptor and the lock. Never
write an ancestor's config when initializing a nested workspace. Preview the
config with all other files and use C04's digest-bound owned writes and D11's
trust decision: no config is persisted on preview/cancel, collision or invalid
input. The sole declined-trust exception is D11's separately confirmed
config/approval-metadata write. Rerunning identical
init is a no-op; an edited config is preserved with a conflict, not overwritten.
Across en/fr/es/ja and all tones, generated workspace/host deliverables must be
byte-identical except the config's language/tone values. Kernel/internal ownership
metadata instead must record the exact corresponding config digest; test that
structurally and document the distinction in the invariance test's doc comment.
No other deliverable exception is allowed; receipts/logs normalize only timestamps
and IDs.

```toml
schema = "maestro-preferences/1"
language = "en"
tone = "normal"
updates = "propose"

[overrides]
model_profile = "balanced"
routing_candidates = 3
```

The schema marker is required. `language`, `tone`, `updates` and `[overrides]`
may be omitted when editing; an omitted value falls to the next precedence
layer. Init writes the selected language, tone and update policy explicitly. Unknown/duplicate keys
at any depth, wrong TOML types, unsupported schema versions and out-of-range
values are errors, never ignored. Bound file size/depth before parsing, as for
C03; no includes, interpolation, expressions or environment/SDK passthrough.
This follows ADR-0014's strictness, retaining TOML for human settings rather
than changing collection/source-policy JSON.

- `language`: a well-formed BCP 47 subset: language (2–3 ASCII letters), optional
  script (4 ASCII letters), then optional region (2 ASCII letters or 3 digits).
  Canonical casing is lower-case language, title-case script and upper-case
  alphabetic region. Accept `en`, `ja`, `zh-Hant-TW`, `es-419` and `sr-Latn`;
  refuse variants, extensions, private-use, grandfathered forms and any other
  subtag with a named unsupported-tag error. This complete bounded subset needs
  no registry lookup or parser dependency; it is not full BCP 47 support.
  Only the canonical tag reaches model instructions, quoted as data, never the
  original input. UI lookup uses the language subtag for en/fr/es; other tags
  get English interface text with one visible note per session.
- `tone`: exactly `brief`, `normal` or `detailed`; the menu's English display
  label for the last is "Very detailed". UI/init defaults are `en` and `normal`.
  Keep an absent language distinct from that UI default: an answer without an
  explicit language in any layer keeps S1's question-language behavior.
- `updates`: `off`, `propose` (default) or `auto`. `auto` is accepted only in user
  `preferences.toml`; workspace files, flags and init drafts can request only
  narrowing to `off` or `propose`, never grant automatic application. Resolve the user/default ceiling
  first, then take the strictest requested value under `off < propose < auto`.
  No flag can override a workspace `off`, nor can workspace `propose` raise user
  `off`. Workspace/flag `auto` is diagnosed as an ignored widening, not consent;
  malformed enum values still refuse. Budget-like bounded keys likewise take
  the strictest bound across all layers before free-value precedence; explain
  each ignored widening. D9's approval and idle rules still constrain catalogs;
  runtime releases are propose-only and MCP never activates either target.
- `[overrides]`: only parameter keys documented with type, range and class in
  C17's checked setting registry. The initial exposed keys are `model_profile`
  (`fast`, `balanced`, `deep`, bounded by role/provider qualification) and
  `routing_candidates` (integer 1–3, bounded by the existing ceiling). A stored
  profile is a preference, never qualification or permission to execute it.
  Reject unknown keys and locked changes; no generic raw option map escapes
  C17. Extra parameters require a documented class/contract and tests first.
- Refuse `[access]`, identity, hooks, authentication, secrets and policy changes
  in either preference file. Existing user `config.toml` remains the sole
  authority-config input for local grants; never hand a workspace file to
  `scope::Config::load` or reconcile grants from it.

The user-level file is `maestro_kernel::paths::config_dir(...)/preferences.toml`.
Both files contain preferences only; reject `[trust]`, paths and receipt fields.
The existing platform/XDG configuration-home resolution stays unchanged. Init
does not modify that user file or the authority `config.toml`. User-local trust
uses D11's existing kernel journal, not a second database or common crate.

#### Every-session discovery and resolution

C05b canonicalizes the CLI working directory once. Inside home, discovery walks
upward no farther than the user's home directory inclusive. Outside home, read
no workspace file until a containing folder is explicitly trusted in the local
kernel journal; then walk only within that approved root, never above it. With
no such approval, use user preferences and show one local line naming
`maestro trust add <exact path>`. Never select drive or mount roots, including
`/mnt/c`, even when their metadata looks user-owned. Do not stop at `.git` or a
`.maestro/` directory without a config; nested workspaces remain supported.

On ADR-0018 held handles, require both `.maestro/` and its file to belong to the
current user and not be group- or world-writable. Unix checks uid/mode; Windows
checks the owner SID and DACL for other-principal write access. Never follow
config symlinks/reparse points or accept an ancestor swap. Foreign-owned,
unsafe, unreadable or unverifiable candidates are skipped with a warning, never
parsed and never allowed to block startup. A selected safe user-owned file
with invalid contents still refuses; it never falls through to a parent.
Reads remain bounded. Tests use planted valid/invalid files and user-owned
neighbours on all three platforms, including `/tmp` and `/mnt/c`-style parents.

Read only the nearest workspace file and the user preferences file; do not
merge workspace ancestors. Missing files are empty layers. Validate each
present file in full even when an explicit flag masks one of its keys, so a
flag cannot hide an unknown setting or an authority attempt. Resolve per key:
**explicit flags > workspace file > user-level config > built-in defaults**.
Only a flag actually present counts as explicit; clap defaults do not outrank
files. Presets may suggest choices in init's displayed draft, persisted only
after confirmation; they are not a fifth session layer. C17 resolves consent ceilings and budget-like bounds by intersection first;
free values then use precedence, additive checks accumulate, and locked values
cannot change. D11's journal trust is never a preference layer. `config explain`
reports the chosen file/key or explicit flag, class and effective value,
including absent layers, ignored widenings and skipped unsafe candidates.

Every CLI invocation loads this context before effects, except D11's trust
administration, which reads user-local authority directly so it can repair trust
without loading workspace files. In S3, MCP uses only explicit `--workspace DIR`
in the registration for workspace discovery; otherwise use user preferences.
The chosen directory passes the same home/trust/ownership rules as CLI discovery.
The client's spawn directory and MCP roots never implicitly select a workspace.
Initialization reports workspace-selected versus user/default fallback, with
no absolute paths in model-visible instructions, and states that workspace
overrides require `--workspace`. Tool arguments cannot change this snapshot;
restart/reinvoke for edits, no watcher or daemon. Init's confirmed draft changes
its conversational choices and the new file, never kernel grants; init always
writes an explicit language.

**Named follow-up:** "roots-based workspace detection, once a client-qualified
channel exists" in 06/08. MCP obtains roots after initialize and has no standard
instructions-changed notification; S3 adds neither a roots exchange nor a
per-client notification mechanism. C01/C08 retain their existing host scope.

#### Conversational presentation versus English artifacts

C05c supplies deterministic `en`/`fr`/`es` interface messages: one wording per language,
not three tone variants. C05l migrates existing user-facing messages/errors;
`--help` and the clap reference stay English as documentation. C05g's plain
init and C05k's later TUI consume the same messages. There is no model/network
call just to display help, a menu or an error. Reuse
existing output adapters and interpolate data as data, not translated format
strings supplied by a workspace. Language/tone apply to conversational prose
only; command/flag names, paths, JSON schema keys, status/error codes, citations,
source bytes and machine protocol structure remain stable. Operational logs and
receipts are always English, distinct from localized human-facing diagnostics.
For other accepted language tags, show the English-interface note visibly once per
session; MCP conveys it in initialization instructions, never unsolicited stdout. Pre-resolution
errors use an already validated explicit language, otherwise the built-in
English diagnostic; they still refuse, never silently accept a bad language.

C05d passes only validated conversational choices into ask's trusted prompt
controls through CLI and MCP; knowledge does not depend on the catalog crate.
A language explicitly set in any layer wins for generated replies; absent one,
use the question's language as in S1, not the UI's English default. Init always
writes a language. All evaluation runs (ladder and eval suites) ignore user and
workspace preferences and explicit session overrides, using question language
and their pinned prompt/tone profile to preserve comparability. C05d updates
`answer/generate.rs`'s response-language selection and en/fr/es host-owned
refusal text (English fallback with the visible note), and makes the public
`lang` field carry the selected canonical tag rather than an en/fr-only enum.
Keep source quotations and evidence validation intact. Check language only
where the detector supports it; otherwise record `unchecked`, never fabricate
a pass or refuse solely because a tag cannot be checked. A French answer may explain English code, but any generated code/comments,
commits, file names, identifiers, logs and documentation are English; tone never
changes artifact content. Do not translate existing user material or pretend
native convenience enforces a host's output.

Brief means the shortest complete response; normal supplies the needed context;
detailed explains more, without changing token/budget ceilings. Every tone
retains mandatory warnings, checks, citations, refusals and unavailability or
uncalibrated markers. Record the language/tone prompt inputs in the existing
prompt/profile identity; prior S1 measurements do not automatically qualify a
changed prompt. This task does not redo S1 calibration or create an S4 engine.

C05e constructs the English session fragment from the canonical tag (quoted as
data, or an explicit question-language fallback), tone and fixed artifact/log
rules. `KnowledgeServer::get_info` supplies resolved explicit-workspace/user
preferences in initialization instructions for all four clients. Include the
path-free source indicator, the `--workspace` guidance and any UI fallback note.
C05e tests an explicit nested workspace and an empty folder, with/without
`--workspace`, including a client that reports roots: roots and spawn directory
never alter the S3 session choice.

C06/C07 project only the fixed English artifact/log rule plus "Follow the
Maestro MCP server's session instructions for language and tone", never the
language/tone values. Thus changing a preference does not drift a host file.
Session fragments and static projections share the fixed rule constructor, not
copied policy prose. Model/tool arguments cannot alter either. No new tool,
resource, plugin or launcher is introduced. Tests inspect actual initialization
payloads and both projections; they prove delivery, not obedience.

**S4 session-preferences launch-adapter obligation:** reuse this fragment in
the actual trusted instructions for every launched, resumed and delegated
agent, preserving the session snapshot. Acceptance observes each actual adapter
payload for the two provider routes, all three tones and a non-interface
language, plus English artifact/log rules and refusal of model-supplied
preference changes. C05e hands this obligation to architecture 06/08; S3 adds no
agent launcher and makes no live S4 compliance claim.

#### Branded init flow

C05g first implements the flow port, plain prompts and script parity without a
new dependency; C08's owner loop does not wait for a TUI or OA9. Separately,
OA9 approved ratatui/crossterm on 2026-09-28 under ADR-0020; C05f measures their
minimum adoption footprint before C05k implements the renderer.
Record visual acceptance during a later C08 walkthrough when the renderer is
available; it is required at C28, not a prerequisite for the first C08 loop.
No general dashboard/theme system is added.
The owner asks for "very nice looking, AAA, pristine": consistent spacing and
hierarchy, uncluttered panels, visible step/focus, concise help and actionable
inline errors. This is a visual quality target, not an accessibility certification.

Brand sources are the organization's
[profile](https://github.com/Orchestration-Maestro/.github/blob/main/profile/README.md),
[dark mark](https://github.com/Orchestration-Maestro/.github/blob/main/assets/maestro-mark-dark.svg)
and [flat mark](https://github.com/Orchestration-Maestro/.github/blob/main/assets/maestro-mark-flat.svg),
with the supervisor-confirmed supporting palette (the originally named
`brand/BRAND.md` does not exist). Use rust `#B7410E` for brand/primary accents,
ember orange `#FF6A1A` for focus, sand `#D9A066` and copper `#C8743A` for secondary
accents, slate `#5E7383` for borders, near-black `#0E0B09` and dark brown `#2A1F1A`
for backgrounds, and cream `#F2E8DC` for text on dark. Never use low-contrast
brand accents as small body text: check at least 4.5:1 for body text and 3:1 for
focus/control boundaries; use labels/markers as well as color.

| Screen | Content and exit condition |
| --- | --- |
| 1. Workspace, trust and preset | Display the canonical init root, authoring/verified mode and preset; explicitly ask whether to trust this clone/workspace, showing existing approval or its absence. Decline offers only the separately confirmed config record; no implicit grant or script execution |
| 2. Language | English (`en`), French (`fr`), Spanish (`es`) and entry of another supported BCP 47 subset tag, current value/source and visible interface fallback note; never change artifact language |
| 3. Tone | Brief, Normal, Very detailed; show one small conversational sample and the English-artifacts notice |
| 4. Updates and overrides | Off or Propose (default); user-only Auto is shown as a ceiling, never enabled here; documented keys, ranges and sources; no permission toggles |
| 5. Review | Config values, root and every proposed file/collision; Back, Cancel, Preview/Exit, and explicit confirmation when `--apply` is present |

Tab/Shift-Tab traverse controls; Up/Down select; Enter chooses or advances;
Escape goes Back (or cancels on the first screen); Ctrl-C always cancels.
Confirmation defaults to no. Errors retain entered values and focus the invalid
field. No mouse, animation, audio, remote font or emoji is required. Restore the
terminal on normal exit, cancellation and errors, including interrupted apply
whose recovery remains C04's responsibility. Report the resulting config path
and how to inspect its effective values after a successful apply.

`--plain` uses sequential, labelled prompts suitable for a screen reader,
without raw mode, alternate screen or cursor repaint. `--no-color`, `NO_COLOR`
or a terminal without color disable color; all information remains available.
`TERM=dumb` and terminals below 80×24 use the plain flow, with wrapped text;
resize never discards a draft or authorizes a write. Plain mode supports the
same choices, Back/cancel and final preview/confirmation, not a reduced feature
set. Check contrast, keyboard order and plain output, not just screenshots.

Init opens the interactive menu when stdin/stdout are terminals unless `--yes`,
`--plain` or `--json` selects the appropriate non-TUI path. `--yes` makes init
non-interactive: explicit `--preset`, `--language`, `--tone`, `--updates` and repeatable
`--set KEY=VALUE` provide choices; omitted preferences use the resolved values
and ultimately `en`/`normal`; init persists off if updates are off, otherwise
propose, never a workspace auto grant. Refuse conflicting duplicate flags/keys and all
invalid values. A preset must be explicit outside the menu. `--yes` accepts the
validated draft without prompting, not permissions, trust grants or file collisions. A script must use an existing
matching local trust approval for effectful init. In a fresh home, first run
`maestro trust add DIR --confirm-path DIR` with the exact canonical root; missing
approval returns that explicit command, never a hidden prompt or grant.
`--apply` is always required for writes, including with `--yes`; without it,
print the exact preview and stop. In a non-terminal or `--json` path without
`--yes`, refuse choices/confirmation that would require interaction. JSON mode
emits the existing versioned machine result only, with no escape sequences or
menu on stdout. Plain/scripted and TUI paths call one planner/validator/writer.

### D7 Settings, policies and graph checks

C17 precedes init's configuration tasks. Free values use
explicit flags > workspace file > user-level config > built-in defaults.
Resolve bounded consent/budgets by narrowing across layers, not by selecting a
winning layer first. Language/tone are free; `updates` and `[overrides]` use
the bounded contracts from D6/D9. Permissions
intersect with parent grants; prohibitions and checks accumulate; budgets take
the stricter bound. A capability's values affect its own role or step only.
`config explain` and `catalog explain` keep declared, effective and observed
states distinct. A missing S4 qualification is unsupported, not evidence that a
model or sandbox works.

Carry the architecture 03 defaults as declarations, not an S3 run engine:

| Setting | Default | Class |
| --- | --- | --- |
| Conversational language; tone | Question language unless set; UI/init `en`; tone `normal` (English artifacts/logs always) | Free |
| Update policy | `propose`; off available; user-only catalog auto never skips trust, widening/hook consent or idle boundaries; runtime/MCP never auto-apply | Bounded |
| Model profile; reasoning | `balanced`; model default, diagnose unsupported effort | Bounded |
| Maximum output | 4,096 tokens where the profile permits | Bounded |
| Inference/workspace writers; delegation depth | 1/1; 2 | Bounded |
| Tool calls; repair attempts; routing candidates | 40; 2; 3 | Bounded |
| MCP call timeout | 30 s within the server profile; long knowledge calls need an explicit supported profile, not silent truncation | Bounded |
| Cross-project memory; apps/extensions/schedules | Off until qualified/activated | Bounded |
| Raw prompt/reasoning logging; provider fallback | Off; none | Locked |
| Evidence/result validation; discovered executable hooks | On; off | Locked |

Cedar evaluates normalized requests against the real schema/policy set. Trusted
actor, allowed operation, target and approval facts come from the host adapter,
not the submitted text. S3's native checker never invents an approval: an
operation needing an unavailable trusted fact fails closed. Unknown tools,
opaque shell constructs and evaluator diagnostics deny. Every rule has an
allowed neighbour, a denied case and a spy proving zero executor calls on
denial. A native hook is defence in depth, explicitly unprotected when absent;
it is not a sandbox or S4's authoritative broker.

Split graph checking into topology (C22a) and contracts (C22b). Together they
cover all twelve architecture 03 §2.3 rules, including conditions with only the
specified comparisons/boolean/array predicates, exact router choices,
reviewer independence, bounded maps and subgraphs, policy/sandbox requirements,
budgets, state flow and outputs on all successful paths. No condition executes
code, no graph runs and no qualification card is fabricated.

C21 adds `ctm-question` with the shared policy/knowledge resources; C21b adds
`feature-delivery` with planner/coder/tester/reviewer and their contracts.
Profiles for both providers may be authored/reviewed without being
live-qualified; routing excludes unsupported combinations. Synthetic
eligibility records test the compiler/router, never qualify live execution.

### D8 Impact and the later comparison

C27a owns the missing catalog projection. After C12, S2 G25 qualification and
S2 G27's public typed-edge port at
`crates/maestro-knowledge/src/graph/projection/port.rs`, project exact scoped
component edges from kernel authority into S2's embedded backend. G27 is owned
by the S2 spec/task set; it exposes application-ID operations only. S3 owns the
catalog edge schema plus write/read adapters and snapshot bindings. Keep this
projection separate from S2 evidence-span claims and rebuild it only from C12's
records; never fabricate evidence to satisfy a knowledge-claim schema.

C27 traverses C27a for exact transitive workflow dependants under caller scopes,
after the shared trust admission. It returns the consulted snapshot and
preserves results across rebuilds. If G25/G27 are absent, block C27a and that M3
exit explicitly. C00 records ownership in 08; no similarity edge, second graph
store or unapproved in-memory closure fallback fills the gap.

Only C29, after M3 and owner-granted access, opens the earlier catalog. It
inventories every item once, records keep/recover/defer/reject with provenance
and reason, and proposes separate ≤4 h recovery tasks/PRs. There is no bulk
import or silent rewriting of imported approval obligations.

### D9 Startup updates through the existing lifecycle

C16c–C16g extend C13/C14/C16, not a second downloader, verifier, installer or
background daemon. Discovery covers the released Maestro runtime and every
installed catalog with its exact pinned component closure. Runtime and catalog
releases keep distinct publisher bindings but share verification/admission.
S3 runtime updates are propose-only: C16e verifies the release and compatibility,
C16g shows its exact installer command without executing it. S3 never stages,
activates or rolls back the running binary; automatic runtime activation and
installer layout/recovery are a named follow-up, not hidden in a four-hour task. Do not independently upgrade a catalog component away
from its bundle closure, resolve Cargo dependencies, update pinned `gh`, install
host clients or download models. Qdrant setup and the router are not S3 targets.

With `updates = "off"`, perform no startup discovery and zero network calls for
version checks; explicit `maestro update check` still works. This does not turn
off mandatory trust admission/refresh. Otherwise at session startup, before the
first task or MCP tool call, check installed identities against verified
production release metadata at most once every 24 hours per managed installation. Record the last attempt as well as success
using kernel state, so concurrent sessions and repeated offline failures do not
cause a request storm. Bound the discovery attempt to five seconds total;
timeout/network failure produces an offline/unavailable notice and leaves the
current install intact. Reuse a cached verified proposal only while its trust
records remain valid. A daily version check is not ADR-0015 trust refresh:
five-minute freshness admission and at-most-24-hour offline expiry still apply
on every protected use. A rollback of the wall clock never extends either limit.

`updates = "propose"` is the default. A proposal identifies target, old/new
version/digest, verified release changes, compatibility and permission/hook diff,
and gives one exact catalog command: `maestro update apply PROPOSAL_ID`.
Runtime proposals instead show the exact command from the approved released
installer contract; if no qualified installer command exists, report that
installation layout as unsupported rather than invent a command or invoke a
package manager. Treat release notes as untrusted display data, never
instructions. `maestro update check` explicitly refreshes discovery through the
same path; `maestro update rollback RECEIPT_ID` requests a prior catalog state.
These are thin lifecycle adapters; `maestro catalog update` enters the same
planner/admission/receipt path. No proposal is verified from a tag, checksum
alone or unverified notes.

MCP sessions never apply updates, including rollback or catalog auto mode.
Through `ClientPreferencesDelivery`, initialization includes at most one fixed
English notice with proposal ID, target and old/new versions (or a bounded
unavailable status). Never insert release-note text, commands, URLs or arbitrary
source prose into instructions. CLI displays the detailed proposal/apply command;
MCP keeps protocol stdout clean and sends no new notification type.

User-level `auto`, narrowed as in D6, is standing consent only for a verified,
compatible catalog update with no widened permissions and no hook change.
Compare exact old/new mandatory policy, permission surface, hook digests and
closure. A missing or inconclusive comparison becomes a proposal requiring
approval, never an assumption of safety. Runtime proposals remain proposals
under every setting. Changed hooks always ask even if the change looks safer.
Show that diff and bind the human confirmation to proposal/source/target digests;
revalidate them before apply. Generic `--yes`, model text and a preference file
cannot supply that approval. With no interactive human or durable scoped approval,
leave the proposal pending and continue only under the valid current install.

All catalog activation and rollback require an exclusive managed-installation lease
and zero active tasks/sessions using the affected runtime/catalog snapshot,
including existing kernel job leases, not merely the foreground CLI command.
Startup reserves this boundary before admitting the first task; a racing active
session makes the update pending. Preview/read-only commands never auto-activate;
release-check bookkeeping is kernel-internal, not a workspace file effect.
Never change a live MCP session's pins. Catalog downloads use content-addressed
staging and C16's transaction/recovery seam, never partly verified candidates.
S4 must take the same lease around tasks before reusing this lifecycle.

C16d records a durable receipt for every activation, with old/new digests and
versions, component closure, publisher/trust revision, effective policy/source,
permission/hook comparison, approval (if needed), outcome and rollback target.
A proposal cannot add trusted folders or alter kernel-local trust records;
any newly needed external target requires C05h's separate user-only trust
operation, never a catalog-authored grant or the generic update confirmation.
Retain/pin the previous artifacts and compatible state before switching. A
migration or update without a tested reversible state transition is not applied,
including in auto mode. Receipts/logs are English, not localized conversation;
show a localized summary. Their bytes are invariant across language/tone after
normalizing timestamps and operation IDs.
Rollback checks the target against current trust, compatibility and version
floors, then atomically restores artifacts/pins and records a linked receipt.
A retained artifact is not a right to use a revoked release: refuse unsafe
rollback and explain the safe forward-update path. Startup checks do not block
valid work just because newer-release discovery is offline; they never bypass
trust refusal of the current install.

**Named follow-up:** "notify-only update checks for owner-managed components
(Qdrant, router runtime, host clients, models)" in architecture 06/08. Each needs
an approved source before even proposing; none is ever applied by that follow-up.
The pinned verifier remains owner-managed. **Named follow-up:** "runtime
auto-apply with the installer" in 06/08 must first define released-installation
layout, cross-platform startup handoff, idle exclusion, durable receipts and
trusted reversible rollback; no automatic runtime switch is claimed in S3.
No dependency-management subsystem is added by this spec.

### D10 Small modules and explicit ports

The owner's 12:05 modular/extensible/plug-and-play requirement authorizes these
small ports, not a plugin runtime, service layer or speculative common crate.
Use typed inputs/results, constructor injection and a small composition root;
no filesystem, terminal, provider SDK or client types cross the domain ports.
Keep pure parsing/resolution/instruction construction beside their source-named
tests. Config/instruction types and the delivery port live in maestro-catalog;
the binary supplies the MCP adapter, so catalog never depends on CLI/MCP types
or on the binary. Human interface translation stays in the binary's presentation
module; knowledge accepts only mapped presentation inputs, never a catalog dependency. Register an adapter once; callers do not switch on a host or release kind.

| Module / port | Input → output | Default adapters and invariant |
| --- | --- | --- |
| Workspace config / `WorkspacePreferences` | CLI working directory or explicit MCP workspace, platform config home, flags → validated preference snapshot with provenance | Strict TOML adapter over ADR-0018 ownership/trust-bounded discovery; storage-independent resolver; no paths, receipts or grants in preference files |
| Path trust / `WorkspaceTrust` | Trusted actor, operation, canonical/held path and approved roots → allow/deny with reason | Kernel-journal approval + filesystem adapter, immutable `secret-paths.json` deny data; every implementation keeps the mandatory deny floor |
| Updates / `UpdateSource` | Installed identity and approved publisher binding → verified candidate/unchanged/unavailable | Both adapters reuse C13/C14; catalog activation/receipts/rollback use C16, runtime exposes proposals only |
| Instructions / `conversation_instructions` | Validated language/tone snapshot → English instruction fragment with fixed artifact/log rule | Pure construction, no host/config lookup or authority; tested once and reused |
| Client delivery / `ClientPreferencesDelivery` | Fragment and validated session metadata → client initialization or projection payload | MCP instructions for all four clients; native Copilot/Pi adapters; a fifth client adds an adapter, not caller branches |

The terminal/plain UI are adapters over one init flow/planner. Port contract
suites run against every shipped adapter, checking refusal as well as success.
Additional trust/update adapters cannot replace locked secret denies, verifier
bindings, approval rules or idle activation with a permissive implementation.
No new dependency or runtime-discovered plugin follows from the word extensible.

### D11 Workspace trust without configuration self-authorization

C05h records trust answers, canonical paths and approval receipts only in
user-local kernel authority records keyed by canonical path, beside their
journal approval. The workspace file holds no paths or receipts and rejects
`[trust]`; init creates preferences only. Manage records with `maestro trust
add/list/remove`; `config explain` and menu screen 1 read the journal directly.
A preference edit never blocks trust changes: `trust add` does not rewrite
`.maestro/config.toml` or touch its ownership digest. Decline records no grant;
missing records mean untrusted. Removal revokes that root for subsequent
controlled effects, without modifying preferences. Reuse kernel state, not a
new authority store, and derive all test paths from isolated homes.

`maestro trust add DIR` accepts the init root or an explicitly selected larger
workspace. On a terminal, display its canonical path and ask for confirmation,
default no. Without a terminal require `--confirm-path DIR` repeating the exact
canonical absolute path; no shorthand, link spelling or mismatched repetition
counts. Without it, return status 2 and the exact command to run. `--yes`,
`--json`, environment variables and MCP/model text never constitute approval.
Refuse filesystem/drive roots, mount roots, the home directory itself and
kernel-internal directories. Init uses its working directory; select a larger
workspace by explicitly running trust add on that directory, never by implicit
ancestor grants. A fresh-home CI setup first runs `maestro trust add DIR
--confirm-path DIR`, then `maestro init --yes --apply` with its preset/choices.
Trust commands use explicit user-local authority, not discovered workspace
settings, so an untrusted or invalid config cannot prevent this setup.

Copied preference files cannot grant trust. Catalogs/updates cannot create or
modify trust records. The Copilot hook denies agent-shell invocations of trust
administration, including a correctly spelled `--confirm-path`; the explicit
command is not process-origin authentication. Pi/Codex/Claude agent-shell
containment remains the named S4 hook proof. S3 cannot distinguish arbitrary
same-user processes outside its controlled surfaces; do not claim otherwise.

Declining trust normally prohibits all workspace writes. The owner-approved
exception is one separately confirmed, digest-bound write of init's config and
its ownership/approval metadata only; no preset, template, hook, projection
or update may piggyback. Cancel/preview writes
nothing. The declined-trust config contains preferences only; the decision,
paths and approval metadata stay kernel-internal. An external host install
folder outside current roots needs its own explicit user trust action, requested
once on first installation and reused only with its journal proof. Never grant
HOME broadly because a client's skills folder needs a write.

The default policy evaluates held/canonical paths, not strings:

1. Always refuse built-in secret locations and kernel-internal storage to
   agents/tools, even under a trusted ancestor. Secret data names SSH/GPG roots,
   cloud/CLI credential stores (`.aws`, Azure/Google credential locations,
   `gh/hosts.yml`, `.docker/config.json`, package-manager token files), password
   stores, platform keyrings, and files named `.env` or `.env.*` at any depth.
   Resolve home/XDG/Windows aliases from existing platform bindings, not a
   checked-in machine path. Both a supplied secret path and its resolved target
   stay denied; a symlink alias is not an exception.
2. Permit non-secret reads inside or outside trust, subject to existing scopes,
   budgets and Cedar restrictions. A path policy cannot grant knowledge access.
3. Permit writes only under a journal-approved trusted root, still intersecting
   other mandatory controls. Outside writes deny, including projection/update/
   rollback destinations. For a new file, validate its existing parent ancestry
   with ADR-0018 handles; refuse link/reparse escapes, `..` escapes, prefix
   lookalikes and swaps between check and effect.

Deny rules are immutable, bounded, versioned built-in data in
`crates/maestro-catalog/resources/secret-paths.json`, not a workspace-editable
allowlist. Each record declares its platform/root alias and exact path/subtree
or basename rule; unknown shapes refuse. Adding a known secret location changes
this data and its allowed/denied contract fixtures, not policy callers. Catalogs
and updates cannot remove a mandatory deny or widen trusted folders. Updating
the runtime's policy data still needs the verified release/change-consent path;
an adapter cannot claim a weaker floor is a harmless update.

Maestro-owned XDG config, data and state directories are kernel-internal:
Maestro's processes write there through kernel rules, including approval/update
bookkeeping, without granting agents/tools access through workspace trust.
Credential references remain opaque runtime bindings, not permission to read
secret files as tool content. Treat operation origin as a trusted adapter fact,
never a caller/model boolean that says "internal".

C05j enforces the port at Maestro's controlled file-effect boundary; C06/C07,
C16 and update/rollback reuse it. C20 adds the same check to Copilot preToolUse,
with zero effects on denied writes/reads and fail-closed normalization/errors.
**S4 non-Copilot workspace-trust hook obligation:** qualify Pi, Codex and Claude
Code's trusted event/identity adapters against outside-write, secret-read,
symlink-escape and agent-shell trust-add cases (including `--confirm-path`),
each with an allowed neighbour and
zero executor calls on denial. Record it beside the already-deferred S4 hooks
in 06/08; configuration or instructions alone are not containment.

## Data model

Reuse kernel artifacts, scopes, journal and transactions; do not duplicate
collection or model registries. C12's migration is next free at landing, above
every migration landed or reserved on `main`, `feat/s1-integration`,
`feat/s2-integration` and `feat/s3-integration`. C00's coordination check on
2026-09-28 found main ending at 0005 and S2/S3 at 0011; the supervisor confirmed
S1 also ends at 0011 and no outstanding reservations exist. S2 already uses
the same next-free rule. Recheck these moving heads and lane reservations at
every landing. C00 allocates no number; `NNNN_catalog.sql` is the number
assigned at landing, never a fixed/gapped reservation.
C14/C25 use the same record seam; later schema changes get newly allocated
numbers, never edits to applied migrations.

| Record | Stored identity and invariant |
| --- | --- |
| Bundle artifact | Digest, source revision and normalized manifest; immutable verified bytes |
| Scoped install | Scope, bundle digest/version, compatibility and verification receipt; active pointer changes atomically |
| Components/closure | Bundle, stable resource ID, kind/digest/maturity, exact dependency edges; no text-inferred relation |
| Artifact pins | Install and project references use existing kernel pins; failed transactions do not leak pins |
| Trust state | Publisher, authenticated record digests/revision, issued/expiry, floor, entry/bundle revocations and last accepted clock observation; monotonic and atomic |
| Discovery binding | Install snapshot to catalog collection/generation; only complete verified generations advertised |
| Workspace/user preferences | Versioned TOML at the nearest workspace root or platform configuration home; language/tone, update policy and allowlisted overrides with provenance, never authority |
| Workspace trust approval | User-local kernel answer, canonical path and approval receipt, keyed by canonical root and changed only through explicit user trust administration; never a workspace field or a second authority database |
| Update state and receipt | Per-installation release-check attempt/result, exact verified proposal, idle/activation lease, prior artifact/state pins and linked apply/rollback receipt; reuse kernel transactions and the shared lifecycle |
| Project lock | Bundle/components, runtime and host versions, model identity/quantization/template/build, supported OS/sandbox profiles; unsupported values explicit, never authoritative |
| Owned operation | Target root, relative path or owned JSON entry, previous/proposed digest, operation progress; recovery and removal preserve user edits |

## Contracts

C03/C09 freeze source/bundle schemas and C05a freezes the preference schema
below before their consumers are written. Authoring and compiled schemas are distinct. Unknown versions fail;
there is no raw SDK configuration passthrough.

| Surface | Contract |
| --- | --- |
| Authoring check | `maestro catalog check --catalog-dir DIR`; strict frontmatter/TOML/JSON, fixed agent body sections, owner/maturity/08 references, exact dependencies and diagnostics |
| Compile | `maestro catalog compile --catalog-dir DIR --output FILE`; deterministic tar with `bundle.json`, normalized entries/closures, source and runtime/feature/tool requirements |
| Install/update | `maestro catalog install VERSION`, `maestro catalog update`; verified compatible releases only, explicit project lock update, no authoring or unsigned option |
| Bootstrap | `maestro init` opens plain prompts first, later the approved TUI; `--preset knowledge-client\|rust-service`, `--language TAG`, `--tone brief\|normal\|detailed`, `--updates off\|propose`, `--set KEY=VALUE`, `--yes` support scripts. Preview by default, `--apply` to write, `--plain`/`--no-color` for fallbacks; `--catalog-dir DIR` selects labelled authoring-only mode |
| Session preferences | Global `--language`, `--tone` and free `--set` values override the safely discovered workspace file, user `preferences.toml`, then defaults; update/budget values only narrow. No language set means question-language answers. MCP uses `--workspace DIR` only, otherwise user preferences; no roots-based discovery in S3 |
| Workspace trust | `maestro trust add DIR` asks default-no on a terminal; otherwise needs exact canonical `--confirm-path DIR`, never `--yes`/`--json`; missing confirmation exits 2. Refuse filesystem/drive/mount roots, HOME and kernel-internal directories. `maestro trust list`/`remove DIR` read/change user-local authority only |
| Hosts | `maestro catalog project --host copilot\|pi --dry-run`, `--apply`, or `--remove`; preview-only by default, owned changes only |
| Resolve/search/route/impact | CLI `maestro catalog resolve ID`, `search QUERY`, `route INTENT`, `impact ID`; MCP `catalog_resolve`, `catalog_search`, `catalog_route`, `catalog_impact` under the existing bounded stdio server |
| Explain | `maestro config explain` and `maestro catalog explain ID`; declared/effective/observed values, class, source rule, requester and unsupported/not-observed states |
| Policy | `maestro policy check --stdin` and `maestro policy test --catalog-dir DIR`; bounded normalized input, Cedar diagnostics, allow/deny/approval-needed result without executing effects |
| CLI output | Existing `--json` convention, versioned strict output, diagnostics on stderr; exit 0 success, 1 operation failure, 2 refused input/usage. A test command that discovers no cases does not pass |
| MCP output | Typed statuses and bounded results, caller-bound context, exact snapshot and used generation; initialization instructions expose conversational preferences and any interface fallback note; never expose arbitrary Qdrant filters or administration |
| Startup updates | `maestro update check` works even when startup checks are off; `apply PROPOSAL_ID`/`rollback RECEIPT_ID` use C13/C14/C16 for catalogs only. Runtime proposals show a verified release and exact approved installer command; no runtime apply. MCP never applies updates; path-free fixed notice only |

## Validation

1. **First checkpoint (through C08):** strict config, safe discovery/narrowing,
   en/fr/es/ja × three-tone prose and byte-identical artifact/log tests, four-client
   --workspace/user-only instructions, user-local trust and fresh-home CI setup.
   Prove outside-write/secret-read/link-escape denials and plain/script parity.
   Then base/Rust init, real Copilot/Pi projection/restart, knowledge tools,
   citation/refusal checks, separate denied kernel, repeat and owned-only removal.
   C05f/C05k's later TUI/keyboard/resize/contrast proof and OA9 visual acceptance
   are required before C28, never before this first plain owner loop.
2. **Trust/update proof (C09–C16g):** real valid/wrong-signer verification,
   deterministic archives, hostile reader cases, atomic records/update,
   expiry/revocation, rollback/clock tests and old-backup replay refusal. Add
   startup off/propose/catalog-auto, widening/hook consent, catalog receipts,
   runtime propose-only/install-command checks, MCP zero activation,
   reversible migration, offline/throttle and no-mid-task tests. Test publisher rotation
   and withdrawal only after the owner's external setup.
3. **Settings/content proof (C17–C22b):** all classes and narrowing rules, source
   explanations, real policy allow/deny neighbours, twelve graph-rule neighbours
   and declarations that remain ineligible without qualification.
4. **Routing proof (C23–C27):** frozen labels, exact baseline, pre-limit
   eligibility, scoped/rebuildable cards, paired hybrid verdict, S2 exact impact;
   failed comparisons and unsupported conditions stay in reports.
5. **M3 proof (C28):** the owner provides a disposable clean WSL user/container
   with released `maestro`, pinned `gh`, authenticated read access and basic shell
   utilities only. Copy `scripts/tests/catalog-m3.sh`, not a checkout; it first
   asserts `cargo`, `rustc` and `python3` absent. Exercise install/init/project/
   update/remove, four-client T038 registration, real `incompatible` routing,
   synthetic measured routing, M1 release and final three-OS CI evidence.

Every task has a red-first check, bounded files and a ≤4 h budget including its
local checks. Split overruns into fresh, independently testable follow-ups;
never weaken a boundary to finish. Only the supervisor integrates. Mutation and
coverage run in CI, with zero misses/timeouts required before main merge, not
as a workstation landing job. Re-estimate after C08.

## Complexity Tracking

No rule violation or speculative subsystem is approved. Reuse S1 stores,
transport, model access and retrieval; add the catalog crate, pure shared
filesystem extraction and necessary kernel records. The first checkpoint omits
distribution and routing work only
in sequence, not from M3 scope.

## Needs owner action

This register distinguishes approved scope from remaining owner operations.
A lane performs only its assigned approved scope; D1–D5 alone grant no credentials
or external permissions. OA2's dated isolated installed-tool tests are approved;
anything broader still waits. Wait time is outside task budgets; mark missing
evidence blocked, never passed.
OA7's frozen inventory and hook deferral are approved by the owner, 2026-09-28.
Only OA7's remaining actions appear below.

| ID | Owner action | Needed by |
| --- | --- | --- |
| OA1 | Create public `Orchestration-Maestro/maestro-manifests` with its first reviewed generic content; assign maintainer/backup and security/platform owners; configure visibility, rulesets, required checks, CODEOWNERS protection and organization properties | Real MAN content in C02/C21/C21b/C23 and workflow landing in C15, not CORE fixture code |
| OA2 | **Approved 2026-09-28:** S3 may probe/test the already-installed Copilot CLI, Pi, Claude Code and Codex, each in an isolated temporary home. C01 records every exact installed version as its pin, including adapters used. No installs/upgrades, changes to real owner configuration or enterprise policy. Anything beyond this scope requires fresh OA2 approval; private/provider data approval remains OA6 | C01, C06–C08, C20, C28 may use this bounded scope; unavailable tools/access or broader operations remain blocked. No checker/bootstrap/routing gate |
| OA4 | Bind separate catalog/runtime repository/workflow/issuer identities, protected environments and emergency/rotation authority. Supply checksum-verified standalone pinned `gh` and approved `gh` login or `GH_TOKEN`; authorize any unlisted-licence organization allowlist change, not a second D4 library approval | C09's owned 08 §17 closure and C28 live trust proof; no C09/C13 implementation-start gate |
| OA5 | Publish checksum-pinned compiler and catalog canary/stable bundles/attestations. Enable the catalog publisher's six-hour trust schedule with protected `id-token: write`/`attestations: write` permissions and minimum publication access; enable hourly missed-refresh alerts to maintainer/backup. Perform withdrawal/rotation/missed-run drills. Supply C28's clean WSL user/container with released `maestro`, pinned `gh`, basic shell utilities and read authentication, but no checkout/Rust/Python | C28 release/clean-environment proof; C15 workflow code and C16 fixture tests do not wait |
| OA6 | Grant private-data use per exact client/provider/account/scope, private receipt location and approved model downloads/access. Without it use synthetic data; do not infer permission from T038 code or a logged-in client | C08, C23, C26, C28 only for the corresponding private/model access |
| OA7 | Accept M3 evidence and eventual main release. Any S2 fallback needs a separate explicit approval and 08 disposition | C28 exit; not a new product-choice gate |
| OA8 | After M3, grant access to the earlier catalog and approve any recovery work, provenance/licence obligations and separate publication PRs | C29 and later recovery tasks |
| OA9 | **Approved 2026-09-28:** ratatui + crossterm for the TUI under ADR-0020. C05f still measures minimum features/dependencies/licences/native links and vet before adoption. Owner visual acceptance of C05k's branded keyboard/plain/no-color walkthrough remains pending | Library choice resolved; C05f measurements precede C05k. Visual acceptance gates C28, never the first plain C08. No parser dependency or decision |

The lane's authorized signed push to its own branch is not a product release.
It does not create a repository, change shared settings, activate workflows,
publish a tag, install clients or send private data to a provider.

## Risks and estimates

| Risk | Early check and response |
| --- | --- |
| Scope/review outruns value | C02/C08 visible checkpoints; one task per fresh lane, no speculative files |
| Host parser/provider drift | C01 real pinned probes; absence blocks instead of yielding a false pass |
| Workspace settings become authority or silently fall back | Separate preferences from kernel grants; strict full-file parse, nearest-file errors, provenance and C17 narrowing tests |
| TUI polish hides accessibility or estimate risk | C05g plain flow first, C05f measured choice and C05k renderer/owner walkthrough; C05l separates existing-message migration. No dashboard or theme framework |
| Language/tone changes invalidate answer evidence | C05d preserves validation and records prompt identity; no reused calibration claim or translated artifact/evidence IDs |
| Automatic updates change authority or active work | C16c–C16g use user-only catalog consent, current trust, exact permission/hook diffs, idle leases and reversible receipts. Runtime/MCP cannot apply; uncertainty proposes |
| Data loss or trust replay | C04a preserves existing ADR-0018 tests; C04 crash/race cases, C13 real signatures, C14 clocks/revocation, C16b restore |
| Publisher outage refuses every laptop within 24 h | Six-hour attestations, independent hourly alert at eight hours, OA5 missed-run drill; never extend expiry to hide an outage |
| Auth/API limits prevent five-minute refresh | C09 measures pinned `gh` authentication and calls per active laptop/root; doctor checks readiness; rate limiting uses the bounded offline window |
| Privacy or missing qualification | Synthetic default, exact approvals; S4-only roles yield real `incompatible`, not a vacuous measured routing success |
| Trivial routing scores | Frozen synthetic eligibility digest, nonempty denominator, independent distractor labels and paired held-out comparison |
| S2 G25/G27 late or incompatible | C27a owns catalog schema/adapters; S2 owns the public port. Block impact/M3 until qualified; no fabricated evidence claims or implicit fallback |
| Migration collision across slices | Supervisor allocates above all landed/reserved numbers on main and S1/S2/S3 integration branches |

The revised **53 tasks total 177 lane-hours**, each at most four hours.
**70 hours to the owner-loop checkpoint** = original 29 + 33 for C05a–C05j
without the later C05f measurement + 4 for C05l migration + 4 for C17 moved
before init. Then **104 more hours to M3**, including C05f (2), C05k (4),
C16c–C16g (15) and C20's extra trust-hook hour; **3 hours** remain post-M3.
Compared with the reviewed 169-hour plan, C05k and C05l add 4 hours each;
C16e's former 4 hours split into propose-only C16e (2) and C16g (2).
Allow **193–201 lane-hours** with the unchanged 16–24-hour review/CI reserve. Re-estimate calendar delivery
after C08; the draft's 7–10-day M3 estimate did not include the added projection
and task splits and is not a commitment. At six lanes/eight effective hours,
total effort still does not divide the serial dependencies by six. M1 may finish
in parallel; its release, owner actions, live hosts and S2 qualification remain
exit waits outside these budgets. Shared CLI, lock and migration work stays
serialized.
