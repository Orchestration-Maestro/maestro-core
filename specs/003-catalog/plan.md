# Implementation Plan: Catalog

**Branch**: `docs/s3-manifest-v4` (base `815ff33`) | **Amended**: 2026-09-30 | **Spec**: [spec.md](spec.md)

**Input**: the approved S3 draft, owner decisions of 2026-09-28 01:56, 08:12,
11:25, 11:50, 11:55, 12:00, 12:05 and evening kind/model-card/settings amendments,
the owner-approved 2026-09-30 final manifest v4 design (minimal Phase 1),
all gap additions and corrected G12 Phase 2 placement, and the review rulings,
architecture [03](../../docs/architecture/03-agent-orchestration.md),
[06](../../docs/architecture/06-roadmap.md) and
[08](../../docs/architecture/08-traceability.md). Tasks: [tasks.md](tasks.md).

## Summary

Build the smallest useful catalog first: one knowledge preset, the Maestro
profile and the dependencies of `workload-question`, then common-plus-Rust starters. Explicit
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
workflows; S4 will execute them. Model cards become one additional declarative
kind, reusing the kernel registry rather than replacing it. C29 audits owner-approved legacy recovery before M3; private sources remain
out of scope. Phase 2 starts immediately after M3 at the checkpoints in D15.

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
canonicalization callers. The owner created `maestro-manifests` with README and
MIT licence. Its approved owner-first README precedes content; C02 waits for the
`/2` checker migration and approved ownership/protections.

**Performance Goals**: trust refresh interval at most five minutes while in
use; offline expiry at most 24 hours. Bound parsing, downloads, subprocess time
and output before allocating or invoking a verifier. Report routing latency,
not an invented latency target. Preserve S1's free-room-only model loading.

**Constraints**: no private vendor content, personal paths or secrets in public
files/logs; every new library measured and approved; no executable source
content during check/compile/init; no silent loss of user files or permissions;
no inference of identity, approvals or qualification from model text.

**Scale/Scope**: two v1 workflows, their required roles/resources, core and Rust
composition, two native projections, four existing MCP registration guides,
100+ independently reviewed intents. Rust is the first init language composition;
the well-formed BCP 47 subset in D6 selects conversational language. Built-in interface
strings start with `en`, `fr`, `es`; other languages use English interface text
with a visible note, without changing conversation language. The approved v4 seed adds eight root language declarations and ten core
personas; seven complete non-Rust gates and live execution remain separate.

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
| Kernel | Scoped records, artifacts, journal, jobs and backup exist. Migrations end at `0011_exact_identifiers.sql`; catalog numbering must exceed every landed/reserved number on main, S1/S2/S3 and the deployment-modes track |
| Filesystem | `crates/maestro-canonicalization/src/filesystem/{mod.rs,root.rs,unix.rs,windows.rs}` owns ADR-0018 held handles and no-follow opens; C04a moves it, not copies it |
| Hosts and content | The draft recorded Pi 0.87.1, MCP adapter 2.37.0 and subagents 0.64.0; Copilot was absent from PATH and `maestro-manifests` did not exist. These are historical observations, not refreshed approvals or qualification |

The 08:12 owner decision starts S2/S3 while S1 finishes. C00 requires only
D1–D5 decided. Integrated T034/T035 and T038 live evidence gate C08 and C28;
the M1 release gates M3 exit, not fixture-based implementation. C01 records
actual pins/capabilities rather than assuming historical versions are approved.

## Constitution Check

The shared Spec Kit constitution delegates to the golden rules and this
repository's [rule map](../../docs/standards/engineering.md). No S3 exception.
The spec's rule-ID inventory and these holders are rechecked after C09 and at
C28. Named future tests are required evidence, not claims that they already pass.

| Rule IDs | S3 obligation | Held by |
| --- | --- | --- |
| C-001, C-005 | Maps before affected work; durable evidence | C00 inventory conventions suite/review; C02 creates MAN maps before content; C13/C15 update maps, C28 verifies holders and evidence |
| FND-002, FND-003, P-001 | Required content only; no engine | C02/C21/C21b scope review against 08 and C03 unused-resource refusal |
| P-004, P-005 | Reuse seams, adapters only for real variations | C04a pure-move review; S1 settings reuse in C05a/C05b/C05d/C17/C18; C03/C10/C11/C12/C16 descriptor-only extension proof; C03a kernel-card adapter tests; C05i/C05e/C16c port contract tests |
| P-011, P-013, SEC-003 | Typed bounded hostile inputs | C03 source, C05a preference, C11 archive and C13 verifier boundary tests, including every D2 limit |
| P-012, P-014 | Scoped, least-privilege admission | C14 shared admission tests; C16b installed-consumer refusal; C09 credential measurement and C28 OA4 review |
| ENF-001, SEC-001 | Derived paths and synthetic public material | Conventions personal-path test and C08/C28 privacy review; live private receipts stay outside CORE |
| ENF-003 | English artifacts/logs | C05c/C05l interface contract, C05j invariance and C16d receipt tests |
| ENF-002 | Every claimed platform tested | Every task's three-target Clippy; C28 Linux/macOS/Windows CI test evidence |
| ENF-005, ENF-006 | Red first, no weakened gate | Each task's retained Red output; supervisor review; C28 unchanged coverage/mutation bars |
| ENF-008, ENF-012 | Layered gates and pinned inputs | Signed commit hooks, architecture/duplication/licence gates; C09 pins, C15 workflow checks, C23 digest checks, C28 CI |
| ENF-009 | No stale exclusions/exceptions | C04a pure-renames the 17 exclusions at `.cargo/mutants.toml:10, 12-25, 29, 32`, leaves :62 and deletes both filesystem ARC-005 exceptions because the new crate root no longer triggers them; conventions exclusion checks and passing architecture/DEP-001 gates |
| ENF-011, SEC-002 | Input grants nothing, content never executes | C05/C10 script-marker tests; C19/C20 refusal of model-supplied authority |
| SEC-004, SEC-005 | Trusted actor and scoped human approval | C05h/C13a explicit journal approval tests; C19/C20 actor/approval refusals; C16f exact-change consent and C15 protected-workflow tests |
| ENF-013 | No secrets in history | gitleaks commit/CI gate and C28 public-evidence review |
| SEC-006 | Pinned bounded subprocess | C13 per-launch digest/substitution and time/output tests; C13 updates CORE SEC-006 map |
| SEC-008 | Truthful states | C18 declared/effective/observed tests; C08/C28 live-receipt review distinguishes missing evidence |
| SEC-011 | Attestations, checksums, SPDX JSON SBOM | C15 generated-SBOM/asset-closure and workflow refusal tests; C28 verifies actual release assets/instructions; both maps mark applicable |
| ADR-0005 | Probe decides metadata per kind | C01 real host-format evidence; C03 depends on its report, with no default |
| ADR-0007 | Real Cedar, no substitute | C19 evaluator allow/deny neighbours and zero executor calls |
| ADR-0015 | Freshness, revocation, floors | C14 expiry/replay/clock tests; C16b restore refusal; C28 rotation/withdrawal drill |
| ADR-0018 | Shared held-handle filesystem | C04a unchanged regressions; C04/C05i/C05j race/link/escape tests on three platforms |
| ADR-0020 | Measured approved dependencies | C09/C05f measurements; adoption-time DEP-001 and vet gates in C10/C19/C22b/C05k |

Rechecked for the supervisor's 2026-09-28 analyze ruling. A dependency
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
The frozen inventory is approved by the owner, 2026-09-28 (C00 inventory
approval); M3 acceptance remains pending.

### Source code (planned)

```text
crates/maestro-catalog/
├── Cargo.toml
└── src/
    ├── source/                # kind descriptors, bounded source types and checks
    ├── model_cards/           # declaration validation and existing kernel registry adapter
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

In `maestro-manifests`, [D13](#d13-manifest-v4-source-contract) is the final
common/core/team layout with root standards and languages. Each area has one
`package.toml`; common owns shared support roots, core owns framework personas,
workflows/backends/hosts, and teams reference global/language content. Use one
pending `/2` cutover before publication, never an intermediate tree. Only
registered required content creates directories; no private corpus is seeded.

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
cannot make a source tree appear attested. Authoring projection reads the source
identity/digests selected by C05's authoring lock, rechecks those bytes and labels
this mode explicitly; installed projection instead enters C14 admission.
Workflow declarations and executable assets remain inert.

The normal M3 path consumes only verified installed bundles and passes trust
admission before init, projection or catalog consultation. Explicit resource
selection works before routing; no code depends on an engine merely to install
or remove an agent profile.

C00 records the contract in architecture 03/06/08: graph checks needed to safely
compile S3 bundles belong to C22a/C22b; S4 still owns general runtime execution.
The frozen exact row-key inventory and S3/remaining portions, including
GD4's four-client MCP scope, were approved on 2026-09-28. The v4 amendment
moves C20's live Copilot hook and 4 h to S4 with the other host adapters. Do not
quietly implement a smaller graph language. A supported construct passes all applicable checks;
unsupported means not executable.

### D2 Trust, publication and installation

Approved: pinned `gh`, separate catalog/runtime publishers, five-minute refresh
and at most 24-hour offline validity. C09 first demonstrates actual verification
of a real public attested artifact and rejection of a wrong signer. Existing
public evidence suffices to develop the verifier: no catalog publisher setup
or release is required to start C09/C13. Freeze the verifier version/digest,
expected issuer/repository/workflow bindings and authenticated record formats.
C13 persists the separate catalog/runtime root bindings and approved executable
path/version/SHA-256 pin as scoped kernel authority records. C13a supplies the
local CLI adapter, `maestro catalog authority set`, with the exact flags below.
It requires all six catalog/runtime repository, workflow and issuer bindings,
a canonical absolute gh path and the executable's SHA-256. Hash before bounded
`gh --version` execution; record the measured version, never discover via PATH.
Display the complete proposal and existing authority revision before approval.
A terminal asks default-no; without one, require `--confirm SHA256` matching
the displayed digest of the canonical JSON proposal (all bindings, canonical
path, measured version, executable digest and current authority revision).
Missing confirmation returns status 2 with the exact command; a changed
proposal/revision requires confirmation again. Atomically journal approval and
write roots/pin; rotation invalidates cached admissions. Use user-local
kernel authority directly, never workspace discovery, `--yes`, environment
approval or MCP. C20/S4 denies agent-shell authority administration, even with a correct
confirmation digest; S3 claims no host-process containment.
Only that explicit authority operation may create or replace them; catalog
releases, updates, model input, locks and all preference layers cannot. These
are Locked authority, not Locked catalog-editable settings. Tests use synthetic
provisioned records, never a permissive default root. C09 owns closure of
architecture 08 §17's "Publisher identities, trust roots, key rotation procedure"
row: record D2 and the exact OA4 bindings/evidence when
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

Online refresh uses OA4's repository-bound read-only fine-grained token through
`GH_TOKEN` or an equivalently minimal approved login, never default broad login
scopes. C09 measures the contents/attestations/metadata read permissions actually
needed; no write/admin permission is accepted as a requirement. A local
unauthenticated probe of `gh` 2.98.0 exited 4 before verification, requesting authentication;
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

Every install/update and installed-bundle init/resolve/search/route/explain/
impact/project consult enters the same admission function, binding caller scopes,
compatible runtime, exact snapshot and current trust. A fresh online check is attempted when the cached
records reach the five-minute refresh interval; an unavailable network permits
only the authenticated offline window, ending at the earlier of signed expiry
and `issued_at + 24 hours`. Cap a longer signed expiry; download/verification
time never renews it. Expiry is enforced at the boundary,
not on a best-effort timer. Refuse clock rollback below durable observations,
record replay and version-floor regression. Recheck the trust revision before
returning a consult whose admission raced with a local revocation update.
There is no promise to learn remote revocation before the bounded refresh.

The verifier uses the kernel-authorized executable path and fixed argv, never
PATH search or a shell command from content. Reuse the existing pinned-launch
safety pattern: verify that executable's digest before **every** launch, even
after a successful doctor check; a substituted executable refuses before spawn.
Bound downloads, verifier stdout, stderr and wall time; timeout kills and reaps
it. Check the attestation's subject against the locally hashed artifact, expected
source commit, repository,
workflow and issuer, not merely `gh`'s exit code or a supplied checksum file.

#### Security limits

C03 defines one immutable `Limits` value in `maestro-catalog/src/limits.rs`;
source/preferences, compiler/reader, downloads and verifier consume it. Production
entry points always supply the constants below, never a user/catalog/CLI override.
Tests inject small limits into the same code paths and check each exact boundary
and one byte/entry/level beyond; assert all production constants once in C03.
C11 retains one streamed full-size archive-boundary case, generated through a
fixed-size buffer without an input-sized allocation. Do not repeat 256 MiB
fixtures across download/compiler/verifier tests or allocate a giant buffer.
Clock-controlled tests cover each deadline and one tick later without sleeping.
A larger production need requires a reviewed contract change. MiB = 1,048,576 bytes.

| Input | Limit | Owner and check |
| --- | --- | --- |
| Source/preference file | Source 1 MiB per file; preferences 64 KiB (S1); 32 container levels (root = 1), 4,096 resources per catalog | C03 source and S1/C05a preference checks refuse before allocation; aggregate walk/asset/mount limits never reset per package |
| Archive | 16 MiB per entry, 256 MiB total stream and aggregate entry payload, 4,096 entries including `bundle.json`; manifest nesting at most 32 container levels | C10 `bundle/write.rs` and C11 `bundle/read.rs` share `Limits`; count/size/nesting neighbours, declared-size checks before allocation |
| Download | 256 MiB per artifact, 60 s total wall time including redirects/retries | C16 `install/download.rs`; streaming count even without or with false Content-Length, exact/over-limit and deadline tests |
| gh verifier | 30 s wall time, 16 MiB stdout, 1 MiB stderr | C13 `trust/verifier.rs`; independent output-overflow tests, kill/reap at deadline; refuse completion at or after it |
| Startup discovery | 5 s total, within the same byte caps | C16f applies the narrower deadline across its entire attempt; no retry resets it |

The source resource cap is only a parsing bound, not a promise that every
source set can fit an archive: an agent and its sidecar count as two archive
entries. C10 refuses to publish a bundle exceeding any C11 entry/count/payload/
stream/nesting limit, including manifest bytes, tar headers/padding/end blocks.
A source-valid but archive-too-large closure must fail compilation, never
produce an unreadable release. The writer and reader use the same `Limits`.

Compile sorted tar entries with fixed archive metadata and canonical bundle
JSON; no compression is needed. The reader rejects extra/duplicate entries,
links, traversal, device entries, oversized bodies and truncation. Verify every
entry and compatibility before activation. Artifacts may be staged, but no
partial install or discovery generation becomes current. Each protected catalog
release includes an SPDX 2.3 JSON SBOM: exact pinned component IDs/versions and
SHA-256 digests plus dependency relationships derived from the checked closure,
not a list reconstructed from names or latest versions. C15 uses the existing
checksum-pinned toolbelt **jaq 3.1.1**, verified against the per-platform SHA-256
in the toolbelt's checked `mise.lock` (no new library or invented digest).
Set `creationInfo.created` to the source commit's UTC time in
`YYYY-MM-DDTHH:MM:SSZ` form, not the build time; derive `documentNamespace` as
`https://spdx.org/spdxdocs/maestro-catalog-<bundle-sha256>` using the lowercase
bundle digest. Sort keys and component/relationship arrays with fixed UTF-8/LF
serialization. Identical bundle/source inputs therefore yield identical SBOM
bytes. Check its package/relationship/digest sets against `bundle.json` and
publish it with the bundle under the release-assets contract below. `SHA256SUMS`
covers both payload assets; attestation binds both names/digests. Release
instructions explain checksum, attestation and closure verification; C28 repeats
those checks on actual release assets. Switch install records, component
references and pins transactionally; retain the old valid
install after failure. The project lock changes only on an explicit update.

Restore preserves the destination's current catalog/runtime roots, gh pin and
matching authority approval receipt; backup copies are historical evidence,
never replacements. A fresh destination without these records refuses catalog
use until C13a explicitly reprovisions them. Restored authenticated trust state
is unready until refreshed against those current roots, not a rotated-out root
from the backup. C16b tests a pre-rotation backup against newer authority and
an unprovisioned destination. An older backup cannot lower the effective floor
or resurrect a revocation by presenting its own signed-but-stale records. Revocation stops new
loads; it cannot retract text already in a native session. The guide names that
limit and the restart/removal action.

### D3 Synthetic data first

Approved: all committed fixtures and public CI are synthetic or public. Generic
`workload-question` instructions describe how to use knowledge tools; they contain no
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
and `cedar-policy` 4.13 support. ADR-0005 remains unchanged: C01's integrated
[host report](research/hosts.md) at `0be954b` is the format evidence.
**Agents use `<stem>.maestro.toml` sidecars.** Each catalog agent's
`<stem>.agent.md` must declare `name: <stem>`, pairing its sidecar with exactly
one agent. Copilot CLI 1.0.88 ignores agent metadata with
`unknown field ignored: metadata`; sidecar agents load without warning.
**Skills use `metadata:` under the [Agent Skills specification](https://agentskills.io/specification).**
C01's unknown-key control also loads silently; neither host's silence proves
metadata support. This outcome rests on the specification, not inferred parser
validation. A host warning on skill metadata reopens the ADR-0005 sidecar
decision; no silent field dropping or automatic sidecar default. C03 consumes
this integrated outcome.
C01 records exact host version/digest, effective tools, discovery order, reload
behaviour and supported fields. Reload inside a running session is **not run**,
an explicit C06 input, not a claimed supported capability. Missing format
evidence blocks C01/C03, not independent C04a/C09 work. Later CORE fixture code
has no MAN content/publication dependency;
C08/C28 own live exits. A changed host pin needs the probe again.

C01 supplies these C06–C08 inputs: Copilot silently lets user agents shadow
project agents, while Pi silently does the reverse; both adapters must detect
same-name collisions themselves, using declared names rather than filenames.
A renamed user agent file with the same `name:` still wins in Copilot; the
catalog's stem/name invariant cannot be assumed for unmanaged host files.
Claude Code project `.mcp.json` needs interactive approval; local-scope
registration connects. Codex evidence is registration only, with its live call
not run; neither C08 nor C28 may relabel that as a successful call.

Copilot output uses native agent/skill/instruction formats. Pi maps only
supported fields, explicit MCP tools and their installed provider; child tool
names without the adapter are not sufficient. Do not install a new extension
or rely on model inheritance/fallback. Unsupported fields are diagnosed rather
than dropped. Only **projectable** resources (reviewed evidence and a supported
native mapping) are projected. A **route-eligible** executable closure also needs S4 qualification
for every member; convenience projection does not grant it. S3 checks ten-point subscription descriptors and explicit host mapping states
only. All live hooks, including Copilot C20, are S4 host-adapter qualification.
Four-client MCP registration remains S3.

C09 records crates added, duplicate versions, native links, feature choices,
licences and vet requirements against the current lock. D4 already approves
these libraries: the supervisor verifies measured features, DEP-001 exceptions
and selection of the JSON Schema validator under ADR-0020, without re-asking
for that approval. Any unlisted licence still needs the authorized maintainer's
organization allowlist action, recorded under OA4. No worker changes that
setting. A hand-written schema or Cedar substitute is not the minimal solution.

### D5 Baseline routing and measured hybrid

**OA10 approved (owner, 2026-09-28):** the absolute routing bar is
**held-out matchable top-1 ≥ 90 %**. This dated amendment to D5 replaces its
original "top-3 accuracy at least 90 %" bar, retained here as history, not
current acceptance. Top-1 measures the first selection rather than shortlist
inclusion. Baseline first and hybrid only when it demonstrates gain remain
unchanged. Compute/report both metrics; top-3 is diagnostic.
C23 owns CORE's public/synthetic fixture, not MAN content:
`tests/fixtures/catalog/routing/` contains reviewed labels, split, eligibility,
checked source workflows and their C10-compiled bundle. `digests.json` pins every
input including the compiled bytes and profiles. CORE CI needs no MAN checkout,
network fetch or publisher. C23 records the exact compile command and validates
the source/closure before freezing; never copy private questions or live grants.

Freeze at least 100 independently reviewed intents: at least 20 tuning and 80
held-out, including at least 60 held-out matchable cases. Record exact split and
label-cohort sizes and at least ten route-eligible synthetic workflow candidates,
including distractors, before running any router. Both splits include no-match,
clarification and adversarial cases; IDs never overlap. Labels name accepted
alternatives. Test grants exist only in the fixture harness, not installed
consumers. Never tune on a failed held-out comparison.

For the held-out matchable cohort M, top-1 accuracy is the number of cases
whose first returned workflow ID is an accepted alternative, divided by |M|.
A refusal/empty result for a matchable case counts as incorrect. Top-1 must be
at least 90 % (OA10 approved by the owner, 2026-09-28, amending D5).
Report top-3 separately: cases whose first three returned workflow IDs
intersect the accepted alternatives, divided by the same |M|. Top-3 cannot
satisfy the amended acceptance bar. Correct no-match and clarification are
each correct typed statuses divided by their own labelled
cohort size. Report each denominator; an empty required cohort refuses scoring.
For each returned candidate, the exact closure includes the workflow itself
and every mandatory resource, so it is nonempty. Dependency completeness is
required closure IDs returned divided by all required closure IDs (100 % required);
unnecessary context is returned IDs outside that exact closure divided by all
returned IDs (0 for an empty result). Report counts, per-candidate completeness
and macro-average unnecessary context; never use context size to omit a required
low-similarity reviewer. Record route p50/p95 latency and distractor results.

Hybrid qualification uses paired per-case top-1 correctness differences on the
same held-out M. Reuse S1's seeded bootstrap method with 2,000 resamples, seed
42 and the 2.5th/97.5th nearest-rank percentile interval (95 %). C26 exposes a
paired-difference entry point from `maestro-knowledge/src/eval/bootstrap.rs`
through `eval/mod.rs`, reusing the existing generator/resampling/percentile
code without changing S1's default or reported results. Enable hybrid only if
the interval's lower bound is strictly above zero, held-out matchable top-1
is at least 90 % (OA10 approved by the owner, 2026-09-28), and exact closure
completeness is 100 %. Record seed, resamples,
interval, cohort/input/profile digests and both scores. Tuning cases never enter
this verdict; all failed comparisons remain.

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

Discovery cards (not model-card identities) use a separate scoped catalog
collection and verified generation per bundle. A stale/missing index triggers explicit lexical fallback only when the
bundle is still authorized and valid. Every cache includes visibility,
snapshot, trust/policy revision, runtime constraints and retrieval profile.
Use the protocol above for workflow IDs, not S1 document-section labels; keep
missing-route markers visible. Architecture 03/06/08 records the conditional
hybrid rule and held-out matchable top-1 ≥ 90 % gate (OA10 approved by the
owner, 2026-09-28), with D5's original top-3 bar retained as history. A real M3
install returns `incompatible` ("not qualified until S4") for executable workflows. C28 and
`docs/how-to/catalog.md` show both outcomes; synthetic receipts never authorize
a live role. C24a resolve/search and C24 routing depend on fixture-based trust,
locks and graph contracts, not C15/C16 or publisher/release setup.

### D6 Owned writes and project bootstrap

C04a first moves `maestro-canonicalization/src/filesystem` and its existing
tests into the minimal `maestro-filesystem` crate, preserving behavior and
security assertions. Change only module imports, visibility and workspace
wiring. Pure-rename the 17 exclusions at `.cargo/mutants.toml:10, 12-25, 29, 32`,
preserving mutant identities, reasons and scope; leave :62, the kernel's
`filesystem.rs` exclusion, unchanged. Delete both `maestro-quality.toml`
ARC-005 exceptions for `Directory` and `open_nofollow`: the new crate root
`src/lib.rs` no longer triggers that `mod.rs`-only rule. Show the architecture
gate passing without stale exceptions. Add no external library or second
platform implementation.
Existing canonicalization callers and C04 use this one ADR-0018 home.

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
selected preset and core/Rust inventories, previews dotfiles too, then applies only
the authorized plan. Validate composed JSON and descriptor references before
publication. Report missing prerequisites, including shell availability on
Windows. `.maestro/project.toml` contains preset, lock, capabilities and context
files only. It cannot redefine policy, hooks or authentication.

#### Shared S1 settings implementation

The evening owner decision assigns the serde-able setting descriptors, strict
user/project parsing, four-layer resolver, `maestro config get/set/unset/list/explain`
and change journal to S1's `feat/s1-settings`. Its integrated public API is a
prerequisite of C05a, C05b, C05d, C17 and C18. Those tasks add catalog descriptors,
init planning, trust-boundary integration, answer presentation and lock/explanation
views over it, not another registry, parser, resolver or config command family.
S1 owns the shared module paths; the S3 file lists name only adapters/tests and
existing integration call sites. Bind them to the landed API before dispatch.
C05h trust and C05e session delivery remain S3 responsibilities above the registry.

**Supervisor prerequisite before C17:** synchronize the required landed S1
settings and kernel-card commits into S3 and record the resulting integration
head. The amendment base `fd39783` still carries S1 `dac543c`; D12's `d882fe5`
API/lookup citations describe the required newer behavior, not that older base.
Do not dispatch against an uncommitted settings tree or infer a sync from the
existence of a branch. Bind the five adapters to the actual integrated API.

**Supervisor ruling, 21:02: S1 registry names are canonical.** For example,
`ask.output_tokens` replaces the catalog's `max_output_tokens` spelling. C17
replaces C03's temporary `KNOWN_SETTINGS` list with S1 keys through C03's
known-settings port (like `KnownRows`), and adds only missing architecture 03
§1.6 catalog descriptors to S1. Preset validation, class completeness, config
commands and menus read that one registry; a new key cannot drift between them.
The naming ruling does not override the landed S3 behavior below.

The supervisor explicitly reconciled the potential file/discovery conflict in
favor of the landed S3 contract: `preferences.toml` separate from authority
`config.toml`, `.maestro/config.toml` within home/journal-trusted boundaries,
explicit flags > workspace > user > defaults, and MCP explicit `--workspace` or
user preferences only. No default-workspace key; retain the BCP 47 subset, tone
values and evaluation isolation below. A differing S1 landing blocks integration
of the affected adapter and needs a ruling, never silent schema migration.

#### Workspace preference schema and ownership

C05a extends S1's schema with init planning for `.maestro/config.toml` at the explicit init workspace root (the current
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
- `[overrides]`: parameter keys come from the shared S1 setting descriptors,
  extended by C17, each with documented type, range, class and permitted layers.
  Under the owner's 20:48 amendment, `model_profile` (`fast`, `balanced`, `deep`,
  bounded by role/provider qualification) and `routing_candidates` (integer 1–3,
  bounded by the existing ceiling) are examples, not a second allowlist limiting
  the every-setting editor. A stored profile is a preference, never qualification
  or permission to execute it. Reject unknown keys, locked/authority-only changes
  and writes to disallowed layers; no raw option map escapes C17. New descriptors
  require the documented contract and tests before becoming editable. All landed
  S3 narrowing, language/tone, discovery and authority rules remain unchanged.
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

C05b extends S1's discovery/resolution adapter with S3 trust integration and
regression coverage; it does not reimplement the walk or precedence.
Canonicalize the CLI working directory once. Inside home, discovery walks
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
**explicit flags > workspace file > user-level config > pinned manifest defaults**.
Only a flag actually present counts as explicit; clap defaults do not outrank
files. Presets may suggest choices in init's displayed draft, persisted only
after confirmation; they are not a fifth session layer. C17 resolves consent ceilings and budget-like bounds by intersection first;
free values then use precedence, additive checks accumulate, and locked values
cannot change. D11's journal trust is never a preference layer. `config explain`
reports the chosen file/key or explicit flag, class and effective value,
including absent layers, ignored widenings and skipped unsafe candidates.

Every CLI invocation loads this context before effects, except D11's trust
administration and D2's `catalog authority set`, which read user-local authority
directly so they can provision/repair trust without loading workspace files. In S3, MCP uses only explicit `--workspace DIR`
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

The owner's 20:48 addition makes this the shared **every-setting editor**:
`maestro init` opens it with the bootstrap draft; `maestro config` without
arguments opens it over S1's existing settings operations. C05g supplies the
shared flow and plain adapter, and C05k the ratatui renderer. Enumerate the S1
registry instead of a screen-specific key list: each entry shows its current
value, allowed values/range, one-line description and source layer. Adding a
setting descriptor makes it appear in both entry points/renderers with no
per-setting screen code. Language/tone remain prominent initial choices, not
the complete editable set. The five stages below organize that same registry.

Every authorized setting is editable at an allowed user/workspace layer through
S1 validation and change journalling. Init still writes workspace preferences
only; config uses S1's explicit layer selection. Display locked/authority-only
entries with their restriction and existing administration command, never raw
secret values or preference-based authority edits. A user-layer update setting
can expose Auto only under the existing consent contract; init cannot grant it.
Cancel/preview produces no setting writes. Inject a descriptor in tests and
observe its entry and permitted edit in both flows; denied neighbours must make
zero unauthorized writes. This adds a menu adapter, not another config API.

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
| 4. All settings | Registry-generated entries show current value, allowed values/ranges, one-line description and source layer. Init permits workspace-authorized edits only; user-only Auto is a ceiling, never enabled here. Locked/authority-only settings are visible with their restriction, not permission toggles |
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

C17 extends S1's shared setting descriptors with catalog classes and restrictions;
no catalog-private setting registry or resolver is added. After the supervisor's
S1-to-S3 sync, replace `KNOWN_SETTINGS` through C03's known-settings port and add
only missing 03 §1.6 descriptors under D6's 21:02 canonical-name ruling. A test
setting must become valid in presets, require exactly one class and appear in
the editor without another key list. C17 precedes init's configuration tasks.
Free values use
explicit flags > workspace file > user-level config > pinned manifest defaults.
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

Rule 10's organization ceilings come from the Bounded ranges in the shared S1
setting descriptors, not a catalog `settings/classes.toml` or the defaults above;
C22b refuses an exceeded ceiling. `settings/README.md` documents the registry; v4 `settings/defaults.toml` feeds
its single lowest defaults slot through C46, never a second key/type registry.

Cedar evaluates normalized requests against the real schema/policy set. Trusted
actor, allowed operation, target and approval facts come from the host adapter,
not the submitted text. S3's native checker never invents an approval: an
operation needing an unavailable trusted fact fails closed. Unknown tools,
opaque shell constructs and evaluator diagnostics deny. Every rule has an
allowed neighbour, a denied case and a spy proving zero executor calls on
denial. A native hook is defence in depth, explicitly unprotected when absent;
it is not a sandbox or S4's authoritative broker.

S3 `reviewed` means the declared stage plus a named resource owner on content
admitted through OA1's protected-branch CODEOWNERS review. C03 checks the stage
and nonempty owner; C15's protected publication provides the review assurance.
A local authoring check or synthetic fixture proves the declaration's shape,
not that a remote review happened. No new evidence field or fake review receipt
is implied. Every compiled closure member must meet this threshold; record/show
maturity and owner in bundle, lock, preview and explain. Placeholder, authored
and retired members refuse. S4 raises execution admission to `qualified`;
S3 never fabricates that evidence. C22a tests the two thresholds independently.

Split graph checking into topology (C22a) and contracts (C22b). Together they
cover all twelve architecture 03 §2.3 rules, including conditions with only the
specified comparisons/boolean/array predicates, exact router choices,
reviewer independence, bounded maps and subgraphs, policy/sandbox requirements,
budgets, state flow and outputs on all successful paths. No condition executes
code, no graph runs and no qualification card is fabricated.

C21 adds team `workload-question` and standard-owned policy checks; C21b adds
core `feature-delivery` with planner/worker/tester/reviewer and their contracts.
Profiles for `copilot` and `llamacpp` may be authored/reviewed without being
live-qualified; routing excludes unsupported combinations. Synthetic
eligibility records test the compiler/router, never qualify live execution.

### D8 Impact and approved recovery

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

C29 audits the owner-authorized legacy inventory before M3, after C68 and
selected Phase 1 recovery/imports. Every input gets provenance and one
keep/convert/drop/defer disposition. Minimal deferrals name their Phase 2 task,
not a false recovery claim. Retained rule/approval obligations cannot weaken
current canonical standards; no private collection access follows.

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
tests. Shared setting types/parsing/resolution belong to S1's registry; catalog
adapters, instruction types and the delivery port live in maestro-catalog.
The binary supplies the MCP adapter, so catalog never depends on CLI/MCP types
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
modify trust records. C20/S4
denies agent-shell invocations of trust administration, including a correctly spelled `--confirm-path`; the explicit
command is not process-origin authentication. All host agent-shell containment remains the named S4 hook proof. S3 cannot distinguish arbitrary
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
C16 and update/rollback reuse it. C20/S4 adds the same check to Copilot preToolUse,
with zero effects on denied writes/reads and fail-closed normalization/errors.
**S4 workspace-trust hook obligation:** qualify Copilot (transferred C20,
4 h), Pi, Codex and Claude Code's trusted event/identity adapters against outside-write, secret-read,
symlink-escape and agent-shell trust-add cases (including `--confirm-path`),
each with an allowed neighbour and
zero executor calls on denial. Record it beside the already-deferred S4 hooks
in 06/08; configuration or instructions alone are not containment.

### D12 Kind extensibility and model cards

**Descriptor contract.** C03 supplies serde-able built-in descriptors behind a
loader seam. A descriptor names the kind/schema version, source location/format,
strict field shape, reference fields and declarative constraints. Check, compile,
read and install consume the same registry and common resource envelope; descriptor
registration is data, not a kind-specific branch. A built-in `Registration` pairs
a descriptor with an existing hook, named in the descriptor. `register` resolves
that name against the fixed hook table and refuses unknown names; deserialized
descriptors retain the same semantic checks as the built-ins. A hook is isolated
and tested only for semantics that cannot be described as data. Untrusted content
cannot load code or register a hook. File-loaded descriptors and JSON as a resource
source format are later additions; C03/C03a use Markdown/frontmatter and TOML; C64 adds the existing contract
consumer's JSON shape without loading executable validators.

C03's review round supplies an Eq-safe finite-float value (non-finite TOML floats
refuse) and a structured-table field type. The whole bounded nested subtree reaches
`KindRules` in serde form; generic scalar-table validation must not reject or
flatten it first. The kernel adapter, not a copied catalog schema, checks the
identity's strict nested fields. C03 also validates descriptors and exposes each
resource's full file set, so later compilation need not re-derive kind layouts.

| A new kind may require | It must not require |
| --- | --- |
| One descriptor constant/registry entry with field/reference rules | Editing generic checker/compiler/reader/installer logic or adding kind switches |
| Positive/negative fixtures and resource owners/08/workflow references | A per-kind bundle format, storage table, migration or install command |
| Selecting an existing validator hook or native mapping | Duplicating validators, changing callers or executing source content |
| A separately reviewed hook/consumer for genuinely new semantics | Calling new runtime behavior a descriptor-only extension or granting authority |
| A runtime release containing the new built-in descriptor | Removing unknown-kind/version refusal or raising D2 limits implicitly |

C03 injects a synthetic glossary descriptor with a finite float and nested table,
plus a model-card-like descriptor whose versioned identity and f64 sampling fields
reach a named test hook whole; neither proof edits the generic checker. The
round-trip descriptor test retains named-hook refusals. C10/C11/C12/C16 carry the
glossary fixture through deterministic
compile, read, scoped persistence and install/resolve, including refusal when the
descriptor is absent. This is the measurable growth guarantee, not infinite input
size. Unsupported source directories continue to refuse until described. Existing
per-kind graph/host rules remain hooks, not exceptions to shared trust or bounds.

**One additional kind: `model-card`.** C03a adds its descriptor and a thin adapter;
C02a authors only owner-approved cards needed by the two workflows. The built-in
registration lives in `source/kinds/{mod.rs,builtin.rs,model_card.rs}`;
`source/registry.rs` remains generic. Human-authored owner-relative
`llm/models/<role>/<local-name>.toml` (seed: `core/llm/models/`) holds common resource
metadata (ID/owner/maturity/08/workflow references), a model-card `version` field,
and structured `identity`, deserialized as the kernel's exact `CardIdentity`.
Do not depend on `version` being a common metadata key. The catalog
resource digest covers that declaration; the model-card fingerprint is the
kernel's canonical `maestro-model-card/2` JSON digest, not a TOML hash.

Reuse the types at S1 `d882fe5`,
`crates/maestro-kernel/src/gateway/card_v2/types.rs:29–46`, without copying them:

| Identity group | Fields retained |
| --- | --- |
| Role and weights | `role`, logical `router_entry`; upstream ID/revision, source/licence URLs, GGUF digest/bytes/quantization, named adapter/draft/projector digests |
| Formats | Tokenizer digest/derivation, qualification digest, template digest or explicit absence, system/tool/document/query formats, embedding pooling/normalization |
| Invocation | Context/output limits, dimensions, complete sampling/seed, reasoning controls, llama.cpp build, runtime binary digest, backend and typed resolved flags |
| Resources | Offload/devices, memory estimate/provenance, slots, KV/cache policy, batch/micro-batch, reference hardware and qualified limits with measured/unavailable states |
| Provenance | Identity/qualification dates, method, exact tool versions and supporting artifact digests |

The intended jobs are embedder, reranker, answerer, extractor and query_expander.
S1 `d882fe5` supports only the first three (`gateway/card_types.rs:13–24`). S2 G17
owns extractor support. S1's named post-M1 query-expander-role follow-up owns that
role; the current intent port uses an answerer card and is not an alias S3 may
register as query_expander. Unsupported roles use the kernel's own unknown-role
refusal during checking; C11 read and C16 install repeat that refusal before a
registration command can run. Do not hard-code a table of planned roles, ship
placeholder cards or extend kernel roles here. S2 joins S3 before M3 for G27,
including G17's extractor role; only query_expander remains unsupported at M3.
This does not block cards for supported jobs. V1 reads retain their exact
historical identity; new registration remains v2-only.

Expose a pure kernel-owned `ModelCard::from_identity(&CardIdentity)` constructor
if no equivalent exists at landing, sharing validation/canonical serialization
with `record_v2`; this is an explicitly named edit to S1's kernel, not catalog
code. Never write artifacts during a catalog check. Reuse
`Database::record_model_card` and `NewModelCard` (`model/write.rs:33–108` and
`model/records.rs:22–29`) for explicit scoped registration. The adapter requires
all artifacts referenced by the identity already present in local kernel storage,
including qualification, template, external tokenizer, provenance and non-weight
flag assets. Rely on `record_model_card`'s transactional pin refusal for absent
artifacts and the existing artifact-store integrity checks, not a new public
`artifact_digests()` query or catalog copy of its logic. Registration succeeds
only where this evidence is already local; the catalog does not import it.

Each machine qualifies its own card: backend, runtime-binary digest and reference
hardware belong to the identity. A declaration from the qualifying machine is
not portable qualification for another laptop. **Named post-M1 follow-up:**
explicit local `--evidence DIR` import with digest-matched files; no downloads
or automatic registration. M3 retains the local-evidence-only limit. C02a compares
canonical fingerprints with the owner-supplied card JSON, not a nonexistent kernel
export command. No machine paths, credential literals or credential-bearing URLs
may enter declarations, flags or provenance.
Router entries obey the existing logical-name grammar; paths/credentials resolve
through runtime bindings. A declared file fingerprint is not proof that a router
alias served those weights; preserve the kernel's declared/observed distinction.

C16h exposes `maestro catalog register-model-card ID --collection COLLECTION`
after C16, C18 and C03a. It owns the exact lock-bound resource/closure lookup over
C18's lock and C14 admission, plus the CLI and its suite; C24a reuses that lookup.
Only an installed, currently admitted snapshot reaches C03a's adapter. No
authoring-source or MCP registration mode. Check, compile, install and update
never call it implicitly; generic installation only stores
resources. Return the declaration/version/bundle identity and kernel card ID/digest.
Re-registering the same card is a no-op. Refusal leaves registrations/selections
unchanged; no model download, load, router mutation or evaluation/selection-record
write occurs.

A bake-off winner is an owner-approved manifest change under ADR-0011, including
the exact card and evidence references, not a mutable built-in model default.
Registration fabricates no evaluation or selection record: the existing kernel
selection API still requires an eligible real evaluation of that exact
card/role/collection (`model/write.rs:160–190`). **S1 compatibility caveat:**
`crates/maestro/src/knowledge/operations/ask/run.rs:100–122` at `d882fe5` chooses
the latest registered answerer for the requested router entry. Explicitly
registering a different card for that entry can therefore change a later ask;
zero selection-record writes does not mean future resolution is unchanged.
Re-registering an earlier card does not restore it: a known card adds no new
registry row. C16h tests register A, register B, register A again and expects B.
Mark that test as a record of the known gap in S1's post-M1 queue, so the explicit
selection fix changes it deliberately, not as a regression to preserve forever.
S1's named post-M1 explicit-answerer-selection follow-up will move answerer lookup
to selection records, as for rerankers; S3 does not implement that change.
A reviewed resource's maturity is not model
qualification or S4 agent qualification. Private evaluation content stays private;
only approved public identities/digests go into MAN. Later source edits, catalog
removal and updates cannot rewrite/delete registered cards. Each answer carries
its registry card ID, which resolves to an immutable card; the kernel stores no
answers or per-answer journal. C16h retains a returned ID in the test and resolves
its unchanged card after replacement/removal; it does not query nonexistent answer
history. C18 explains the card an ask would use now, not an observed answer ID.
Per-answer storage would be a separate S1 follow-up, not part of this amendment.

**M059 identity is unchanged.** Owner-relative `profiles/models/<agent-role>.toml`
(core for framework roles, team areas for their own roles) and bounded
`model_profile = fast|balanced|deep` select allowed agent-session configurations,
including provider controls and later S4 qualification. They do not choose the
kernel's embedder/reranker/answerer/extractor/expander. A local agent profile may
reference a compatible exact model-card identity, but no automatic conversion,
selection or new profile schema is needed for this amendment. C18 explains both
identities without merging their authority; changing the preference leaves kernel
selections and the immutable cards named by previously returned answers unchanged.

**Later kinds (owner, 20:50).** After the glossary spike shows value, add glossary
and source-class-table descriptors with reviewed, versioned per-collection
content in the manifest, not only a kernel binding. Finite weights and nested
entries use C03's generic fields. Consumers reuse their existing validation ports;
any genuinely new semantic hook is reviewed separately under this contract.
The synthetic glossary proves extensibility only; neither production kind,
its runtime binding nor a new schema is in the M3 estimate.

### D13 Manifest v4 source contract

**Approved 2026-09-30; target contract, not delivered behavior.** The detailed
rationale is the approved `manifest-design-final.md` §§2–7; gap obligations are
`manifest-gap-analysis.md` §§3 and 7. [ADR-0022](../../docs/adr/0022-manifest-layout-v4-and-language-neutral-extensions.md)
records durable choices. These public sections and task acceptances are the
implementation contract; the design's earlier proposed/full alternative is not
an additional budget. C44 records the **minimal** choice: approved Phase 1/M3
149 h (including the later URL-rule +1 h), plus **1 h for C02's validation fix**
(Rust profile/reference, checked MCP base and refusal cases) = **150 h**.
Phase 2 immediately after M3 remains 121 h. The retained 7 h S2 amendment is
included in Phase 1; all other validation fixes add 0 h.

#### Tree and ownership

```text
maestro-manifests/
├── package.toml                         # package:common
├── standards/<domain>/                  # standard:<domain>, always selected
│   ├── package.toml
│   └── {instructions,policies,checks,exceptions}/
├── languages/<language>/                # language:<language>, reusable
│   ├── package.toml
│   └── {profiles/quality,instructions,bootstrap,contracts,evals}/
├── {skills,knowledge/sources,profiles,prompts,contracts,evals,hooks}/
├── bootstrap/<inventory>/files/         # common inert outputs; inventory TOML
├── settings/{defaults.toml,README.md}    # one S1 lowest defaults slot
├── core/                                # package:core
│   ├── package.toml
│   ├── {agents,workflows,handoffs,contracts}/
│   ├── backends/{graphdb,vectordb,mcp}/config.toml
│   ├── hosts/{pi,claude-code,copilot}/config.toml
│   ├── extensions/<name>/extension.toml  # Phase 2/X1; otherwise unsupported
│   └── llm/models/<role>/<name>.toml     # router/ only after M1
├── capabilities/<group>/<name>/
│   ├── package.toml
│   └── <registered-family>/             # team-owned content, not copies
├── presets/<name>.toml
├── marketplace/index.json               # generated, never trust authority
├── templates/{package,<kind>}/          # Phase 2/A0 authoring kit
├── schemas/source-2/                    # generated from pinned checker
├── fixtures/{valid,invalid}/<kind>/      # authored, with exact diagnostics
├── docs/{catalog,standards,governance,setup,examples,releases}/
└── .github/{CODEOWNERS,workflows}/
```

Instantiate only registered content; braces abbreviate siblings. Core and team
areas can use common families plus agents/workflows/handoffs/contracts/backends/
extensions/model cards. Common has root prompt contracts, but no agents and no
root instructions/policies. Universal normative instructions/policies belong to
standards. The seven discovery groups (architecture, orchestration,
observability, practice, security, operations, governance) are extensible data,
not inherited authority or 29 empty leaves. Asset inventories are explicit,
owner-local files, never recursive includes.

All areas use `package.toml`: kind, name, exact SemVer version, description,
nonempty owners, optional maintainers (empty by default), maturity, rows,
requires and active/deprecated/retired status. Language and standard areas have
one identity each, not package aliases. Resource metadata names its owning area;
ownership derives from that descriptor, never a second editable owner list.
Owners control descriptors, delegation and exceptions; maintainers review
content. Generate anchored `.github/CODEOWNERS`: broad owner+maintainer content
rules first, owners-only descriptor/exception rules last. Root owners protect
CI/generator/ownership policy/CODEOWNERS itself. Standard exceptions additionally
need central root-owner approval. Groups have no inherited approval authority.

Offline validation checks syntax/duplicates/placement. Trusted CI uses read-only
GitHub identity/team lookups and base-revision owners' approval of the exact head;
missing access, unknown identities and self-added approvers block release.
Never expose credentials to PR code. CODEOWNERS routes review; protected CI and
rulesets enforce required quorums/separation, not a generated-file claim.

#### Identity, naming and one cutover

Use `kind:namespace/local-name`; area roots are `package:common`, `package:core`,
`package:<name>`, `language:<name>`, `standard:<name>`, and presets `preset:<name>`.
All area namespaces are globally unique. Resource names are unique within
`(kind, namespace)`, including across hook-point/model-role folders of that kind.
An agent and profile may share a stem. Segments retain the lowercase-hyphen
64-character grammar. Moving a package's group preserves identity but changes
the source-path lock and requires a new preview.

Functional names are required in paths/files/IDs. Products are values, including
`type = "ladybug"`, `type = "qdrant"` and `type = "gerrit"`. One checker-owned
exception table permits only:

| Exception category | Exact limit |
| --- | --- |
| Approved hosts | `core/hosts/pi/`, `core/hosts/claude-code/`, `core/hosts/copilot/`; no product-named package/ID exception |
| Required native filenames | Registered exact relative placements of `AGENTS.md`, `CLAUDE.md`, `SKILL.md`, `.github/copilot-instructions.md` and tool-required outputs; no filename wildcard exemption |

Known products/aliases come from the adapter/host registry. New spellings require
reviewed registry coverage; the checker cannot identify all trademarks from
English. Packages cannot exempt themselves; exported schemas use the same table.

C30–C39 revise the still-unpublished `/2` contract once, not through a temporary
layout. Refuse `/1`, mixed layouts, `capability.toml`, `capability:` IDs and old
locks with migration/fresh-preview diagnostics. Increment changed descriptors;
check output, project and authoring lock remain their planned `/2` contracts.
Kernel card fingerprints, S1 preference schemas and C04 owned-file records stay
unchanged. Retired engine resources, backend capability packages, backend-only
presets, `mcp/` resource files/edges and flat hook JSON are not alternative inputs.

#### Formats and registered consumers

All structured shapes refuse duplicate/unknown keys, undeclared assets and
unsupported versions. Reads remain bounded: 1 MiB source, 32 container levels,
4,096 resources; preferences retain S1's stricter 64 KiB. Walk/byte ceilings
aggregate across folders/assets/mounted sources and cannot reset per package.
D2 archive limits remain unchanged. Registered native formats use sidecars where
necessary; no file-loaded executable validator is admitted.

| Shape | Required contract beyond common metadata |
| --- | --- |
| Agent/skill/instruction | Existing native fields and fixed agent sections; specification-backed skill metadata and explicit inert assets; instruction applicability plus sidecar |
| Prompt/handoff/contract | Bounded nonempty prompt plus input/output contract IDs; sender/recipient and Inputs/Context/Deliverables/Acceptance sections for handoff; JSON Schema 2020-12 typed object, local `$id` and declared digest-locked local references only; JSON/Cedar metadata sidecars |
| Workflow | Existing typed graph, entry/nodes/edges, roles/steps, conditions, bounded maps/loops, budgets/state/outputs; all twelve C22 checks |
| Policy/check/exception | Real Cedar/shared schema; stable rule IDs and allow/deny neighbours; check names registered validator/applicability/typed inputs/evidence/refusals, never an executable; exception names rule/scope/rationale/expiry/evidence |
| Language/quality profile | Language/technology value, required profile/instruction/starter references; gates, applicability, failure conditions, evidence formats, thresholds and manager/tool default/alternatives; missing required bindings unresolved |
| Session profile/model card | Existing fast/balanced/deep provider/role settings and qualification states, separate from quality; exact kernel card identity/version at `llm/models/<role>/`, role matches path and kernel validator owns canonical bytes |
| Knowledge source | `knowledge/sources/<name>/source.toml` plus explicitly inventoried strict JSON URL policy, decisions/promotions/expiry and identity migrations; immutable pins/provenance/classification and local credential/storage references; signed-review rule admission below, never an ingested collection |
| Hook/host | `hooks/<point>/<name>.toml`: point, target/action/tool, timeout/budget/failure behavior; host config has type/version/adapter/native aliases and explicit evidence state for all ten points; implicit engine event protocol, not a resource dependency |
| Eval | Registered driver, subject IDs, explicit synthetic JSON inputs/cases, expected outputs/statuses, limits and required checks; fixtures now, generic driver execution Phase 2/E1 |
| Bootstrap/preset | Explicit area-local source/output/digest inventory with binding/tool requirements; preset exact package/language requirements and area/inventory selectors, mandatory standards added/pinned by closure |
| Defaults/backend | Typed canonical S1 defaults and registered base/extension shapes in D14; authority/secrets/duplicate default producers refuse |

C02's minimal Rust seed includes `languages/rust/profiles/quality/default.toml`
and the required `quality-profile:rust/default` reference in its `package.toml`,
with instruction/starter references, an exact seeded `standard:quality` baseline,
complete gate categories and honest unresolved runtime bindings. C76a extends
that same file/identity after C82a; C02 never waits for its own recovery consumer.
C21/C21b list paired `.maestro.toml` metadata for their JSON/Cedar contracts,
policies and supporting inputs explicitly; a native-format file alone is not a
complete resource. C85a supplies `capabilities/practice/project-creation/package.toml`
and its `requires` reference to `extension:project-creation/starter-renderer`,
rather than installing an extension without an owning area.

Every registered kind/config shape needs an authored valid and refusal fixture
with expected diagnostic and allowed neighbour. Registry inventory rejects
missing/zero/stale coverage. Schemas/editor associations are exported from the
pinned registry/settings/owning semantic hooks, not another handwritten validator.
Cedar, Markdown-body and cross-resource semantics remain checker-only where
necessary; editor success is not semantic or trust admission.

Ten hook points are session-start, session-end, prompt-submit, pre-tool,
post-tool, agent-stop, subagent-stop, pre-compact, post-compact, notification.
The engine/checker owns their versioned event protocols. Common prompt contracts
live in common `contracts/`; common → core still refuses. Guard evidence is an
ordinary core output contract, not a platform event protocol or extra hook point.

#### Manifest-owned source rules

**Owner decision, 2026-09-30 20:45:** URL rules and their definitions live in
the manifest, not per-site engine configuration or a collection-local allowlist.
C66 registers typed JSON payloads under each owning knowledge-source resource:

```text
knowledge/sources/<name>/
├── source.toml
├── policy.json                 # maestro-source-policy/1
├── decisions/<name>.json        # maestro-source-decisions/1; expiry included
├── promotions/<name>.json       # maestro-source-promotions/1; expiry included
└── migrations/<name>.json       # maestro-url-identity-migration/1
```

These are explicitly inventoried typed resources inside the source kind, not
unvalidated arbitrary assets. The tree shows permitted placements, not mandatory
empty records: preserve core optionality, but every referenced companion must
exist. Non-fetching source kinds gain no URL permission from absent rules. Retain existing wire IDs, versions and exact
original-byte digests; the catalog-qualified source/owning area binds their
inventory, without rewriting the core wire identity grammar. No implicit glob,
undeclared companion, collection-local `[approved_urls]` or duplicate rule copy.
The source carries its policy's referenced closure; unreachable/unpinned records
refuse. Use the shared strict JSON decoder and owning semantic validators,
never a TOML translation or a second policy parser (ADR-0014).

Reviewed admission combines **the signed, digest-pinned catalog release** with
**the owner and maintainer approvals recorded under the ownership rules**.
Protected base-code checks bind review to exact source/head/digests; preserve
that evidence through bundle/lock/install. A metadata `reviewed` label or a
policy's own authority field is not proof. Current expiry, revocation, visibility
and principal access still apply; manifest review does not grant credentials,
network access, filesystem trust or processing entitlement. Offline validation
can check shape and fixture evidence, not claim a real approval occurred.

Collections keep strict `collection.json` plus catalog metadata and exact
`source_policy` references; their own declaration is not a second URL-rule store.
C41/C43's deferred S6 contracts check those links. S6 alone supplies a
catalog-backed adapter behind existing `PolicySource` and `ResourceSource`,
resolving admitted original bytes and review evidence through the existing
strict validator. That runtime adapter is a separately assigned S6 dependency,
not an S3 task or M3 gate; maestro-core holds zero real per-site rules.

C52a/b export JSON Schemas from the core `schemars::JsonSchema` types into
`schemas/source-2/` and associate the companion files. C66 authors synthetic
valid/invalid neighbours for each family; C68 checks them at editor, pre-commit,
CI and install. Unknown/duplicate keys, proposal schema, stale/altered digest,
expired decision, contradictory references, fabricated approval or missing
fixture fail. Schema generation does not replace semantic or trust admission.

**Observed code and dependency:** S6 `a9c4f43` exposes the ports in
`crates/maestro-acquisition/src/ports.rs:63–85`; `policy/schema.rs:10–12` and
`policy/decisions.rs:135–180` derive the existing JSON contracts. N07
`746cc58`, `policy/identity.rs`, adds `IdentityMigration`; it is under review
with fixes pending, not landed or usable as a released pin. C66 synchronizes
that owning type only after N07 lands; do not copy it into catalog or claim a
migration fixture passed before then. This wire-type prerequisite is distinct
from the deferred live S6 adapter. Existing schema derivation is reused, with
no new library decision.

Private inventories, including the Control-M source, live only in C69-admitted
private manifest packages. Public catalogs/indexes/fixtures contain none of
those URLs or metadata. C42's restricted private collection mount remains as
narrow as before; it does not become a general source-rule overlay. Public
checks run without private access; affected private selection fails closed.

C66 changes from 3 to 4 h (**+1 h**); C52a/b, C68 and C41/C43 keep their
existing hours. This URL-rule decision made the approved amendment **149 h
Phase 1 / 121 h Phase 2**. C02's later validation fix adds 1 h for its minimal
Rust profile/reference, MCP base and refusal cases: **150 h / 121 h**, 271 h
combined. The S6 runtime adapter's separate budget is not guessed or charged here.

#### Resolution and mandatory standards

1. **Global:** all pinned standards, common defaults/resources and explicitly
   required languages. Standards/common cannot require core or optional packages.
2. **Framework:** core declarations/backends/hosts, requiring global resources
   only. Common/core and every admitted standard are mandatory exactly once.
3. **Selected packages:** explicit team/language closure in stable topological
   order. A language may require global resources/other languages, never a team
   package. Reject cycles and undeclared or unresolved edges.

Selection is a union, not last-file-wins. Duplicate full IDs/paths refuse even
for identical bytes; never merge bodies/policies/native settings by directory
order. All required common/core/standard resources enter every preset's checked
forward closure/lock. Adding a standard needs a reviewed signed catalog update,
not discovery of a local folder. Availability is not context injection: knowledge
projection includes Maestro, required instructions/skills and selected MCP tools,
not all ten personas. Maestro has one canonical core persona/system prompt.

All standards are mandatory and non-removable. Required checks/prohibitions
accumulate, ceilings take minima, floors maxima and permissions intersect;
conflicts refuse. Exceptions are central, exact-scope/revision, expiring and
owner-approved; non-negotiable rules have none. Consumers cannot add/expand an
exception, select weaker revisions or disable secret scanning. Runtime authority
is independent of content ownership.

Phase 1 C82 imports canonical organization golden rules/maps plus four
rust-workflows standards documents with exact revisions/digests, read-only and
drift-checked. Preserve rule IDs and distinguish normative rules from repository
"held here by" observations. Do not invent enforcement for prose. C83/ST1 freezes
source edits, changes manifests to the sole editable source, then renders pinned
organization/gate mirrors. This 8 h switch precedes independent standards edits;
legacy recovery cannot create competing normative authority.

Eight languages ship profiles/instructions and minimal inert CI declarations:
Rust, Python, TypeScript, JavaScript, Java, Go, configuration-management,
infrastructure-provisioning. Products/managers are values. Every gate category
is present; one default and explicit alternatives cannot weaken standards.
Rust references the released pinned gate. Seven complete non-Rust gate projects
remain separate at 24–40 h each (168–280 h total), not M3 qualification.

#### Recovery and private collection boundary

C29's pre-M3 audit covers every one of 395 legacy manifest/template files with
path, SHA-256, attribution, destination and disposition. Recover four authored
core agents, one Maestro prompt merge, reviewing skill and handoff now; author
six required unauthored roles afresh across baseline/C72. Phase 1 has ten core
personas; Phase 2 adds scaffolder. Minimal defers Translator/translation skill
with two references, two instruction families/13 references, five legacy YAML
policy conversions, six hook documents and nine full starter inventories.
Existing new M3 policies/imports/profile declarations/minimal Rust init remain.
C74 records every retained rule's real Cedar or unsupported-prose disposition,
including single-owned CMD-008; no pretend injection detector.

The historical nine starter inventories contain 107 payloads: repository 40,
Rust/Python/Go 20, Java/TypeScript/JavaScript 33, configuration/infrastructure 14.
Drop the repository `just/lang.just` stub: 106 retained/adapted payloads.
C52c recovers three authoring templates. Drop 45 placeholder agents, 130
`.gitkeep` files, 39 section records and the obsolete validator/schema stack;
these categories use different units and do not sum to a new whole-tree count.
Do not copy scratch graphs/caches or infer content from empty directories.

C41–C43 retain S6's external opt-in restricted collection mount under public
application-workflow ownership and its private preset. Use functional logical
names `workload-docs`/`workload-private`; the external repository name is not a
catalog ID. No replacement owner record, public→private dependency, shadow or
new grant. Check public alone before combined sources under aggregate bounds;
missing access disables private selection only. URL rules instead belong to
source resources in admitted packages; private source rules use C69, never a
widened mount. Current rule/access checks on seeds, discovery and every redirect,
private provenance/retention and zero fetch on check/install remain required. No private source was read for C44.

### D14 Backends, preferences and extension boundaries

Reuse `SourceTree`, kind hooks, `PresetPort`, S1 settings, C04, kernel install
records, C14 admission, C18 lock/explain, `PolicyChecker`/`HostFacts`, graph and
vector ports. Register product translations behind role adapters; knowledge
never imports catalog TOML or a concrete database SDK. New consumer semantics
get a narrow reviewed adapter, not generic kind switches or a plugin loader.

Common defaults and core backend files feed **one manifest-backed lowest S1
slot**, one producer per key. Free flags > nearest safe workspace > user
preferences > pinned defaults; bounded user/default ceilings intersect workspace,
flags and package limits. Additive restrictions accumulate/intersect; any locked
attempt refuses even if masked. Validate whole files and inactive type tables.
Preserve landed C05a/C05b no-ancestor merge, held-handle discovery, explicit MCP
workspace, English-artifact boundary and frozen sessions; C46/C47 are follow-ups.

Keep `graph.engine`; map `type = "ladybug"` via the registered adapter. The
product-free knobs are `graphdb.buffer_pool_size`, `graphdb.max_db_size` and
`graphdb.max_num_threads`; old `lbug` values require explicit migration.

| Graph control | Contract, not a measured optimum |
| --- | --- |
| Buffer pool | 16 MiB–1 GiB; default 256 MiB; no zero/auto |
| Database size | Power of two, 16 MiB–1 TiB; default 16 GiB; not a disk quota |
| Query threads | 1–64; default 2; unrelated to Cargo parallelism |
| Root/reader/writer | Locked rooted handle, read-only reader, writable writer; no manifest filesystem path |
| Checkpoint-on-close | Locked true through actual supported adapter behavior; checked checkpoint/close/reopen, no invented setter |

Native activation requires G25-qualified fork/lock/build/feature metadata,
source-only build and disabled native extension installer. Uncompiled selection
refuses before native calls; repair commands remain usable. Explicit `none`
makes zero graph calls; stale/absent/rebuilding graph gives explicit unavailable
while passage retrieval continues. C48's qualified graph/vector publication
waits for G25/E07a and applicable backend evidence, then G22 feeds C49a; there
is no C48↔G22 cycle. Qdrant retains identity/dimensions/lifecycle and local
endpoint/credentials; base defaults cannot change these.

A package's `backend_extensions` must name exactly its role files. Registered
extension shapes allow namespaced additions and narrowings only: no type/base/
endpoint replacement, borrowed namespace, expanded budget/trust or server shadow.
Intersect/minimize multiple limits; empty/conflicting results refuse independent
of input order. Aggregate ceilings apply to all additions together. Core type
changes revalidate every selected extension before activation. Logical collection,
projection and tool bindings neither create stores nor authorize effects.

C02 supplies the minimal checked MCP base for the existing S1 knowledge server
in `core/backends/mcp/config.toml`; core/presets bind it without a retired MCP
resource edge. C21 reuses it and C08 depends explicitly on C47b's consumer
wiring. C48 reuses that same base at qualified backend publication, so neither
C48 nor G25 gates the early owner loop. Raw server records have no resource ID.
Cross-package consumers require the binding owner's package;
extension tools additionally require their qualified extension ID. Resolve
namespace/server/tool at the final slash, never basename fallback. Add/remove
derives the effective core view from immutable core plus selected records and
owns only unchanged host entries. Lock/selection/projection/receipt activate
atomically at a safe boundary; preserve shared dependencies, edits and kernel data.

Phase 1 secret-bearing fields use exactly one environment-variable or keychain
service/account reference. Literal secrets, credential URLs, environment-value
maps and credentials in arguments/defaults refuse. Checking/install/explain
never resolves or persists secrets; missing runtime binding refuses the action.

Phase 2/X1 extensions use any language out of process; Maestro code remains Rust.
`extension.toml` requires name/version/description/type, tools or hook-subscriber
kind, runtime name/version, verified relative entry, MCP requirements, tool
input/output and config contracts, secrets, egress ceiling, tests/evals and
subscriber points where applicable. Code is one explicit local inventory or
immutable version/digest/platform-assets/expected-signer release; verify with
existing attestation ports. No latest/branch/shell/install/build scripts. Local
runtime authority supplies interpreter/executable bindings. Every action is an
MCP tool call; ADR-0013 durable events/cursors remain independent.

Each extension needs its own trusted Cedar egress grant bound to principal,
package/code digest and destinations. Default deny is absence of a valid permit,
not an unconditional forbid that makes all valid permits impossible. Hard denies
still win; unknown facts, wrong digest, invalid consent or unmediated action deny.
S3 checks/verifies/projects data and launches nothing. S4 owns sandbox/process
verification/limits/restart/supervision and durable subscription delivery.

### D15 Checkpoints, lifecycle and gap deliverables

All commands below are target interfaces, not statements that the current binary
implements them. Reuse one checksum-pinned released checker; no new framework.

| Checkpoint | Target command/binding | Evidence boundary |
| --- | --- | --- |
| Editor | `schemas/source-2/index.json`; `maestro catalog check --catalog-dir DIR` | Structured fields plus full semantic checker; no approval, trust or runtime proof |
| Pre-commit | `catalog check`, `catalog fixtures`, schema/codeowners/index `--check` | Whole working-tree closure and authored refusal neighbours; partial staging still needs exact-commit CI; offline/no launch |
| CI | Same checks, deterministic compile/read, policy/graph fixtures; trusted `catalog owners --check-identities`; Phase 2 `catalog eval --all --offline` | Exact source/check attestations; untrusted PR gets no credentials; eval drivers isolated/egress denied; unsupported or zero cases fail |
| Install | `maestro package add <package-id>@<version> --apply` | Exact signatures/digests/current trust/compatibility/closure/local authority/owned projections before atomic activation; no eval, runtime install or launch |

Generation commands are `maestro catalog schema --output schemas/source-2`,
`catalog schema --catalog-dir DIR --check`, `catalog codeowners --catalog-dir DIR
--check` and `catalog index --catalog-dir DIR --check`. They compare schemas,
`.github/CODEOWNERS`, `marketplace/index.json`, `docs/catalog/by-type.md` and the
starter view `docs/catalog/templates.md`, rejecting stale/extra/missing rows.
Index source entries are non-installable until a verified release identity
exists. Public indexes never enumerate private metadata. The marketplace is a
view of pinned inputs, not network trust authority. Existing presets and CLI
add/remove remain Phase 1; C56 expands the selector in Phase 2.

**Phase 1 lifecycle:** exact package versions/dependency pins, runtime/descriptor/
feature/tool/host compatibility and existing signed whole-catalog bundles.
Package selection stays within an admitted bundle, not independent publishers.
Preserve five-minute refresh, offline expiry `min(expiry, issuance + 24 h)`,
revocation/floors, user-only catalog auto consent, exact permission/hook-change
human approval, idle transactions, receipts, rollback and propose-only runtime.

**Phase 2 L1/E1:** independently signed/private package delivery, eval attestation,
dependency intervals and deprecation. Exact `=1.2.3` or one `>=1.2.3, <2.0.0`
interval only, bounded integer components and stable releases; no latest/caret/
wildcard language. Keep an installed/locked compatible version; otherwise choose
the highest admitted stable version in the selected signed index satisfying all
constraints. Empty intersections, duplicate publishers and republished versions
refuse. Optional packages cannot force a core upgrade; propose a whole compatible
transaction. Deprecation warns with reason/replacement/migration guide, never
auto-replaces; ordinary removal waits one minor release and 90 days, in a new
major. Authenticated security revocation is immediate. Private publishers need
explicit local authority and scoped credential references; public CI never fetches
them. C69 is distinct from S6's restricted collection mount.

**Phase 2 A0/A1:** `maestro package new <name> --group <group> --owner <identity>`
uses templates/live descriptors and C04 preview/apply, creates authored requested
kinds only and refuses occupied files/invented owners/self-review. Language and
standard kinds use their root placement; central approval controls standards.
Scaffolder is an ordinary unprivileged persona running available checkpoints,
never a generation service or authority bypass. `maestro catalog explain
<qualified-id-or-setting-key>` reuses C18/S1's frozen snapshot: exact version/
digest/layer/field/requirement path, contributions/narrowings/rejections and
adapter/evidence state. No re-resolution against changed files; MCP path-free,
private names/secrets redacted. Phase 1 config explain remains available.

| Gap group | Phase and existing seam | Falsifiable deliverable |
| --- | --- | --- |
| G01–G02 starters | G01 P1; G02 P2/X1+E1; C50/C51/index and extension/eval ports | Typed input/output contracts and one index entry per eligible inventory; unknown parameters/stale output/missing standard refuse; optional renderer declaration and one isolated synthetic expected-output comparison, not a new renderer implementation |
| G03–G04 consumer setup | P2/L1 after receipts/package transactions | Existing receipt binds inventory/source/generator/schema/nonsecret normalized inputs/output digests/standard snapshot/approved operation; fresh preview on change, preserve edits. Portable enrollment schema/plan, no fleet service or real records in catalog |
| G05–G06 lifecycle | P2/R1 after workflow/contracts/eval | Inception/construction/operations assets in feature-delivery, stable steps, blocking/advisory approvals, pending prerequisites/resumable revision-bound evidence and requirement/control/test trace links; one synthetic read-only walkthrough with complete/partial/unavailable labels; no second engine or deployed operator |
| G07–G11 guides/evidence | P1/M3 | Ownership/contributing/security guides; installation/prerequisites/contributor/MCP/recovery; actual-identity release notes and one ledger-linked roadmap; reader index; redacted guard output schema/cases, not extra hook points or live enforcement |
| G12 context | Contract reserved now; P2/A1 static implementation | Approved per-host aggregate injection/description ceilings, reviewed always-loaded sets, stable-rule deduplication, LF/CRLF parity and overflow refusal; neutral evidence consumed by standard checks, no standard→core dependency or removal of rules |
| G13 code graph | P2/R1, existing skill/contract/source kinds | Portable indexing recipe, node/edge/source/extractor/provenance/freshness/exclusion contracts and synthetic stale/malformed cases; full graphs/caches external, zero fetch/build/import on check; unregistered source subtypes refuse |

These gaps add 16 h Phase 1 and 42 h Phase 2 exactly once. They exclude real
renderer implementation, automatic tool installation, remote enrollment,
interactive training, live event/audit transport, code-graph ingestion and full
non-Rust gates. Every task records its phase/milestone and named test/refusal;
Phase 2 never gates M3. No synthetic result qualifies a live runtime.

## Data model

Reuse kernel artifacts, scopes, journal and transactions; do not duplicate
collection or model registries. C12's migration is next free at landing, above
every migration landed or reserved on `main`, `feat/s1-integration`,
`feat/s2-integration`, `feat/s3-integration` and the deployment-modes track.
C00's 2026-09-28 check observed main at 0005 and S1/S2/S3 at 0011; that historical
check does not reserve a number or clear later deployment-modes reservations.
S2 uses the same next-free rule. Recheck all moving heads and lane reservations
with the supervisor at every landing. C00 allocates no number;
`NNNN_catalog.sql` is the number assigned at landing, never a fixed/gapped reservation.
C13/C14/C25 use the same record seam for authority, authenticated state and
discovery respectively; each owns its next-free migration/registration, never
an edit to an applied migration.

| Record | Stored identity and invariant |
| --- | --- |
| Bundle artifact | Digest, source revision and normalized manifest; immutable verified bytes |
| Scoped install | Scope, bundle digest/version, compatibility and verification receipt; active pointer changes atomically |
| Components/closure | Bundle, stable resource ID, kind/digest/maturity, exact dependency edges; no text-inferred relation |
| Artifact pins | Install and project references use existing kernel pins; failed transactions do not leak pins |
| Trust-root authority | Separate catalog/runtime repository, workflow and issuer bindings; approved gh path/version/digest and authority revision; owner provisioning/rotation journal receipt. Only C13a's explicit `catalog authority set` changes these Locked records; restore preserves current records or requires reprovisioning |
| Trust state | Publisher, authenticated record digests/revision, issued/expiry capped at issue + 24 hours, floor, entry/bundle revocations and last accepted clock observation; monotonic and atomic |
| Discovery binding | Install snapshot to catalog collection/generation; only complete verified generations advertised |
| Workspace/user preferences | Versioned TOML at the nearest workspace root or platform configuration home; language/tone, update policy and allowlisted overrides with provenance, never authority |
| Workspace trust approval | User-local kernel answer, canonical path and approval receipt, keyed by canonical root and changed only through explicit user trust administration; never a workspace field or a second authority database |
| Update state and receipt | Per-installation release-check attempt/result, exact verified proposal, idle/activation lease, prior artifact/state pins and linked apply/rollback receipt; reuse kernel transactions and the shared lifecycle |
| Model-card registration | Existing kernel model-card artifacts/registry, evaluations and selections; no second model table. Catalog declaration/version/bundle identity is provenance, never authority for what ran |
| Project lock | Bundle/components, runtime and host versions, exact model-card digest distinct from M059 agent-profile identity, model quantization/template/build, supported OS/sandbox profiles; unsupported values explicit, never authoritative |
| Owned operation | Target root, relative path or owned JSON entry, previous/proposed digest, operation progress; recovery and removal preserve user edits |

## Contracts

C03/C09 freeze source/bundle schemas; C05a/C17 extend S1's shared preference
schema below before their catalog consumers are written. Authoring and compiled schemas are distinct. Unknown versions fail;
there is no raw SDK configuration passthrough.

| Surface | Contract |
| --- | --- |
| Authoring check | `maestro catalog check --catalog-dir DIR`; `maestro-source/2`, area-scoped descriptors, qualified `requires`, fixed body sections, owner/maturity/08 checks; JSON output `maestro-cli/catalog-check/2`. C03 source formats remain Markdown/frontmatter and TOML until later JSON consumers |
| Generated ownership | `maestro catalog codeowners --catalog-dir DIR` renders anchored rules to stdout; `--check` refuses tracked-file drift without writing. Normal source checking validates mirrors; no command configures GitHub protection or approves owners |
| Compile | `maestro catalog compile --catalog-dir DIR --output FILE`; deterministic tar with `bundle.json`, normalized entries/closures, source and runtime/feature/tool requirements |
| Catalog authority | `maestro catalog authority set --catalog-repository OWNER/REPO --catalog-workflow PATH --catalog-issuer URL --runtime-repository OWNER/REPO --runtime-workflow PATH --runtime-issuer URL --gh-path FILE --gh-sha256 HEX [--confirm HEX]`; D2's complete proposal/revision confirmation, terminal default-no or exact proposal digest, journalled atomic provisioning/rotation; never `--yes`, environment approval or MCP |
| Release assets | Canonical SemVer `VERSION` without leading `v` maps exactly to tag `vVERSION`. Payloads: `maestro-catalog-VERSION.tar` and `maestro-catalog-VERSION.spdx.json`; checksum file: `SHA256SUMS`. One lowercase SHA-256, two ASCII spaces, exact basename and LF per payload, sorted by basename; no paths, duplicates or extra entries. Attestation subjects are those two exact payload names/digests under D2's publisher/source bindings. C15 workflow checks and C16 download fixtures assert this same contract |
| Install/update | `maestro catalog install VERSION`, `maestro catalog update`; verified compatible releases only, explicit project lock update, no authoring or unsigned option |
| Model-card registration | C16h: `maestro catalog register-model-card ID --collection COLLECTION`; its exact lock-bound lookup over C18/C14 feeds C03a and the existing scoped kernel registry, with all evidence already local and qualification specific to that machine; explicit, idempotent, no fabricated evaluation/selection record, import, download or MCP equivalent; D12 records S1's latest-registered-answerer and re-registration caveats |
| Source bootstrap outputs | `maestro-project/2` and `maestro-authoring-lock/2`; complete selected areas and source-bound inputs per D13. Old locks require a fresh preview, not silent rebinding |
| Bootstrap | `maestro init` opens plain prompts first, later the approved TUI; `--preset knowledge-client\|rust-service`, `--language TAG`, `--tone brief\|normal\|detailed`, `--updates off\|propose`, `--set KEY=VALUE`, `--yes` support scripts. Preview by default, `--apply` to write, `--plain`/`--no-color` for fallbacks; `--catalog-dir DIR` selects labelled authoring-only mode |
| Settings editor | `maestro config` without arguments opens the same registry-generated every-setting editor as init, with plain/no-color fallbacks. Current value, allowed values, description and source layer come from S1; authorized edits reuse its config API/journal, locked/authority-only entries cannot become preference writes |
| Session preferences | Global `--language`, `--tone` and free `--set` values override the safely discovered workspace file, user `preferences.toml`, then defaults; update/budget values only narrow. No language set means question-language answers. MCP uses `--workspace DIR` only, otherwise user preferences; no roots-based discovery in S3 |
| Workspace trust | `maestro trust add DIR` asks default-no on a terminal; otherwise needs exact canonical `--confirm-path DIR`, never `--yes`/`--json`; missing confirmation exits 2. Refuse filesystem/drive/mount roots, HOME and kernel-internal directories. `maestro trust list`/`remove DIR` read/change user-local authority only |
| Hosts | `maestro catalog project --host copilot\|pi`, `--apply`, or `--remove`; preview-only by default, owned changes only. D1 authoring lock selects/rechecks source bytes; installed lock uses admission |
| Resolve/search/route/impact | CLI `maestro catalog resolve ID`, `search QUERY`, `route INTENT`, `impact ID`; MCP `catalog_resolve`, `catalog_search`, `catalog_route`, `catalog_impact` under the existing bounded stdio server |
| Explain | Phase 1 `maestro config explain`/C18 provenance; Phase 2/A1 `maestro catalog explain ID-or-setting`; exact locked contributors and explicit unsupported/not-observed state, no second resolver |
| Policy | `maestro policy check --stdin` and `maestro policy test --catalog-dir DIR`; bounded normalized input, Cedar diagnostics, allow/deny/approval-needed result without executing effects |
| CLI output | Existing `--json` convention, versioned strict output, diagnostics on stderr; exit 0 success, 1 operation failure, 2 refused input/usage. A test command that discovers no cases does not pass |
| MCP output | Typed statuses and bounded results, caller-bound context, exact snapshot and used generation; initialization instructions expose conversational preferences and any interface fallback note; never expose arbitrary Qdrant filters or administration |
| Startup updates | `maestro update check` works even when startup checks are off; `apply PROPOSAL_ID`/`rollback RECEIPT_ID` use C13/C14/C16 for catalogs only. Runtime proposals show a verified release and exact approved installer command; no runtime apply. MCP never applies updates; path-free fixed notice only |

## Validation

1. **First checkpoint (through C08):** C02's checked Rust seed and MCP base plus
   C47b consumer wiring, without C48/G25 publication waits; strict config, safe discovery/narrowing,
   en/fr/es/ja × three-tone prose and byte-identical artifact/log tests, four-client
   --workspace/user-only instructions, user-local trust and fresh-home CI setup.
   Prove outside-write/secret-read/link-escape denials and plain/script parity.
   Then v4 common/core/Rust init, real Copilot/Pi alias projection/restart, knowledge tools,
   citation/refusal checks, separate denied kernel, repeat and owned-only removal.
   C05f/C05k's later TUI/keyboard/resize/contrast proof and OA9 visual acceptance
   are required before C28, never before this first plain owner loop.
2. **Trust/update proof (C09–C16h):** real valid/wrong-signer verification,
   deterministic archives, hostile reader cases, atomic records/update,
   expiry/revocation, rollback/clock tests and old-backup replay refusal. Include
   C13a fresh-home provisioning/rotation and C16b pre-rotation backup refusal
   without replacing current roots/pin. Add
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
   asserts `cargo`, `rustc` and `python3` absent. Provision D2 authority through
   C13a with exact confirmation before doctor/install. Exercise detached install/init/
   projection-file generation/update/remove there, without host execution. In a
   separate OA2-approved host-home stage, allow the pinned Copilot CLI, Pi and
   its existing adapter, Codex and Claude Code binaries for real load/MCP
   receipts. Require checked ten-point descriptors/mappings with explicit
   unsupported/unqualified states, not a live hook. C20's live allow/deny/error
   proof is transferred to S4 (+4 h there); never call static fixtures protection.
   Record each environment separately; the host stage does not claim toolchain absence.
   Collect real `incompatible` and synthetic routing results,
   release SBOM/checksum/attestation verification, M1 and final three-OS CI.

C28 also requires SC-S3-013's descriptor-only lifecycle proof and SC-S3-014's
card round-trip/returned-ID/refusal proof (C16h, with C03a/C11/C16 role refusals),
plus owner-reviewed C02a content. Require the local-evidence prerequisite and
A/B/A known-gap test; this is not a per-answer kernel history claim. A model
card never counts as S4 qualification. Missing S1 settings or required kernel
role support is a named prerequisite, not something a fixture can qualify.

Every task has a red-first check and bounded files. Budgets include local checks:
≤4 h except C03's explicitly approved 6 h registry/checker task. Split overruns into fresh, independently testable follow-ups;
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
The dated C00 inventory/hook approval is complete and distinct from OA7,
which names only M3 acceptance and eventual main release below.

| ID | Owner action | Needed by |
| --- | --- | --- |
| OA1 | Repository created with README and MIT licence, 2026-09-30; content waits for D13 migration. Supply approved nonempty owners and optional maintainers per area, maintainer/backup and security/platform owners; configure visibility, rulesets, required checks, CODEOWNERS protection and organization properties | MAN C02/C02a/C21/C21b/C15 only; C23 fixtures and all CORE consumers are independent |
| OA2 | **Approved 2026-09-28:** S3 may probe/test the already-installed Copilot CLI, Pi, Claude Code and Codex, each in an isolated temporary home. C01 records every exact installed version as its pin, including adapters used. No installs/upgrades, changes to real owner configuration or enterprise policy. Anything beyond this scope requires fresh OA2 approval; private/provider data approval remains OA6 | C01, C06–C08, C28 may use this bounded scope; C20 live hooks move to S4; unavailable tools/access or broader operations remain blocked. C01's evidence gates C03's format; no later CORE fixture-code gate from live receipts |
| OA4 | Bind separate catalog/runtime repository/workflow/issuer identities, protected environments and emergency/rotation authority. Supply checksum-verified standalone pinned `gh` and repository-bound read-only fine-grained `GH_TOKEN` (or equivalently minimal login), with C09-measured read permissions only; doctor reports detectable excess scopes; authorize any unlisted-licence organization allowlist change, not a second D4 library approval | C09's owned 08 §17 closure and C28 live trust proof; no C09/C13 implementation-start gate |
| OA5 | Publish checksum-pinned compiler and catalog canary/stable bundles/attestations. Enable the catalog publisher's six-hour trust schedule with protected `id-token: write`/`attestations: write` permissions and minimum publication access; enable hourly missed-refresh alerts to maintainer/backup. Perform withdrawal/rotation/missed-run drills. Supply C28's clean WSL user/container with released `maestro`, pinned `gh`, basic shell utilities and read authentication, but no checkout/Rust/Python | C28 release/clean-environment proof; C15 workflow code and C16 fixture tests do not wait |
| OA6 | Grant private-data use per exact client/provider/account/scope, private receipt location and approved model downloads/access. Without it use synthetic data; do not infer permission from T038 code or a logged-in client | C08, C23, C26, C28 only for the corresponding private/model access |
| OA7 | Accept M3 evidence and eventual main release. Any S2 fallback needs a separate explicit approval and 08 disposition | C28 exit; not a new product-choice gate |
| OA8 | **Approved 2026-09-30:** bounded legacy recovery before M3, preserving provenance/attribution and minimal Phase 2 deferrals. Private collection access is not included | C29 and named C70–C78 recovery tasks |
| OA9 | **Approved 2026-09-28:** ratatui + crossterm for the TUI under ADR-0020. C05f still measures minimum features/dependencies/licences/native links and vet before adoption. Owner visual acceptance of C05k's branded keyboard/plain/no-color walkthrough remains pending | Library choice resolved; C05f measurements precede C05k. Visual acceptance gates C28, never the first plain C08. No parser dependency or decision |
| OA10 | **Approved by the owner, 2026-09-28:** amend D5's absolute routing bar to held-out matchable top-1 ≥ 90 % (correct first selection). The original top-3 ≥ 90 % wording is retained as history, not the current bar | Quality target resolved; C24/C26 apply the top-1 gate and report both metrics; C28 verifies evidence against the amended bar before final quality acceptance |

The evening model-card decision also requires the owner to approve each winner's
manifest change and supply approved public card identities/evidence references for
C02a. It grants no new model download, private-data access or evaluation approval;
missing input blocks MAN content, not C03a's synthetic CORE adapter tests.

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
| Trivial routing scores | C23 CORE compiled fixture pinned by digest, ten eligible synthetic workflows, recorded split sizes, held-out top-1 denominator and seeded paired interval |
| S2 G25/G27 late or incompatible | C27a owns catalog schema/adapters; S2 owns the public port. Block impact/M3 until qualified; no fabricated evidence claims or implicit fallback |
| Migration collision across slices | Supervisor allocates above all landed/reserved numbers on main, S1/S2/S3 and deployment-modes |
| S2/S3 shared retrieval or MCP changes | Serialize C26 with S2 G12/G14 and all MCP dispatch edits; cover every retrieval branch present at landing, including R4 graph, or prove catalog queries cannot enter it |
| S1 settings or kernel roles differ at landing | Reuse integrated APIs; retain S3 preference/authority/discovery rules. Extractor waits for G17, query_expander refuses until S1's role follow-up; no silent alias or second registry |

The authoritative [task accounting](tasks.md#critical-paths-and-effort) is
recomputed from headings, phases and After edges. Whole S3 totals are
**143 tasks / 478–490 h**: **108 Phase 1 tasks / 357–369 h** and
**35 Phase 2 tasks / 121 h**. Deferred S6 adds 3 tasks / 6–10 h; transferred
C20 adds 4 h in S4, not S3. Completed work retains historical weights, not a
remaining-work estimate. The original 16–24 h CI/review reserve stays separate;
C39 and new task estimates already include their focused review.

The incremental amendment is **271 h = 150 Phase 1 + 121 Phase 2**, including
7 h retained S2 work, +1 h original C02 v4 adjustment, +1 h C66 URL rules and
+1 h C02 validation fix for the minimal Rust profile/reference, MCP base and
refusal cases. This last hour is the only increase above the owner's approved
149 h Phase 1. C08/C21/C21b/C44/C48/C76a/C85a fixes add 0 h. It replaces the
approved 27 h engine amendment: **244 h extra**, not 271+27.
Phase 1 gaps add 16 h and
Phase 2 gaps 42 h once. Seven later language-gate projects (168–280 h) and S4,
S6 and post-M1 runtime work remain outside this amendment.

Dependency closures are **C08 57 tasks / 191–203 h** and
**C28 108 tasks / 357–369 h**. Endpoint-weighted internal paths are
**C08 66–74 h** and **C28 79–86 h**. C08 adds exactly C46/C47a/C47b (12 h)
and C02's 1 h to its previous 54 tasks / 178–190 h closure; C48 and C76a
remain outside it. C43 stays 24 tasks / 70–84 h with a 42–53 h path.
These are recomputed after validation fixes, not guessed from a lane count.
The DAG is acyclic; Phase 2, C41–C43/S6 and S4 have no edge into M3. C66 separately needs the
existing core wire types synchronized after N07 lands; it does not wait for
the live S6 catalog adapter. External owner identities/approvals, host pins, G25/G27/native release
qualification, M1, publication credentials, CI and shared-file serialization can
dominate these theoretical lower bounds. Re-estimate after C08, never promise
calendar dates from effort sums.
