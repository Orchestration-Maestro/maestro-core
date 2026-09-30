# Native Acquisition Implementation Plan

> **For agentic workers:** use superpowers:executing-plans for the task in your
> brief. A lane starts no subagents, changes no integration branch, and reports
> evidence rather than ticking its own delivery complete.

**Goal:** acquire approved sources with Rust producers, preserve their evidence,
and feed S1 without replacing its downstream pipeline.

**Architecture:** kernel-owned frontier, artifacts and journal; a small acquisition
library with replaceable policy, transport, extractor, connector and scheduling
ports. Local adapters work without S3/S4. A factored S1 import transaction accepts
mapped extraction; S1 alone prepares, embeds and publishes.

**Tech Stack:** Rust 1.98.1, edition 2024, MSRV 1.98; existing SQLite, serde,
reqwest, Tokio, Markdown and model ports. New libraries are approval-blocked.

**Spec:** [spec.md](spec.md), Revision 2, 62 FRs and 15 SCs.
**Research:** [research.md](research.md), Revision 2.1 (source-register completion only), sources accessed 2026-09-30.
**Tasks:** [tasks.md](tasks.md).
**Branch:** `docs/s6-spec-set`, from S1 `0204846f7640e7219021b0b38daa60b4c4f98ee4`;
the supervisor creates `feat/s6-integration` after review.
**Status:** proposed implementation design, not adopted libraries, live authority,
qualification results or independent approval. Spec requirements prevail over
research alternatives: no Python extractor/connector/producer qualifies cutover.
Research's term-alias question is outside S6; source-reference continuity and
publication aliases are in scope, synonym dictionaries are not. The spec also
settles the Xberg name; research's older naming question is not an open scope
decision, while adoption of a particular version remains OA5-blocked.

## Global constraints

- S6 depends only on S1 delivery, not M1 release or S3/S4 delivery. Missing
  downstream qualification evidence still blocks the gate that needs it.
- The manifest is processing-configuration authority; it cannot grant access.
  Questions do not crawl. Grants require a separate owner-authenticated writer.
- First-party producers, extractors and connectors are Rust. Native components
  need named OA5/ADR-0020 exceptions. No implicit download or hidden fallback.
- Preserve source bytes, permissions and losses. Unknown is not zero or passed;
  held, partial and unsupported outputs are not searchable admission.
- Only the five automatic change classes in spec §Fixed mandatory gate matrix
  are eligible. Every cell is mandatory; missing retrieval evidence holds.

## Technical context

| Area | Choice and constraint |
| --- | --- |
| Workspace | New `maestro-acquisition` library over `maestro-kernel` and `maestro-knowledge`; existing `maestro` CLI binds ports. No separate daemon/database, general extension runtime or agent-driven crawler. |
| Storage | Existing kernel SQLite WAL, artifacts, jobs and journal; new frontier/capture/receipt records are kernel-owned. Network, conversion and models run outside transactions. Forward-only migration gets the next free number at landing, not a reserved number here. |
| Dependencies | Reuse root Cargo versions, including reqwest 0.13.5, pulldown-cmark 0.13.4, whatlang 0.18.0 and serde. Spider, converters, robots, codecs and containment helpers remain pending owner approval; no dependency is added by this plan. |
| Platforms | Linux, Windows and macOS. Unit contracts run everywhere; actual browser/process containment and offline extraction must be tested on each claimed environment. An unavailable control blocks that adapter, never chooses an unsandboxed implementation. |
| Tests | Rust unit and existing single-binary integration suites, independently authored synthetic fixtures, real child-process denial/crash tests, separately approved local private comparisons. Tests first; code coverage/mutation gates run in CI. |
| Performance | No invented crawl-throughput target. Enforce approved OA3 budgets and inherited S1 release/interactive latency floors. Report observed CPU/RAM/VRAM/storage, pending work and cold/warm latency under named hardware/configuration. |
| Scale | Owner-frozen families/media/strata, not a guessed corpus size. OA3's proposed defaults are inputs, not measured capacity; OA4c bounds every live sample. Parser/protocol ceilings below are independent defensive format limits. |
| Execution | Manual command first, local timer later, both invoke the same operation. Protected sources use admitted subprocess connectors. Independent sources continue after a typed source-local failure; stop-all and revocations always win. |

### Existing seams and explicit gaps

Evidence below was inspected at S1 `0204846`; recheck paths/signatures at each
implementation dispatch, preserving these contracts if S1 moves.

| Evidence | Reuse or change |
| --- | --- |
| `crates/maestro-knowledge/src/import/entry.rs:99–137,165–187,229–258` | Current importer verifies bytes, derives IDs, records dispositions/revisions and artifacts; canonicalization receives metadata but no extractor blocks/assets. Factor this path, not a second importer. |
| `crates/maestro-knowledge/src/corpus.rs:33–70` | `maestro-corpus/1` has no typed block/location/asset handoff. Keep this interchange version unchanged; its existing extractor map is not the new transport. |
| `crates/maestro-canonicalization/src/model.rs:124–173` | `ExtractorBlock`, `OriginalLocation`, `CanonicalizeInput.extractor_blocks` and `.assets` already exist. Preserve their exact types and validation rather than inventing another canonical document. |
| `crates/maestro-kernel/src/{job,artifact,document,generation,scope}/` | Reuse fencing/job progress, digest-addressed storage, occurrences, dispositions, scoped reads and compare-current publication. Frontier and authority-store separation are new, not claimed implemented. |
| `crates/maestro-knowledge/src/{prepare,index}/` | Existing exact/near grouping, structural chunking, token/profile binding, prepared cache, embedding validation and verified generations stay authoritative. Source aliases are not the Qdrant current-generation alias. |
| `docs/architecture/07-extensibility.md:144–217` | Reuse JSON-RPC 2.0 stdio lifecycle and Operations API. The local S6 supervisor implements the bounded subset below, not S4's general event host. |

### Phase 0: research and decisions

Research Revision 2 is complete as source inspection, **not** a qualification run.
The decision is the minimal Rust-owned stage map with research option **(b)**,
the in-process core ingestion seam. Option (a), corpus/2 interchange, is deferred
until an external/offline producer requires it. Adapter processes still cross
an untrusted governed boundary; they do not call Rust APIs inside the core.

| Adoption group | Recommendation from research; all pending OA5/ADR-0020 |
| --- | --- |
| Crawl/browser | Reuse reqwest; evaluate Spider 2.53.9 with defaults disabled as a leased fetch/render adapter. Its chrome path uses chromey; use one qualified browser stack. chromiumoxide is a replacement only if control hooks fail, not a second default. Chromium needs its named native exception. [research §§2, 3.1; P1, P3, P27] |
| Technical HTML | Site selectors plus htmd 0.5.5; dom_smoothie 0.18.2 only a gated selection fallback. Compare Xberg's already-transitive html-to-markdown-rs before retaining two converters; never chain converters. [research §3.2; P9–P11, P27] |
| Documents/OCR | Offline bake-off Xberg 1.3.0 minimal native PDF/Rust layout/OCR versus docling 1.78.0 `default-features = false, pdf-text`; one qualified path/profile. Prefer tract-supported layout; ocrs/RTen are bounded alternatives, not assumed multilingual support. `docling-rs` is a service SDK, not this converter. [research §3.3; P5, P6, P24–P28] |
| Spreadsheet/detection/helpers | calamine 0.36.1 for cells plus separate formulas; reuse Xberg's infer if selected, otherwise evaluate infer 0.22.0. Reuse whatlang with abstention; evaluate texting_robots 0.2.2 against RFC 9309 and minimum ZIP/TAR codecs. No tree_magic_mini embedded database, new language detector or ADWIN without a demonstrated need and approval. [research §§3.4–3.5; D1–D5, P16, P20–P21] |
| Sessions/native assets | Existing read-only KeePassXC mechanism first, separately approved external component, not plaintext unlock files. keyring/keepass/secrecy/zeroize only if needed and approved. ONNX-only table models may require dynamic ORT; PDFium/full docling ML is an optional comparison. Xberg/docling feature edges enabling `download-binaries` must be split upstream or by an approved audited patch before adoption; adding dynamic loading alone does not remove additive download features. [research §§3.3, 3.7–3.8, 6; P17–P19, P23, P27] |

N02 records exact features, added packages, duplicates, native links, licence and
model-weight terms against the then-current lock. Research's historical counts
are not current measurements. Approved exceptions name the forcing library,
role, artifact digest, scope and removal condition. No speculative alternate
engine is adopted simply because the research lists it.

### Approval ledger and blocked effects

| Approval | Work allowed before approval | Blocked until receipt exists |
| --- | --- | --- |
| OA1 | Synthetic test design and non-qualifying probes | Q1–Q5 qualification, automatic activation, qualified publication and family cutover; recommend spec's exact targets/matrix and deterministic equality or approved non-inferiority protocol. |
| OA2 | Default-deny implementation and isolated synthetic private-range cases | Architecture private-network amendment and live named-origin access; origin grant remains separately necessary. |
| OA3 | Enforceable budget implementation and fixture envelopes | Live concurrency/adaptation without an approved envelope. Use spec's proposed defaults only after approval. |
| OA4a/OA4b | Synthetic source/account/authority fixtures | Real source inventory, protected account/preflight, robots overrides and source grants. Authority command must enforce separation before any live grant. |
| OA4c/OA4d/OA5 | Public fixture design and source inspection | Private reads/comparison/retention, each family retirement, new library/component/model adoption respectively. No task authorizes itself. |

## Constitution check

The workspace-installed `.specify/memory/constitution.md` delegates to the
organization rules and repository [rule map](../../docs/standards/engineering.md).
No new exception is granted here. Pre-design: scope is explicit, downstream reuse
is established, private reads are not needed for planning. Post-design: no known
unresolved design conflict; effects remain blocked at named approval/qualification
boundaries. A planned enforcing test is not a passing test.

| Rule IDs touched | Enforcing task, test or review |
| --- | --- |
| FND-001–004, P-001–006 | N01 requirements/architecture review; N02 measured shortlist, N26 shared import seam; per-task red/green evidence and final N54 audit prevent speculative duplicate engines. |
| P-009, P-015, ARC-001–002 | N01 ownership map and every commit's architecture/size gates; module doors contain declarations/imports only. Ports separate admission, processing and grants. |
| P-011–014, SEC-002–004, ENF-011 | N03 strict parsers, N05 real owner-only authority boundary, N07–N10 egress/robots refusal and N32 whole-proposal protected-field comparisons; hostile content changes no grants. |
| SEC-001, ENF-001, ENF-013 | N06 scoped artifact/log/notifier canaries; N15/N16 privacy-preserving extraction receipts, N45 session canaries, N50–N54 private evidence stays local; path checks and gitleaks on every commit. |
| SEC-005 | N05 scoped authenticated authority command; N48 OA2 checks; N51/OA4c and N53/OA4d explicit live/cutover decisions. A digest, review or generic `yes` is never owner authentication. |
| SEC-006–007 | N17–N19 real platform containment; N43–N46 connector/browser denial, stop/reap, crash and expiry tests. Stop affected work and use private incident channel for exposure; no evidence deletion. |
| SEC-008–009, C-005 | N04/N06/N12–N14 reconcile pending/partial receipts; each task records commands, outputs and unchecked controls; N50–N54 report blocked strata rather than remove them. |
| C-001 | N01 reconciles 08 row ownership and this rule table; N54 verifies final FR/SC/evidence links. |
| C-006, DEP-001, ENF-009, ENF-012 | N02 dependency/licence/vet and exception records; N21/N22 pin offline binaries/models/features; N43 refuses substituted extension artifacts at each launch. Exceptions expire by named condition, not silent exclusions. |
| ENF-002–003, ENF-005–008 | All tasks use English, failing tests first, signed conventional commits and review. CI runs three-OS tests/Clippy, coverage and mutation gates; no local gate weakening or self-integration. |
| ENF-010 | N03 single manifest schema, N30–N35 proposal/CAS/persistence contracts; N05 grants remain separate authority, N55 catalog adapter cannot rewrite trusted bundles. |
| SEC-010–011 | N54 checks existing private disclosure and signed-release/checksum/SBOM workflow evidence; N43/N53 verify launched/cutover artifacts against it, not a checksum-only trust claim. |
| COV-001–002, TST-001 | Every implementation task records deterministic red/green tests; N54 requires CI ≥90% overall and ≥95% changed-line coverage, zero missed mutants and zero mutation timeouts. Network/model/private evidence cannot be silently skipped. |

## Phase 1: architecture, data model and contracts

### Architecture amendments before code

N01 is a documentation-only prerequisite to every S6 code task. It changes all
four decided locations and corresponding 08 rows together:

| Location | Amendment and 08 row reconciliation |
| --- | --- |
| `01` §2.2 introduction | Qualified local subprocess connectors now; S4 later. Reconcile `ingest modules`, `ingest scope`, `ingest sessions`, `delivery.§2.2`. |
| `01` §2.2.1 Scheduling | CLI/local timers invoke the same admitted sync operation now; no second queue. Reconcile `ingest modes`, `delivery.U11 (D2–D3)`, `delivery.U12 (Q4, D4–D5)`. |
| `01` §2.2.7 registries | Machine-qualified new knowledge exclusions/asset-only entries only under the closed allow-list/matrix; no machine-as-human label. Reconcile `ingest policy`, `ingest precedence`, `ingest receipts`. |
| `06` §S6 | S1-only delivery dependency, local adapters, gated adaptation and collision-safe parallel cutover. Reconcile `ingest approach`, `ingest updates`, `ingest audit defects` and delivery rows above. |

N01 also checks every existing key in spec §Requirement inventory, including
`rag.N011 two circuits`, `ingest discovery`, `ingest states, captures`,
`product.CD2`, `ingest preflight`, `ingest assets, archives`, `product.KIS`,
`rag.N046 contract`, `rag.N046 filter`, `rag.N011 normalization`,
`rag.N052 quality`, `product.CD2 multimedia`, 08 §§11.3–11.4 and
`delivery.U10 (Q1–Q3, D1)`. Add explicit rows for adaptive configuration,
manifest-write authority, concurrent acquisition and evidence privacy; assign
N-task ownership without claiming completion. `ingest URLs, egress` keeps its
private-network prohibition until N48 lands the separately approved OA2 amendment.

### Code ownership and file layout

Paths below are planned unless named in the existing-seam table. Create modules
only with their first tested behavior; registration edits accompany that task.

```text
crates/maestro-acquisition/
  src/policy/          strict policy, identity, decisions, baseline resolution
  src/transport/       checked addresses, HTTP, robots, browser, aggregate permits
  src/capture/         immutable capture transaction and asset validation
  src/discovery/       partitions, links, change windows and repository selectors
  src/extraction/      registry, typed outputs, media adapters and fidelity
  src/isolation/       qualified platform subprocess controls
  src/adaptation/      inference, proposals, fixed gates, activation and rollback
  src/connector/       extension protocol, principal, sessions and wiki mappings
  src/lifecycle/       sync, resume, withdrawal, repair and cutover
  src/ports.rs         only the small consumer-facing contracts below
  tests/it/            one integration-test binary, synthetic fixtures/processes
crates/maestro-kernel/src/acquisition/  frontier, captures, scoped receipts/leases
crates/maestro-knowledge/src/import/   one shared mapped-ingestion transaction
crates/maestro/src/acquisition/        CLI binding, timers and authorized views
specs/006-native-acquisition/          spec.md, research.md, plan.md, tasks.md
```

Shared root dependency, module-door, CLI-registration, migration and guide edits
are serialized at landing. No vendor connector implementation or real URL registry
is created in this repository. N51 operates on owner-approved private bindings.

### Strict wire types and policy manifest schema

These are the executable v1 schema design for N03, not loose metadata suggestions.
All records reject duplicate/unknown keys at every nesting level, wrong types,
unknown enum values, unresolved references and duplicate IDs. Validate bounded JSON
before materializing objects. JSON Schema is generated from the same typed
contracts; a second hand-maintained validator must not drift.

| Primitive | Bound and meaning |
| --- | --- |
| `Id` / `Digest` | `Id`: 1–128 ASCII letters/digits/`-_.`, first character alphanumeric. Digest: exactly 64 lowercase hexadecimal SHA-256 characters. IDs are not filesystem paths. |
| `Ref` / `Time` | `Ref = {id: Id, digest: Digest}`; resource closure resolves exactly once. Time is UTC RFC 3339, compared using the authority's clock; leases additionally use monotonic local deadlines and fencing epochs. |
| `Text` / `Url` | Text ≤4,096 UTF-8 bytes unless a smaller bound applies; no NUL. URL ≤8,192 bytes, absolute HTTPS, no user information/control bytes. Signed transfer URLs exist only in protected transient transport memory. |
| Numbers | Integer counts/durations/bytes are unsigned 64-bit; positive unless zero explicitly means disabled/retries-none. CPU milli-cores, bytes, milliseconds and basis points are integer units. Fractions use numerator/positive denominator; no NaN/infinity or float ambiguity. |
| Containers | Configuration file ≤4 MiB, nesting ≤32, total object members/array elements ≤20,000; arrays ≤10,000, source/profile arrays ≤1,000. These parser ceilings never authorize a crawl of that size. Large receipt item inventories are paged digest artifacts, not unbounded JSON frames. |

All fields listed for a record are required; `?` means explicit null is allowed,
not a guessed default. Arrays may be empty only where the meaning is explicit
(e.g. no promotions, no auth hops). Common `Resource` fields are `schema`, `id`,
`version` (positive integer), `collection_id`, `visibility`, `scope_tags` and
`owner_ref`. Visibility/scope types reuse kernel conventions; never infer access
from a URL, media label or profile.

| Record / exact schema | Fields in addition to Resource, or complete fields for nested records |
| --- | --- |
| Collection link | Extend the existing collection declaration with a versioned, optional `source_policy: Ref`; old S1-only declarations remain valid but cannot acquire. Place it under the capability hierarchy in spec. The decoder's version change must preserve old read compatibility rather than silently add keys to old strict input. |
| `maestro-source-policy/1` | `sources: Source[]`, `registries: Ref[]`, `profiles: Ref`, `address_table: Ref`, `aggregate_limits: Limits`, `adaptation: AdaptationPolicy`, `retention_rule: Ref`, `qualification: Ref`. No secrets or machine paths. |
| `Source` | `id`, `origins: Origin[]`, `seeds: Url[]`, `discovery: Discovery[]`, `selectors: Selector[]`, `decisions: Ref[]`, `promotions: Ref[]`, `robots: Robots`, `limits: Limits`, `identity: IdentityRule`, `auth_role: Id?`, `connector: Ref?`, `wiki_mapping: Ref?`, `selected_profiles: Ref[]`, `sync: SyncPolicy`. |
| `Origin` | `id: Id`, `scheme` fixed `https`, `host` canonical DNS name, `port` 1–65535, `path_prefixes: Text[]`, `purpose` one of `content/authentication/asset`, `private_grant: Id?`. Match path boundaries after one unambiguous URL parse; encoded separators/ambiguous hosts refuse. |
| `Discovery` | Tagged union: `links {depth}`, `sitemap {url}`, `api {mapping: Ref}`, `repository {owner, repository, ref, paths, submodules, large_files}`. No keywords/guessed identifiers as enumeration. Repository lists are explicit selections, not booleans enabling everything. |
| `Selector` | `source_id`, `origin: Id?`, `path_prefix: Text?`, `object_ids: Id[]`, `versions: Text[]`, `channels: Id[]`, `media_types: Text[]`; conjunction of present fields, disjunction of list values. All-null/all-empty selector refuses; no scripts, regex execution or hidden precedence. |
| `maestro-source-decisions/1` | `entries: Decision[]`, `qualification: Ref`. Decision: `id`, `selector`, `action: deny_fetch/exclude_from_knowledge/asset_only`, `reason_code: Id`, `explanation: Text`, `authority: Ref`, `evidence: Ref[]`, `effective_at`, `review_at`, `expires_at: Time?`, `reversal: Ref?`. Expired automatic exclusions hold eligibility, never re-admit. |
| `maestro-source-promotions/1` | `entries` with decision fields but action fixed `promote_knowledge`. Separate approval only; promotion never defeats denial/robots/grant limits. |
| `Robots` / `IdentityRule` | Robots: `agent`, `override: Ref?`, `rules_max_bytes`, `cache_ttl_ms`. Identity: `version`, `meaningful_queries`, `ignored_tracking_queries`, `query_order: preserve/sort`, `repeated_queries: preserve/reject`, `fragment: discard_for_fetch`, `migration: Ref?`. Unknown query names refuse; display link remains distinct. |
| `SyncPolicy` | `mode: manual/one_off/watch`, `timer_period_ms: positive?`, `overlap_ms`, `clock_skew_ms`, `revision_fields: Text[]`; watch requires explicit finite cadence. Content cannot enable it. |
| `Limits` | Positive `requests`, `pages`, `partitions`, `redirects`, `depth`, `elapsed_ms`, `wire_bytes`, `dom_bytes`, `asset_bytes`, `staging_bytes`, `free_reserve_bytes`, `cpu_millicores`, `memory_bytes`, `source_runs`, `origin_concurrency`, `origin_interval_ms`; nonnegative `retries`, `max_backoff_ms`, `gpu_batches`, `gpu_bytes`; `decode: DecodeLimits`. Every effective limit is the minimum of host, grant, collection and run ceilings. Zero GPU disables GPU work. |
| `DecodeLimits` | Positive `expanded_bytes`, `expansion_ratio`, `nested_levels`, `members`, `decoded_pixels`, `elapsed_ms`, `memory_bytes`; `xml_entities` fixed `disabled`. One cumulative accounting object follows every HTTP/container/image/PDF/Office decode; no unchecked intermediate allocation. |
| `maestro-wiki-mapping/1` | Resource plus `origin_id`, `list_endpoint`, `item_endpoint`, `items_path`, `identity_path`, `parent_path`, `revision_path`, `content_path`, `content_kind: html/markdown/blocks`, `block_mapping: Ref?`, `attachments_path`, `permissions_path`, `permission_semantics: explicit_scopes/inherit_with_restrictions`, `pagination: Pagination`, `withdrawal: explicit_tombstone/complete_inventory`, `tombstone_path: FieldPath?`. Missing effective-permission semantics refuse. Endpoints are admitted relative paths, not executable templates; bounded literal/ID segment substitution only. |
| `FieldPath` / `Pagination` | FieldPath is an array of at most 32 typed `field {name: Text}` or `index {value: u32}` steps; total mapping nodes obey config limits. Pagination is `cursor {request_field, response_path, terminal_path}` or `next_link {response_path, terminal_path}`. Preserve entire typed continuation object; empty items is not terminal. Next links are always re-admitted. |
| `maestro-wiki-block-mapping/1` | Resource plus `blocks_path`, `block_id_path`, `parent_id_path`, `kind_path`, `text_path`, `children_path`, `kind_rules` mapping declared kind IDs to existing structural block kinds and typed payload paths. Unknown kinds remain explicit unsupported units; no prose synthesis, scripts or inferred permission inheritance. |
| `AdaptationPolicy` | `matrix: Ref`, `thresholds: Ref`, `baseline: Ref`, `minimum_sample`, `consecutive_runs`, `activation_interval_ms`, `automatic_classes` fixed subset of spec's five classes. Definitions, matrix, thresholds, cadence and budgets are protected. |
| `maestro-acquisition-manifest/1` | Resource plus `baseline: Ref`, `baseline_kind: local/catalog`, `proposals: Ref[]`, `activations: Ref[]`, `active: Ref`, `effective_digest`. Digest covers canonical baseline bytes plus ordered activation identities, not file whitespace. Baseline bytes are immutable; direct-file updates use atomic compare-and-swap. |

No absent budget receives an unbounded default. OA3 approval supplies the spec's
proposed operating values or an explicitly approved stricter/different envelope;
the schema itself grants none. Tests pin an envelope and exercise each limit.

### Data model, state and capture/receipt formats

Every persistent record binds collection visibility/scopes, owner/principal,
policy/registry/profile digests and immutable evidence references. Reuse kernel
artifact digests and scoped reads. The following records are authoritative rows
plus versioned JSON artifact payloads where a payload is large; artifacts do not
form a competing evidence store.

| Entity/schema | Required fields and invariants |
| --- | --- |
| `maestro-acquisition-run/1` | `run_id`, source/family, mode `full/incremental/resume/withdrawal/repair`, starting policy/effective config/authority-decision refs, job/lease epoch, baseline generation/source revision set, start/end, envelope, partition refs, terminal status, receipt ref. Resume creates an attempt under the logical run and rechecks current authority. |
| Frontier item | Key `(source_id, fetch_identity, authorization_context_digest, representation_profile_digest)`; `run_id`, partition, discovery parent/representation digest, state, attempts, validator/revision/link/permission digests, lease epoch/deadline, capture/refusal ref. A uniqueness constraint prevents duplicate in-flight identity/context; source-writer fencing rejects stale completions. |
| Partition | ID, enumeration kind, bounded cursor/window, overlap/skew, expected/observed item inventories, completion evidence, pending count, committed watermark. Cursor progression is not completeness. Accepted catalog snapshot and pending checkpoint are distinct. |
| `maestro-capture/1` | Capture ID, source/item/run, request/final stable identities, redacted hop records, status, allow-listed headers (`content-type`, safe length/validator values), detected/declared media, artifact digest/length, observed time, transport/profile IDs, representation `wire_body/rendered_dom/selected_html/api_record`, parent capture/member ref, access and decision refs. Unsafe validators/header values are hashed or omitted with a reason. Identity URLs are sanitized; no signed URL, token or cookie reaches provenance. |
| `maestro-extraction/1` | Attempt ID, capture ref, Markdown digest/length, typed `ExtractorBlock[]`, asset inventory, metadata, profile/tool/model refs, source locations (nullable), warnings/missing-content entries, detection evidence, fidelity ref, quality disposition. Raw media and extracted Markdown have different artifact identities. |
| `maestro-fidelity/1` | Attempt/source/output digests; measured and unknown structural inventories; correspondence pairs for headings, code, lists, table/row/cell/span, links/assets and media-specific units; exact literal mismatches, unsupported/lost units, applicable gold/profile refs, outcome and reasons. Counts alone never pass. Unknown source measurements remain null with reason. |
| `maestro-acquisition-receipt/1` | Unique immutable run/attempt ID, frozen inputs and current tightening decisions, partition inventory pages, distinct-item stage dispositions, attempt counters separately, budget usage, errors/holds and downstream revision/generation refs. Terminal `complete/partial/blocked/failed/cancelled`; only complete is successful completion. |
| Source alias/migration | Existing and replacement source refs/document IDs, evidence, versioned mapping, collection/access context, effective/reversal refs. Preserve byte-equal legacy `source_ref` by default; similarity or canonical HTML tags do not authorize identity changes. |
| Qualification/cutover | Frozen inventory/support matrix, suite/gold, input arms, actual tool artifacts, Q1–Q5 outcomes, per-item diff dispositions, independent reviewer, owner approval, producer switch epoch, scheduled refresh/rollback refs. Every family is separate; all approved families are required for M6. |

Frontier transitions are `pending → leased → captured → extraction_pending →
held | accepted | excluded | asset_only`; admission can produce `denied`, auth or
limits produce `blocked/pending`, proven lifecycle withdrawal produces
`withdrawn`. An unchanged result still verifies stored artifacts and discovers
links. Transient errors preserve retryable work; expired leases return eligible
work to pending, with a higher epoch. Captures commit before stage acknowledgement;
replay verifies/reuses immutable digests. No network occurs in SQLite writes.

### Profile registry and extractor port

`maestro-extraction-registry/1` has Resource fields, `profiles: Profile[]`,
`unknown_profile: Ref`, `qualification: Ref`. A Profile has `id/version`,
`definition_digest`, `detectors`, `structures`, `languages`, `extractor`,
`processing`, `decode_limits`, `output_schema`, `required_fidelity`,
`admission_rules`, `artifacts`, `platforms`, `qualification_state` and
`qualification_evidence`. Its definition is immutable and separately approved.

Detectors are a tagged union of bounded magic signature `(offset, hex_bytes)`,
container member signatures, declared-media hints and installed parser-capability
IDs. Structural/cleanup selectors use a bounded declarative AST: DOM tag/attribute
path, JSON field/index path, block kind, literal prefix, and boolean conjunction/
disjunction with explicit node/depth caps inherited from the config limits. No
executable snippets or unbounded regex. A new format registers a profile and, if
needed, a pinned Rust plug-in providing the parser capability; callers do not
change. Ties/contradictions produce unknown/held, not first-match trust.

Processing names approved cleanup IDs, selected S1 chunk strategy/profile and
dedup key IDs. Candidate selection compares **effective protected fields** of old
and new profiles, not names alone. Qualified definitions with different fidelity,
permission or admission settings cannot be swapped automatically. Profile state is
`proposed/qualified/held/revoked`; qualification binds exact platform/artifacts,
fixtures and owner-approved thresholds. Safe unknown profiles retain bounded assets
and obtainable text/metadata but are never implicit searchable acceptance.

Extractor request: capture artifact handle, profile ref, shared cumulative budget,
read-only scopes, deadline/cancellation. Result: `maestro-extraction/1` with a
separately stored fidelity receipt. Child output is untrusted, digest/span/asset
validated by core before admission. OCR/transcript/visual outputs carry derived
method, uncertainty and page/region/time evidence (or explicit unknown); no model
rewrite of original text. Extraction and builds run offline with provisioned assets.

### Small port contracts

Method names below are the consumer-facing contract to implement, not claims of
existing Rust APIs. DTOs are the records defined above; implementations may use
async futures without exposing a particular runtime to callers.

| Port | Operations and guarantees |
| --- | --- |
| `PolicySource` | `resolve(collection, principal) → CheckedPolicy`; same strict validator for local reviewed baseline and later catalog baseline. Validates digests/trust but neither creates grants nor activates connectors. |
| `Authority` | `decide(principal, operation, target, now) → Permit/Refusal`; read-only to pipeline. Owner command separately creates/revokes exact-scope grants with authenticated confirmation and audit. |
| `AdmittedTransport` | `fetch(lease, request, permit, budget) → CaptureCandidate/TypedFailure`; resolves/classifies every address/hop, pins checked address with hostname TLS validation, origin-binds credentials, ignores ambient proxies; bounded stream, cancellation and owned-process stop. Browser channels use this path or are blocked. |
| `Extractor` | `extract(capture_handle, profile, budget) → ExtractionAttempt`; capabilities are explicit, artifacts checked at each launch, no network/credential access. |
| `ConfigurationWriter` | `propose(expected_baseline, expected_active, proposal) → ProposalRef`; `activate(expected_baseline, expected_active, candidate, gate_receipt) → ActivationRef/Conflict`; `rollback(expected_active, previous, current_authority) → ActivationRef/Held`. All callers use the same port for direct-file and later catalog-backed baselines. |
| `ConnectorHost` | `activate(declaration, authority)`, `invoke(operation, bounded_input)`, `deactivate(reason)`; local supervisor first, S4 adapter later. Every invocation rechecks principal, current grant and lease epoch; stop reaps only owned processes. |
| `ScheduleTrigger` | `invoke(sync_request, principal) → RunRef`; CLI/manual and OS timer implementations call identical admission. Schedule is a trigger, not a second queue or trusted credential container. |

### Grant separation and process isolation

Live grants require an owner-authenticated authority command whose store writer
is unavailable to acquisition and connector tokens. Use a separately protected
local authority identity/store and authenticated local IPC; owner invocation
confirms scope/target/expiry and authenticates through the platform boundary.
Filesystem permissions while all processes share an unrestricted owner identity
are **not** sufficient. The setup probe must demonstrate failed create/edit/delete
from the actual pipeline and connector identities. Missing privileged setup or
platform authentication holds live work; synthetic authority adapters cannot be
used as a production fallback.

Linux uses the architecture's namespace/Landlock/seccomp and cgroup boundary;
Windows and macOS use qualified platform-specific restricted identities, filesystem,
network-denial and owned-process resource controls behind the same port. N17–N19
must demonstrate every requested effect restriction on real processes, record the
exact mechanism/library before adoption under N02, and refuse unsupported hosts.
A restricted token or child process alone is not a network sandbox. Platform
containment qualifies public parser workers too, not just private connectors.

Secrets normally stay in the transport behind opaque session handles bound to
run/account/origin. An exceptional approved secret-byte function uses only private
inherited authenticated IPC into non-dumpable, non-swappable memory, bounded
lifetime and zeroization. Failure to enforce any protection blocks that function;
secrecy/zeroize types alone are not proof. Owned browser profile/session storage
requires the same protection; no plaintext session export.

### Connector subprocess protocol

Reuse architecture 07's **Maestro Extension Protocol**, JSON-RPC 2.0 over stdio.
Messages are one UTF-8 JSON object per line, ≤1 MiB, depth ≤32, ≤64 in-flight
requests, unique bounded request IDs. Larger results are admitted artifact handles,
never raw content frames or connector-chosen host paths. Protocol-major mismatch,
unknown operation, malformed/duplicate keys, over-limit input or mismatched principal
refuses before effects. S4 must adopt these same bounds when it implements the port.

| Method | Request/result and control |
| --- | --- |
| `initialize` | Declared artifact digest, protocol/event/operation majors, kind and capabilities → negotiated supported subset plus assigned principal. Verify declaration/launch artifact before spawn; claimed capabilities cannot grant effects. |
| `ops.invoke` | Operation name, run/source/lease epoch, idempotency key and typed bounded payload. Supported S6 operations: `knowledge.frontier.lease`, `knowledge.transport.request`, `knowledge.capture.submit`, `knowledge.extraction.submit`, `knowledge.run.status`. Core checks every operation and all supplied handles. |
| `events.poll`, `events.ack` | Optional scoped progress cursor and acknowledgement; no private report/source content to notifiers. Durable kernel cursor, at-least-once delivery and idempotent acknowledgements, not a connector queue. |
| `health`, `shutdown` | Bounded status/cancellation handshake; deactivation revokes future dispatch immediately, closes protected handles, checkpoints pending work and terminates/reaps owned children within the approved shutdown deadline. |

`knowledge.capture.submit` accepts only a current leased item and a core-issued
staging handle, declared length/digest/representation plus safe metadata. Core
reads/validates the bytes before promotion; duplicate submission is idempotent,
stale epoch/substituted artifact/path refuses. Connector has no direct sockets,
DNS or ambient proxy route; the actual isolation boundary enforces this. Browser
WebSocket/service-worker/prefetch/WebRTC/DNS channels must be intercepted by the
checked-address route or disabled, with real bypass tests. No supported-channel
claim from an interception API's existence alone.

### Proposal, activation and privacy transaction

`maestro-processing-proposal/1`: Resource fields, `proposal_id`, expected baseline/
active refs, pinned sample/cohort/profile refs, observations plus uncertainty,
`changes: Change[]`, classified fields, candidate effective digest, gate/report refs,
state `proposed/held/qualified/active/rejected/superseded`, and rollback ref.
Change is typed, not arbitrary JSON Patch: `select_profile`, `set_cleanup`,
`set_s1_chunk_strategy`, `set_dedup_keys`, `add_knowledge_exclusion` or
`add_asset_only`. Unsupported edits can be recorded as held proposals for owner
review, but cannot be decoded as automatic changes. Existing exclusion edits,
indirect profile changes and mixed proposals hold in full.

N30's direct-file implementation stages immutable proposal/evidence artifacts,
then locks the collection, rereads expected baseline/active digests and performs
an atomic manifest replacement with durable recovery marker. The effective-manifest
pointer is the commit point; only after it commits is the journal notification
emitted. Recovery completes a missing notification or discards an uncommitted
staging record, never exposes a half-activation. The report is immutable and exists
before the pointer references it. Later catalog carriage changes baseline resolution,
not this write contract, and makes zero trusted-bundle writes.

Before activation revalidate baseline trust/current revocations, protected effective
fields, persistence/rate ledger and all six matrix columns on changed **and**
unchanged controls. Gate receipts bind candidate/baseline/suite/gold/profiles and
cohort digests; no suite or unauthorized N/A is inconclusive. Apply automatically
only at a safe run boundary, CAS the expected state, and keep in-flight processing
pinned while enforcing current revocations immediately. First post-activation
completed run repeats the matrix against the saved baseline **before publishing**;
failure restores previous still-authorized configuration and keeps the prior index,
otherwise holds for owner action. No further adaptation before this check; rollback
is exempt from the rate delay but cannot restore revoked content/grants.

All samples/proposals/drift/gate/activation reports inherit transitive collection
scope and live only in protected artifact storage. Authorized local inspection
resolves an access-checked opaque handle. Journal/log/notifier payloads contain
only fixed status codes and opaque handles, never private URLs/excerpts or a
purportedly sanitized report. Current access checks apply even to historical handles.

### S1 mapped-ingestion seam and identity continuity

N26 factors `import/entry.rs` into a shared `import/ingest.rs` core-side call:
`ingest_mapped(target, input) → imported/unchanged/held/refused`. Its input carries
verified Markdown bytes/digest, stable collection/source/source_ref, S1 metadata,
existing typed extractor blocks/assets, raw-capture and fidelity refs plus approved
quality disposition. Both corpus/1 import and S6 call this transaction; corpus/1
supplies empty structural additions, preserving existing behavior and identities.

The core independently checks artifact lengths/digests, span UTF-8 boundaries,
source mappings and asset references, calls `canonicalize` once and preserves the
existing revision/conflict/hold/journal path. Keep S1 `Revision.original_digest`
as its unchanged Markdown reference; raw PDF/HTML bytes are separately linked
capture artifacts, never relabelled Markdown. Capture-to-revision linkage is a
kernel relation, not an opaque extractor blob that callers cannot resolve.

Retain `doc- SHA256("collection\\0" + collection_id + "\\0" + source_ref)`
semantics and byte-equal legacy source_ref by default. Versioned reviewed
old-ID/new-ID mapping is required for identity changes; record alternate references
without merging permissions. A mapped PDF table/image test must resolve real supplied
page/cell/asset evidence all the way through stored canonical artifacts and chunks;
missing coordinates remain unknown. Exact groups retain separate occurrences and
permissions; near groups stay non-destructive. Prepared-cache reuse depends on
complete input/profile identity; metadata-only changes avoid needless embedding
but still produce the correct revision/access state.

N27/N40 serialize publication per collection using S1 compare-current generation
and the exact source-revision-set digest. Different source runs may harvest in
parallel; a stale candidate must reconcile/rebuild against the new set before
publishing. Search/ask continue pinning verified generations with current revocation
checks. Failure/cancellation leaves staging invisible and previous generation current.

## Phase 2: delivery and validation

### Incremental delivery

1. N01–N06 establish amendments, approval measurement, policy/authority/frontier
   and scoped receipts. N07–N14 deliver **US1 public-docs MVP**: manual preview,
   admitted HTTP, durable discovery/capture and truthful completion without a host.
   It is not a claim of embedding, browser/private support or M6.
2. N15–N28 deliver US2 media receipts, bounded native extraction and the shared S1
   seam. Public HTML can feed S1 as soon as its dependencies qualify; other media
   need their own required-cohort evidence, not a universal-support claim.
3. N29–N35 implement US3 inference, proposals and automatic gated activation;
   N36–N42 implement US4 lifecycle, resource-safe concurrency and local timers.
4. N43–N48 implement US5 local connectors, sessions, browser/wiki and separately
   approved private origins. N49–N54 qualify US6 family parity/retrieval/cutover.
5. N55 coordinates later S3 JSON consumer and S4 adapter conformance, outside M6's
   critical path. Their real delivery waits for those slices; synthetic baseline/
   port substitution tests required for S6 are already in N03/N30/N43/N42.

### Quickstart validation guide

These are **planned commands**, supplied by N14/N42 and task integration tests;
they do not exist at the planning baseline. `$MAESTRO_BIN`, `$FIXTURES` and
`$SCRATCH` are approved local bindings, not embedded machine paths. Synthetic
transport/process harnesses use explicit test adapters; production admission
must not gain a loopback/private-address testing exception.

```bash
export CARGO_BUILD_JOBS=3
capped cargo test -p maestro --test it n14_ -- --nocapture
"$MAESTRO_BIN" knowledge acquire preview --manifest "$FIXTURES/public/manifest.json"
"$MAESTRO_BIN" knowledge acquire sync --manifest "$FIXTURES/public/manifest.json" --mode full
"$MAESTRO_BIN" knowledge acquire inspect --run "$RUN_ID"
capped cargo test -p maestro-knowledge --test it n26_ -- --nocapture
```

The CLI example requires an approved fixture HTTPS origin and real owner grant;
without either it must refuse, not silently use synthetic authority. Unit/integration
commands inject the fixture transport and require zero calls to denied destinations.
For scratch embedding use an already qualified S1 model/generation profile and
approved resources; absent model access is a reported blocker, not a mock pass.

| Story | Independently observable result |
| --- | --- |
| US1 | Allowed public page/attachment captured once, denied neighbour receives zero requests, all discoveries have dispositions, host absent, partial windows never return successful completion. |
| US2 | Exact code/table/asset/location receipts across required media; held neighbours stay out; repeat input yields the same S1 identities/prepared hashes and failed publication leaves old generation. |
| US3 | Two unrelated source samples yield evidence/unknown for every inference field; all five classes have pass cases and every matrix/forbidden-field failure holds; conflict/restart/post-check rollback remains scoped. |
| US4 | Full/incremental/resume/withdrawal/repair match uninterrupted reference; missing repo/asset repaired, temporary auth loss not deleted, overlapping runs preserve updates and interactive floors. |
| US5 | Two isolated synthetic account/collection bindings, real child network-denial and session canaries, two declarative wiki fixtures, private-origin positive case only after OA2 and exact grant. |
| US6 | Frozen twelve-defect/media inventory, explained fixture/live differences, Q5 passing per family and combined, single writer, observed scheduled refresh and revocation-safe rollback before M6. |

### Qualification, cutover and gates

Freeze Q1–Q5 exactly as specified, with owner approval **before** results. Required
media and all twelve historical regressions cannot be removed after failures.
N49 maps R1–R12 from research §7.2 to executable tests, including source-grounded
legacy corrections rather than blind parity. N51 covers all five lifecycle modes
with approved live strata and safe fixtures where destructive live changes are
not permitted; unavailable evidence holds that family.

N52 extends the existing S1 evaluator, not a new scoring implementation. Pin
actual current generation, suite/gold v2.2 where applicable, relevance semantics,
access, aliases, models/runtime/hardware and route/final metrics. Deterministic
arms require metric equality overall and each predefined nonempty stratum. For
nondeterminism use the **owner-approved** delta (recommended 0.01), three paired
repetitions and seeded 2,000 question-level bootstrap resamples as spec proposes;
lower 95% bound ≥−delta, except false-abstention upper bound ≤delta. Report every
lost question for independent source-grounded disposition without changing gold,
removing questions or excusing aggregate failure. Changing S1 baseline before
cutover invalidates the comparison and requires both arms rerun.

N53 switches one approved family only, fences the old writer, observes one native
scheduled refresh and one rollback exercise, and retains two verified generations
subject to current revocation/retention. N54 verifies every approved family and
combined-generation Q5, actual native artifact inventory, independent review,
three-platform behavior, coverage/mutation totals and release controls. No private
connector/content/question/receipt is copied into public CI or this spec set.

All code tasks run focused red/green tests, workspace tests before push, formatting,
org-configured three-target Clippy, strict public/private rustdoc, guide,
architecture, duplication and licence checks; dependency changes also run vet.
CI, not this workstation, runs coverage and mutants. Document-only N01 uses
traceability assertions, Markdown/links/spelling/privacy checks and normal hooks.
Actual platform/browser evidence and live approvals are additional gates, not
replaced by cross-target lint or synthetic tests.

## Complexity and unresolved adoption status

The new library separates acquisition from an already substantial knowledge crate;
the kernel retains state and S1 retains indexing. Ports exist only for requirements
that need replaceable local/later adapters. No graph, term-alias engine, adaptive
statistics package, scheduler runtime, converter chain or corpus/2 format is added.
N09 introduces transport-side cumulative decode accounting; N16 extends that same
accounting contract across parser/asset IPC rather than introducing a second meter.
N33–N35 can land against synthetic gate receipts before every real matrix producer
exists: live activation remains held until N28, lifecycle/concurrency qualification,
N49 and N52 supply actual required evidence. This separates code dependencies from
runtime qualification and never permits a synthetic gate receipt in production.
N02/N17–N22 may discover a library/containment infeasibility; report the affected
adapter as blocked and request a design ruling, never weaken the spec. Pending
owner receipts are explicit execution blockers, not unanswered schema decisions.
