# Implementation Plan: Catalog

**Branch**: `docs/s3-spec` | **Date**: 2026-09-28 | **Spec**: [spec.md](spec.md)

**Input**: the approved S3 draft, owner decisions of 2026-09-28 01:56 and 08:12,
and the supervisor's specification-review ruling,
architecture [03](../../docs/architecture/03-agent-orchestration.md),
[06](../../docs/architecture/06-roadmap.md) and
[08](../../docs/architecture/08-traceability.md). Tasks: [tasks.md](tasks.md).

## Summary

Build the smallest useful catalog first: one knowledge preset, the Maestro
profile and the dependencies of `ctm-question`, then the Rust overlay. Explicit
selection and the existing knowledge tools provide the first owner loop;
there is no engine or intent router on that path. Label reviewed-source
bootstrap and projection as authoring convenience.

Then complete M3: deterministic bundles, verified installation, current trust
records, settings, policies, graph validation and measured routing. Reuse the
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
is not a runtime schema validator.
Pinned `gh` performs attestation verification. No custom signature code, new
Pi extension, separate service or speculative interface crate.

**Storage**: kernel SQLite and content-addressed artifacts remain authority.
The catalog collection in Qdrant and the S2 dependency projection are
rebuildable. Project descriptors, locks and ownership records are local files,
not a second authoritative catalog database.

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
100+ independently reviewed intents. No new languages or roles without a real
workflow requirement.

## Starting point (checked 2026-09-28)

The fresh planning clone starts at
`dac543ce29aee667234821c63e3947c0d5441b34` on `feat/s3-integration`.

| Seam | Observed state and reuse |
| --- | --- |
| CLI | `crates/maestro/src/cli/args.rs` and `run.rs` have no init/catalog/config/policy commands; add thin adapters there, not a second binary |
| MCP | `crates/maestro/src/mcp/server/handler.rs` advertises collections, get, search and ask; reuse bounded transport and server operations. The draft's older `server.rs` path no longer exists |
| Host proof | `docs/how-to/knowledge-mcp.md` and `crates/maestro/tests/it/mcp_clients.rs` exist; client-like stdio tests do not prove real Pi/Copilot loading or T038 completion |
| Retrieval | `crates/maestro-knowledge/src/search/filter.rs` filters scope/version only; C26 must add eligible-ID filtering before every branch's limit |
| Evaluation | `crates/maestro-knowledge/src/eval/` scores sections, not workflow IDs; reuse statistical methods, not section labels masquerading as intent labels |
| Kernel | Scoped records, artifacts, journal, jobs and backup exist. Migrations end at `0011_exact_identifiers.sql`; catalog numbering must exceed every landed/reserved number on main and both S2/S3 integration branches |
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

Recheck this table at C00, after C09's measurements and at C28. A dependency
exception names the exact crate/version and removal condition in
`maestro-quality.toml`; it is not permission to relax the whole gate.

## Project Structure

### Documentation (this feature)

```text
specs/003-catalog/
├── spec.md                    # requirements and observable outcomes
├── plan.md                    # design, contracts, decisions and owner actions
└── tasks.md                   # bounded, test-first implementation tasks
```

C01/C09 later add measured research under `specs/003-catalog/research/`.
C00/C28 maintain architecture 03/06/08 and the exact delivery map. This commit
creates specifications only; it does not claim those implementation tasks done.

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
    ├── install/               # verified download and atomic activation
    ├── trust/                 # verifier and shared freshness admission
    ├── settings/              # classes and restrictive resolution
    ├── resolve/               # exact definitions, locks and explanations
    ├── policy/                # Cedar checks and native request normalization
    ├── graph/                 # topology and contract checks, not execution
    ├── route/                 # exact-ID, lexical and measured hybrid routes
    ├── eval/                  # intent labels and paired comparison
    ├── discovery/             # cards through S1's publication pipeline
    └── impact/                # exact dependencies through qualified S2
crates/maestro-filesystem/           # existing ADR-0018 code moved unchanged
crates/maestro-kernel/src/catalog/    # scoped install/trust/component records
crates/maestro/src/cli/              # init, catalog, config and policy adapters
crates/maestro/src/mcp/              # catalog tools in the existing server
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

C00 synchronizes architecture 03/06/08: graph checks needed to safely compile
S3 bundles land in C22a/C22b; S4 still owns general runtime execution. OA7
approves the frozen exact row-key inventory and remaining portions, including
GD4's Copilot-only S3 hook and S4 host-adapter qualification for the other
clients. Do not quietly implement a smaller graph language. A supported
construct passes all applicable checks; unsupported means not executable.

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

C00 amends architecture 06's old unconditional hybrid-win exit to the approved
conditional rule and reconciles architecture 03/08. Top-3 ≥ 90 % remains the
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

### D7 Settings, policies and graph checks

Resolve free, bounded, additive and locked values as specified. Permissions
intersect with parent grants; prohibitions and checks accumulate; budgets take
the stricter bound. A capability's values affect its own role or step only.
`config explain` and `catalog explain` keep declared, effective and observed
states distinct. A missing S4 qualification is unsupported, not evidence that a
model or sandbox works.

Carry the architecture 03 defaults as declarations, not an S3 run engine:

| Setting | Default | Class |
| --- | --- | --- |
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

## Data model

Reuse kernel artifacts, scopes, journal and transactions; do not duplicate
collection or model registries. C12's migration is next free at landing, above
every migration landed or reserved on `main`, `feat/s2-integration` and
`feat/s3-integration`. The supervisor checks all three heads and moves S2's
fixed reservations under the same allocation rule before either slice lands.
`NNNN_catalog.sql` is the assigned number, not a fixed/gapped reservation.
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
| Project lock | Bundle/components, runtime and host versions, model identity/quantization/template/build, supported OS/sandbox profiles; unsupported values explicit, never authoritative |
| Owned operation | Target root, relative path or owned JSON entry, previous/proposed digest, operation progress; recovery and removal preserve user edits |

## Contracts

C03/C09 freeze strict schemas from these fields before their consumers are
written. Authoring and compiled schemas are distinct. Unknown versions fail;
there is no raw SDK configuration passthrough.

| Surface | Contract |
| --- | --- |
| Authoring check | `maestro catalog check --catalog-dir DIR`; strict frontmatter/TOML/JSON, fixed agent body sections, owner/maturity/08 references, exact dependencies and diagnostics |
| Compile | `maestro catalog compile --catalog-dir DIR --output FILE`; deterministic tar with `bundle.json`, normalized entries/closures, source and runtime/feature/tool requirements |
| Install/update | `maestro catalog install VERSION`, `maestro catalog update`; verified compatible releases only, explicit project lock update, no authoring or unsigned option |
| Bootstrap | `maestro init --preset knowledge-client` or `--preset rust-service`, preview by default, `--apply`; `--catalog-dir DIR` selects the labelled authoring-only path |
| Hosts | `maestro catalog project --host copilot\|pi --dry-run`, `--apply`, or `--remove`; preview-only by default, owned changes only |
| Resolve/search/route/impact | CLI `maestro catalog resolve ID`, `search QUERY`, `route INTENT`, `impact ID`; MCP `catalog_resolve`, `catalog_search`, `catalog_route`, `catalog_impact` under the existing bounded stdio server |
| Explain | `maestro config explain` and `maestro catalog explain ID`; declared/effective/observed values, class, source rule, requester and unsupported/not-observed states |
| Policy | `maestro policy check --stdin` and `maestro policy test --catalog-dir DIR`; bounded normalized input, Cedar diagnostics, allow/deny/approval-needed result without executing effects |
| CLI output | Existing `--json` convention, versioned strict output, diagnostics on stderr; exit 0 success, 1 operation failure, 2 refused input/usage. A test command that discovers no cases does not pass |
| MCP output | Typed statuses and bounded results, caller-bound context, exact snapshot and used generation; never expose arbitrary Qdrant filters or administration |

## Validation

1. **First checkpoint (C08):** fresh home/project, base and Rust init, real
   pinned Copilot/Pi projection and restart, three knowledge tools, citation and
   refusal checks, separate denied kernel, repeat and owned-only removal.
2. **Trust proof (C09–C16b):** real valid/wrong-signer verification, deterministic
   archives, hostile reader cases, atomic records/update, expiry/revocation,
   rollback/clock tests and old-backup replay refusal. Test publisher rotation
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

These are external operations, not actions for a coding lane to perform.
D1–D5 approval does not constitute credentials or permission to execute them.
Wait time is outside task budgets; mark blocked when evidence is unavailable.

| ID | Owner action | Needed by |
| --- | --- | --- |
| OA1 | Create public `Orchestration-Maestro/maestro-manifests` with its first reviewed generic content; assign maintainer/backup and security/platform owners; configure visibility, rulesets, required checks, CODEOWNERS protection and organization properties | Real MAN content in C02/C21/C21b/C23 and workflow landing in C15, not CORE fixture code |
| OA2 | Supply/approve exact Copilot, Pi and adapter pins/downloads; install/upgrade hosts, approve accounts/enterprise MCP policy, provide isolated homes and authorize host configuration/restarts | C01, C06–C08, C20, C28 live proofs only; C03/C05/routing do not wait |
| OA4 | Bind separate catalog/runtime repository/workflow/issuer identities, protected environments and emergency/rotation authority. Supply checksum-verified standalone pinned `gh` and approved `gh` login or `GH_TOKEN`; authorize any unlisted-licence organization allowlist change, not a second D4 library approval | C09's owned 08 §17 closure and C28 live trust proof; no C09/C13 implementation-start gate |
| OA5 | Publish checksum-pinned compiler and catalog canary/stable bundles/attestations. Enable the catalog publisher's six-hour trust schedule with protected `id-token: write`/`attestations: write` permissions and minimum publication access; enable hourly missed-refresh alerts to maintainer/backup. Perform withdrawal/rotation/missed-run drills. Supply C28's clean WSL user/container with released `maestro`, pinned `gh`, basic shell utilities and read authentication, but no checkout/Rust/Python | C28 release/clean-environment proof; C15 workflow code and C16 fixture tests do not wait |
| OA6 | Grant private-data use per exact client/provider/account/scope, private receipt location and approved model downloads/access. Without it use synthetic data; do not infer permission from T038 code or a logged-in client | C08, C23, C26, C28 only for the corresponding private/model access |
| OA7 | Approve C00's frozen exact key inventory and S3/remaining portions, including four-client MCP with Copilot-only hook administration in S3 and Pi/Codex/Claude Code hook qualification in S4. Accept M3 evidence and eventual main release. Any S2 fallback needs a separate explicit approval and 08 disposition | C00 inventory acceptance and C28 exit; not a new product-choice gate |
| OA8 | After M3, grant access to the earlier catalog and approve any recovery work, provenance/licence obligations and separate publication PRs | C29 and later recovery tasks |

The lane's authorized signed push to its own branch is not a product release.
It does not create a repository, change shared settings, activate workflows,
publish a tag, install clients or send private data to a provider.

## Risks and estimates

| Risk | Early check and response |
| --- | --- |
| Scope/review outruns value | C02/C08 visible checkpoints; one task per fresh lane, no speculative files |
| Host parser/provider drift | C01 real pinned probes; absence blocks instead of yielding a false pass |
| Data loss or trust replay | C04a preserves existing ADR-0018 tests; C04 crash/race cases, C13 real signatures, C14 clocks/revocation, C16b restore |
| Publisher outage refuses every laptop within 24 h | Six-hour attestations, independent hourly alert at eight hours, OA5 missed-run drill; never extend expiry to hide an outage |
| Auth/API limits prevent five-minute refresh | C09 measures pinned `gh` authentication and calls per active laptop/root; doctor checks readiness; rate limiting uses the bounded offline window |
| Privacy or missing qualification | Synthetic default, exact approvals; S4-only roles yield real `incompatible`, not a vacuous measured routing success |
| Trivial routing scores | Frozen synthetic eligibility digest, nonempty denominator, independent distractor labels and paired held-out comparison |
| S2 G25/G27 late or incompatible | C27a owns catalog schema/adapters; S2 owns the public port. Block impact/M3 until qualified; no fabricated evidence claims or implicit fallback |
| Migration collision across slices | Supervisor allocates above all landed/reserved numbers on main and both integration branches |

The revised **36 tasks total 118 lane-hours**: 29 to the owner-loop checkpoint,
86 more to M3 and 3 for the post-M3 comparison; allow **134–142 lane-hours**
with the unchanged 16–24-hour review/CI reserve. Re-estimate calendar delivery
after C08; the draft's 7–10-day M3 estimate did not include the added projection
and task splits and is not a commitment. At six lanes/eight effective hours,
total effort still does not divide the serial dependencies by six. M1 may finish
in parallel; its release, owner actions, live hosts and S2 qualification remain
exit waits outside these budgets. Shared CLI, lock and migration work stays
serialized.
