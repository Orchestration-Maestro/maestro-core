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
reqwest, Tokio, Markdown and model ports. Owner-selected tools still need exact
artifact/feature audits and qualification; other new libraries remain approval-blocked.

**Spec:** [spec.md](spec.md), Revision 2.3, 62 FRs and 15 SCs.
**Research:** [research.md](research.md), Revision 2.2 (dated owner decisions and validation fixes), sources accessed 2026-09-30.
**Tasks:** [tasks.md](tasks.md).
**Branch:** `docs/s6-spec-set`, from S1 `0204846f7640e7219021b0b38daa60b4c4f98ee4`;
the supervisor creates `feat/s6-integration` after review.
**Status:** revised design after validation and the owner's 2026-09-30 decisions;
not implementation, live authority, qualification results or independent approval.
Rust owns producers, extraction and connectors. Only unavoidable browser rendering
uses crawl4ai (Python, out of process) under the named ADR-0020 exception below;
that exception is not a Python producer/extractor/connector fallback.
**Alias scope approved, 2026-09-30:** ingestion automatically learns term aliases
(other names for the same thing) from explicit source definitions, with supporting
spans. Candidates are reviewable/reversible, never hand-written product lists or
silent merges. Preserve an S2 `ALIAS_OF`-compatible identity seam; search-time query
expansion over approved aliases is a later S1 item, not S6 implementation.
Source-reference continuity and publication aliases remain separately required.
Xberg 1.x (MIT) and the unchanged native docling bake-off are approved directions,
not a measured winner or blanket feature approval.
**Catalog amendment, 2026-09-30:** N56 adds the catalog-backed policy/resource
adapter (6 estimated lane-hours); N55 retains integrated write-port/S4 conformance
(6 h). N15/N30 keep only N03/N06 as predecessors and start with `DirectFiles`
plus test-only synthetic catalog fixtures, without waiting for N56 or any S3 task.
**Adaptation contracts, 2026-10-01:** N57 adds the missing typed artifacts and
snapshot reader (8 h), N29 adds candidate persistence (+2 h), and N34 adds the
fresh authority adapter (+2 h). Total: **57 tasks / 345 h**, of which
**55 M6-path tasks / 333 h** and **2 later tasks / 12 h**. The +12 h is a
planning estimate; no delivery or qualification is claimed. The recomputed
MVP/M6 precedence bounds remain 43/95 h; see tasks.md for the paths.

## Global constraints

- The S6 M6 path depends only on S1 delivery, not M1 release or S3/S4 delivery.
  Later N56 consumes explicit S3 handoffs; N55 also waits for S4. Missing
  downstream qualification evidence still blocks the gate that needs it.
- The manifest is processing-configuration authority; it cannot grant access.
  Source-owned URL rules are strict JSON data under `knowledge/sources/<name>/`,
  not collection-local copies or per-site engine code. Questions do not crawl.
  Grants require a separate owner-authenticated writer.
- First-party producers, extractors and connectors are Rust. The sole non-Rust
  browser adapter is out-of-process crawl4ai for unavoidable Chromium work;
  native components need named OA5/ADR-0020 records. No implicit download or hidden fallback.
- Preserve source bytes, permissions and losses. Unknown is not zero or passed;
  held, partial and unsupported outputs are not searchable admission.
- Only the five automatic change classes in spec §Fixed mandatory gate matrix
  are eligible. Every cell is mandatory; missing retrieval evidence holds.

## Technical context

| Area | Choice and constraint |
| --- | --- |
| Workspace | New `maestro-acquisition` library over `maestro-kernel` and `maestro-knowledge`; existing `maestro` CLI binds ports. No separate daemon/database, general extension runtime or agent-driven crawler. |
| Storage | Existing kernel SQLite WAL, artifacts, jobs and journal; new frontier/capture/receipt records are kernel-owned. Network, conversion and models run outside transactions. Forward-only migration gets the next free number at landing, not a reserved number here. |
| Dependencies | Reuse root Cargo versions, including reqwest 0.13.5, pulldown-cmark 0.13.4, whatlang 0.18.0 and serde. The dated owner shortlist below is approved; N02 audits exact features/artifacts before adoption. Unselected robots, codecs and containment helpers still need owner approval. No dependency is added by this plan. |
| Platforms | Linux, Windows and macOS. Unit contracts run everywhere; actual browser/process containment and offline extraction must be tested on each claimed environment. An unavailable control blocks that adapter, never chooses an unsandboxed implementation. |
| Tests | Rust unit and existing single-binary integration suites, independently authored synthetic fixtures, real child-process denial/crash tests, separately approved local private comparisons. Tests first; code coverage/mutation gates run in CI. |
| Performance | No invented crawl-throughput target. Enforce approved OA3 budgets and inherited S1 release/interactive latency floors. Report observed CPU/RAM/VRAM/storage, pending work and cold/warm latency under named hardware/configuration. |
| Scale | Owner-frozen families/media/strata, not a guessed corpus size. OA3's approved defaults are inputs, not measured capacity; pending OA4c bounds every live sample. Parser/protocol ceilings below are independent defensive format limits. |
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

### Owner tool decisions — 2026-09-30

The owner answered yes to the shortlist, with the amendment: “rust only except we
have no choice for js page or chromium use python alternative like crawl4ai”.
N02 records that named ADR-0020 exception before N46 implements the render adapter.

| Adoption group | Approved direction; exact audits and qualification still required |
| --- | --- |
| Crawl/browser | Spider for crawling with its extras/defaults switched off; reuse reqwest and kernel-owned leases. For unavoidable JavaScript/Chromium work use **crawl4ai**, Python out of process behind `AdmittedTransport`, not Spider's chromey path. Pin browser/runtime/adapter artifacts and allow only declared browser capabilities. No chromey/chromiumoxide fallback or second production browser stack. [research §§2, 3.1; P1, P2, P27] |
| Technical HTML | htmd for HTML to text, checked against Xberg's built-in `html-to-markdown-rs`; site selectors preserve technical structure. No converter chain. dom_smoothie remains an unselected, separately approved fallback candidate. [research §3.2; P9–P11, P27] |
| Documents/OCR | Xberg 1.x (MIT) against the native docling.rs converter on our files; adopt the qualified winner per profile. Keep the researched pins/baselines unchanged: Xberg 1.3.0 minimal native PDF/Rust layout/OCR versus `docling` 1.78.0 `default-features = false, pdf-text`, with separately approved ML comparison only. The owner's “docling-rs” refers to this bake-off, not the same-named service SDK. **tract by default for layout/OCR; ONNX Runtime only for table models, loaded dynamically and never auto-downloaded.** Pin weights offline and check every weight licence before use. [research §3.3; P5, P6, P24–P28] |
| Spreadsheet/detection/helpers | calamine 0.36.1 for cells plus separate formulas; reuse Xberg's infer if selected, otherwise evaluate infer 0.22.0. Reuse whatlang with abstention; evaluate texting_robots 0.2.2 against RFC 9309 and minimum ZIP/TAR codecs. No tree_magic_mini embedded database, new language detector or ADWIN without a demonstrated need and approval. [research §§3.4–3.5; D1–D5, P16, P20–P21] |
| Sessions/native assets | Existing read-only KeePassXC mechanism remains a separately approved external component, not plaintext unlock files. keyring/keepass/secrecy/zeroize only if needed and approved. PDFium/full docling ML remains an optional comparison, not a new production exception. Xberg/docling feature edges enabling `download-binaries` must be split upstream or by an approved audited patch before adoption; adding dynamic loading alone does not remove additive download features. [research §§3.3, 3.7–3.8, 6; P17–P19, P23, P27] |

N02 records exact features, added packages, duplicates, native links, licence and
model-weight terms against the then-current lock. Research's historical counts
are not current measurements. Approved exceptions name the forcing library,
role, artifact digest, scope and removal condition. No speculative alternate
engine is adopted simply because the research lists it.

### Approval ledger and blocked effects

| Approval | Owner decision — 2026-09-30 | Remaining boundary |
| --- | --- | --- |
| OA1 | **Approved:** Q1–Q5, fixed matrix/protocol, deterministic equality or nondeterministic **δ = 0.01**. | Freeze suite/cohorts/baseline before results; missing or failed evidence still holds qualification/activation/cutover. |
| OA2 | **Approved:** internal wikis one named origin at a time, owner approval and expiry. | N48 lands the architecture amendment; each exact-origin authenticated grant remains necessary. No global allowance. |
| OA3 | **Approved:** spec's operating envelopes as proposed, unchanged. | Enforce them; source-specific variations need approval. Capacity is not measured and GPU scheduling remains separately authorized. |
| OA4a/OA4b | **Approved:** private vendor connector one source family at a time; first preflight one source/account/target, 180 seconds, two protected reads, one login cycle, zero retries; no robots override by default. | Exact origins/seeds/entitlement/account and expiring source grants still need the owner-authenticated command; exceptional robots overrides need their own receipt. |
| OA5 | **Approved:** required M6 file/media scope and the tool shortlist above, including only the named crawl4ai browser exception. | N02 records exact artifact/feature/licence/exception evidence; winner selection and profile/platform/model qualification still need results. Unselected dependencies/exceptions need approval. |
| Alias | **Approved:** automatically learned source-defined term-alias candidates with supporting spans, reviewable/reversible, no hand-written product list or silent merge. | N29 records candidates and the S2-compatible identity seam; approved-alias query expansion is later S1 work only. |
| OA4c / OA4d | **Pending:** private data comparison use / each family's retirement. | No private comparison or retirement without its own receipt. |

## Constitution check

The workspace-installed `.specify/memory/constitution.md` delegates to the
organization rules and repository [rule map](../../docs/standards/engineering.md).
The owner-approved crawl4ai exception must be recorded by N02; no broader exception
is granted here. Scope and downstream reuse are explicit; private reads are not
needed for planning. Effects remain blocked at named grant/qualification boundaries,
and the newly approved alias-candidate scope remains evidence-bound. A planned
enforcing test is not a passing test.

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
| ENF-010 | N03 single manifest schema, N30–N35 proposal/CAS/persistence contracts; N05 grants remain separate authority; N56 resolves immutable catalog resources and N55 proves integrated writes never rewrite trusted bundles. |
| SEC-010–011 | N54 checks existing private disclosure and signed-release/checksum/SBOM workflow evidence; N43/N53 verify launched/cutover artifacts against it, not a checksum-only trust claim. |
| COV-001–002, TST-001 | Every implementation task records deterministic red/green tests; N54 requires CI ≥90% overall and ≥95% changed-line coverage, zero missed mutants and zero mutation timeouts. Network/model/private evidence cannot be silently skipped. |

## Phase 1: architecture, data model and contracts

### Architecture amendments before code

N01 is a documentation-only prerequisite to every S6 code task. It changes all
four decided locations and corresponding 08 rows together, and records the
2026-09-30 browser amendment in 01 §2.2.2 and 06/08's native-cutover wording:
crawl4ai replaces Spider's chromey path only for unavoidable browser work. N02
records its named ADR-0020 role/artifact exception; Rust still owns the producer:

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
  src/policy/          strict policy, identity, decisions, baseline validation
  src/files.rs        DirectFiles local resource/policy adapter
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
  catalog.rs                         N56 CatalogSource composition adapter
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
| `maestro-source-policy/1` | `sources: Source[]`, `registries: Ref[]`, `profiles: Ref`, `acquisition_profiles: Ref[]`, `address_table: Ref`, `aggregate_limits: Limits`, `adaptation: AdaptationPolicy`, `retention_rule: Ref`, `qualification: Ref`. No secrets or machine paths. |
| `Source` | `id`, `origins: Origin[]`, `seeds: Url[]`, `discovery: Discovery[]`, `selectors: Selector[]`, `decisions: Ref[]`, `promotions: Ref[]`, `robots: Robots`, `limits: Limits`, `identity: IdentityRule`, `auth_role: Id?`, `connector: Ref?`, `wiki_mapping: Ref?`, `acquisition_profile: Ref`, `selected_profiles: Ref[]`, `sync: SyncPolicy`. |
| `Origin` | `id: Id`, `scheme` fixed `https`, `host` canonical DNS name, `port` 1–65535, `path_prefixes: Text[]`, `purpose` one of `content/authentication/asset`, `private_grant: Id?`. Match path boundaries after one unambiguous URL parse; encoded separators/ambiguous hosts refuse. |
| `Discovery` | Tagged union: `links {depth}`, `sitemap {url}`, `api {mapping: Ref}`, `repository {owner, repository, ref, paths, submodules, large_files}`. No keywords/guessed identifiers as enumeration. Repository lists are explicit selections, not booleans enabling everything. |
| `Selector` | `source_id`, `origin: Id?`, `path_prefix: Text?`, `object_ids: Id[]`, `versions: Text[]`, `channels: Id[]`, `media_types: Text[]`; conjunction of present fields, disjunction of list values. All-null/all-empty selector refuses; no scripts, regex execution or hidden precedence. |
| `maestro-source-decisions/1` | `entries: Decision[]`, `qualification: Ref`. Decision: `id`, `selector`, `action: deny_fetch/exclude_from_knowledge/asset_only`, `reason_code: Id`, `explanation: Text`, `authority: Ref`, `evidence: Ref[]`, `effective_at`, `review_at`, `expires_at: Time?`, `reversal: Ref?`. Expired automatic exclusions hold eligibility, never re-admit. |
| `maestro-source-promotions/1` | `entries` with decision fields but action fixed `promote_knowledge`. Separate approval only; promotion never defeats denial/robots/grant limits. |
| `Robots` / `IdentityRule` | Robots: `agent`, `override: Ref?`, `rules_max_bytes`, `cache_ttl_ms`. Identity: `version`, `meaningful_queries`, `ignored_tracking_queries`, `query_order: preserve/sort`, `repeated_queries: preserve/reject`, `fragment: discard_for_fetch`, `migration: Ref?`. Unknown query names refuse; display link remains distinct. |
| `SyncPolicy` | `mode: manual/one_off/watch`, `timer_period_ms: positive?`, `overlap_ms`, `clock_skew_ms`, `revision_fields: Text[]`; watch requires explicit finite cadence. Content cannot enable it. |
| `Limits` | **Ceilings (`min`):** positive `requests`, `pages`, `partitions`, `redirects`, `depth`, `elapsed_ms`, `wire_bytes`, `dom_bytes`, `asset_bytes`, `staging_bytes`, `cpu_millicores`, `memory_bytes`, `source_runs`, `origin_concurrency`; nonnegative `retries`, `max_backoff_ms`, `gpu_batches`, `gpu_bytes`. **Floors (`max`):** positive `free_reserve_bytes`, `gpu_reserve_bytes`, `origin_interval_ms`. Nested `decode: DecodeLimits` composes by its own kinds. Zero GPU disables GPU work. |
| `DecodeLimits` | **Ceilings (`min`):** positive `expanded_bytes`, `expansion_ratio`, `nested_levels`, `members`, `decoded_pixels`, `elapsed_ms`, `memory_bytes`. `xml_entities` is an exact invariant fixed `disabled`, not a numeric bound. One cumulative accounting object follows every HTTP/container/image/PDF/Office decode; no unchecked intermediate allocation. |
| `maestro-wiki-mapping/1` | Resource plus `origin_id`, `list_endpoint`, `item_endpoint`, `items_path`, `identity_path`, `parent_path`, `revision_path`, `content_path`, `content_kind: html/markdown/blocks`, `block_mapping: Ref?`, `attachments_path`, `permissions_path`, `permission_semantics: explicit_scopes/inherit_with_restrictions`, `pagination: Pagination`, `withdrawal: explicit_tombstone/complete_inventory`, `tombstone_path: FieldPath?`. Missing effective-permission semantics refuse. Endpoints are admitted relative paths, not executable templates; bounded literal/ID segment substitution only. |
| `FieldPath` / `Pagination` | FieldPath is an array of at most 32 typed `field {name: Text}` or `index {value: u32}` steps; total mapping nodes obey config limits. Pagination is `cursor {request_field, response_path, terminal_path}` or `next_link {response_path, terminal_path}`. Preserve entire typed continuation object; empty items is not terminal. Next links are always re-admitted. |
| `maestro-wiki-block-mapping/1` | Resource plus `blocks_path`, `block_id_path`, `parent_id_path`, `kind_path`, `text_path`, `children_path`, `kind_rules` mapping declared kind IDs to existing structural block kinds and typed payload paths. Unknown kinds remain explicit unsupported units; no prose synthesis, scripts or inferred permission inheritance. |
| `AdaptationPolicy` | `matrix: Ref`, `thresholds: Ref`, `baseline: Ref`; positive **floors (`max`)** `minimum_sample`, `consecutive_runs`, `activation_interval_ms`; `automatic_classes` fixed subset of spec's five classes. Definitions, matrix, thresholds, cadence and budgets are protected. |
| `maestro-acquisition-manifest/2` (N57 amendment) | Resource plus `baseline: Ref`, `baseline_kind: local/catalog`, `processing_baseline: Ref`, `proposals: Ref[]`, `activations: Ref[]`, `active: Ref`, `effective_digest: Digest`. N30's baseline/activation lineage remains; the new pin names the initial scoped processing snapshot. The typed effective preimage below includes that pin. Baseline bytes are immutable; the existing atomic pointer/recovery path remains. |

Compose each bound across host, grant, collection, source/origin and run by its
kind, never by precedence order: **ceilings use `min`; floors use `max`**. The
same rule applies to any additional “at least” constraint, including required
server delay and minimum timer cadence/overlap/skew windows. Other declared
settings/references remain exact validated values, not numbers to minimize.
Robots `rules_max_bytes` and `cache_ttl_ms` are ceilings. N10 mixes 1,000/100 ms
interval floors with 1/4 request-concurrency ceilings: result 1,000 ms and 1.
A server delay exceeding the backoff/elapsed ceiling leaves work pending; never
shorten the delay. N11 mixes 30/10 GiB disk-reserve floors with 20/10 GiB staging
ceilings: result 30 GiB and 10 GiB. GPU headroom subtracts the effective reserve
floor (OA3: 2 GiB), clamped at zero, before applying the `gpu_bytes` ceiling.
Impossible combinations hold/pause, not relax one side. All compose identically
regardless of input order; current stricter controls also apply on resume.

No absent budget receives an unbounded default. OA3's 2026-09-30 approval supplies
the spec's operating values; variations need explicit approval. The schema grants
no access. Tests pin an envelope and exercise each field's kind.

### Acquisition profiles and declarative browser readiness

| Record / exact schema | Complete fields beyond Resource, or nested fields |
| --- | --- |
| `maestro-acquisition-profile/1` | `transport: http/browser_request/browser_render`, `adapter: Ref`, `required_capabilities: Id[]`, `readiness: Readiness?`, `qualification: Ref`. The adapter ref pins an installed declaration whose digest binds executable/runtime/browser artifacts, supported transports and platforms. No source-provided executable path. |
| `Readiness` | `document_state: dom_content_loaded/load`, `all_of: ReadyCondition[]` (1–32 conditions), positive `timeout_ms`, `poll_interval_ms`, `stable_for_ms`. `poll_interval_ms ≤ timeout_ms` and `stable_for_ms ≤ timeout_ms`. Conditions must remain true throughout the stability window; completion or failure is bounded by the effective run/transport deadline. |
| `ReadyCondition` | Tagged union: `element_present {selector: DomPath}`, `element_text {selector: DomPath, contains: Text}`, `element_absent {selector: DomPath}`. `DomPath` has 1–32 steps of `tag: Id`, `attributes` (0–8 exact `{name: Id, value: Text}` pairs); direct-child paths only, no executable CSS/JavaScript/regex. Text comparisons are literal. |

Every Source's `acquisition_profile` resolves by ID **and digest** within
`acquisition_profiles`; the run, request and capture bind that digest. HTTP and
browser-request profiles require `readiness: null`; browser-render requires the
bounded record. Timeout is a ceiling (`min`); minimum poll/stability requirements
are floors (`max`). Invalid bounds, unsupported predicates/capabilities, disabled
or unqualified adapters and substituted artifacts refuse before launch. Readiness
failure is typed pending/held, never a successful partial document; network-idle
alone cannot qualify readiness. No scripts, arbitrary callbacks or runtime downloads.

Composition injects the adapter behind `AdmittedTransport`; changing only an
approved profile/reference selects an installed qualified adapter without caller
edits. The production crawl adapter is Spider with extras off; the sole non-Rust
browser adapter is crawl4ai out of process for unavoidable Chromium work. A
`browser_request` capability must actually use Chromium's network stack and pass
N46, or refuse; it is not an HTTP impersonation fallback. Synthetic substitute
and disabled adapters prove replacement without approving another production
stack. Acquisition transport, adapter and readiness definitions/references are
protected, **not** the automatically mutable extraction `selected_profiles`.

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
| `maestro-term-alias-candidate/1` | Resource plus `term: Text`, `expansion: Text` (both nonempty), `language: Text?`, `context: Ref`, `evidence` (nonempty capture/revision refs with exact supporting spans and profile digest), `relation` fixed `ALIAS_OF`, `state: proposed/approved/rejected/revoked`, `review: Ref?`, `supersedes: Ref?`, `reversal: Ref?`. Immutable state-change records retain reviewer/evidence and current scope; approved/rejected/revoked transitions require explicit review, never model inference alone. This is a candidate interchange seam compatible with S2's reviewed `ALIAS_OF` records, not a second graph or an identity merge. |
| Qualification/cutover | Frozen inventory/support matrix, suite/gold, input arms, actual tool artifacts, Q1–Q5 outcomes, per-item diff dispositions, independent reviewer, owner approval, producer switch epoch, scheduled refresh/rollback refs. Every family is separate; all approved families are required for M6. |

Frontier transitions are `pending → leased → captured → extraction_pending →
held | accepted | excluded | asset_only`; admission can produce `denied`, auth or
limits produce `blocked/pending`, proven lifecycle withdrawal produces
`withdrawn`. An unchanged result still verifies stored artifacts and discovers
links. Transient errors preserve retryable work; expired leases return eligible
work to pending, with a higher epoch. Captures commit before stage acknowledgement;
replay verifies/reuses immutable digests. No network occurs in SQLite writes.

### Profile registry and extractor port

Callers consume the `ProfileRegistry` port below, never the registry's storage
or concrete detector implementation. `resolve` validates the exact immutable
registry and its qualification closure for the principal; `select` consumes that
checked handle, bounded detection/structure evidence and the policy's eligible
extraction-profile refs. It returns a digest-bound selected profile with reasons,
or explicit unknown/ambiguous held outcome. Core revalidates returned refs and
protected effective fields against the checked policy; an adapter cannot grant
qualification or authority. Neither operation performs acquisition or model loads.
A substitute must pass the same contract. A disabled adapter returns
`RegistryUnavailable(disabled)` from both operations: no implicit default,
extractor launch, network call or searchable admission. Existing retained captures
remain inspectable under current access, but dependent acquisition is blocked.

`maestro-extraction-registry/2` (N57's unreleased-format amendment) has Resource fields, `profiles: Profile[]`,
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

N57 changes the existing N15 `Processing` in place to
`{cleanup: Ref, chunk: Ref, dedup: Ref}`: one immutable rule-set/strategy/key-set
pin per group, never an ID-only lookup or two competing representations. These
are each profile's defaults. Collection selections override them group by group
only through the exact resolution below. Candidate selection compares
**effective protected fields** of old and new profiles, not names alone. Qualified definitions with different fidelity,
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

`PolicySource` and `ResourceSource` below are the existing read-only Rust ports
at S6 `f9a57e5`, `crates/maestro-acquisition/src/ports.rs:54–91`; their signatures
stay unchanged. Other method names are planned consumer-facing contracts, not
claims of existing Rust APIs. DTOs are the records defined above; implementations
may use async futures without exposing a particular runtime to callers.

| Port | Operations and guarantees |
| --- | --- |
| `PolicySource` | `resolve(&Declaration, &Principal<'_>) → Result<CheckedPolicy, Refusal>`; `DirectFiles` and N56 `CatalogSource` delegate to the same `policy::resolve::validate`. Neither creates grants nor activates connectors. |
| `ResourceSource` | `read(&Ref, &Principal<'_>) → Result<ImmutableResource, Refusal>`; bounded original bytes, exact reference and separate admission evidence, never a policy-supplied path. The shared validator rechecks identity/digest/review/platform and the typed closure. N56 additionally verifies installed catalog trust, ownership and current access before returning resources. |
| `ProfileRegistry` | `resolve(registry_ref, principal) → CheckedRegistry/RegistryUnavailable`; `select(checked_registry, bounded_evidence, eligible_refs) → ProfileSelection/RegistryUnavailable`. `ProfileSelection` is `selected {profile: Ref, evidence: Ref[]}` or `held {reason: unknown/ambiguous, safe_profile: Ref, evidence: Ref[]}`; unavailable reasons include disabled/missing/corrupt/unqualified. Same pure selection and validation contract for local, substitute and disabled adapters. |
| `Authority` | `decide(principal, operation, target, now) → Permit/Refusal`; read-only to pipeline. Owner command separately creates/revokes exact-scope grants with authenticated confirmation and audit. |
| `AdmittedTransport` | `fetch(lease, request, acquisition_profile, permit, budget) → CaptureCandidate/TypedFailure`; checked digest-bound profile selects the injected adapter and bounded readiness; resolves/classifies every address/hop, pins checked address with hostname TLS validation, origin-binds credentials, ignores ambient proxies; bounded stream, cancellation and owned-process stop. Browser channels use this path or are blocked. |
| `Extractor` | `extract(capture_handle, profile, budget) → ExtractionAttempt`; capabilities are explicit, artifacts checked at each launch, no network/credential access. |
| `ConfigurationWriter` | `propose(expected_baseline, expected_active, proposal) → ProposalRef`; `activate(expected_baseline, expected_active, candidate, gate_receipt) → ActivationRef/Conflict`; `rollback(expected_active, previous, current_authority) → ActivationRef/Held`. All callers use the same port for direct-file and later catalog-backed baselines. |
| `ConnectorHost` | `activate(declaration, authority)`, `invoke(operation, bounded_input)`, `deactivate(reason)`; local supervisor first, S4 adapter later. Every invocation rechecks principal, current grant and lease epoch; stop reaps only owned processes. |
| `ScheduleTrigger` | `invoke(sync_request, principal) → RunRef`; CLI/manual and OS timer implementations call identical admission. Schedule is a trigger, not a second queue or trusted credential container. |

### Catalog-backed policy and resource adapter (N56)

The owner decision fixes source-rule placement relative to the owning manifest
area: `knowledge/sources/<name>/source.toml` inventories `policy.json`,
`decisions/<name>.json`, `promotions/<name>.json` and `migrations/<name>.json`.
They carry core's strict JSON policy/decisions/promotions/migration types; preserve
optional companions and original-byte digests. Collections keep only their strict
`knowledge/collections/<name>/collection.json`, catalog metadata and exact policy
reference. Rules are data, never site-specific code. S3 C52a/C52b publish schemas
from those core types into `schemas/source-2/`; C66 supplies synthetic neighbours
and C68 checks schema/fixture drift. N56 does not copy parsers or generate a
competing schema inventory.

**External handoff, not shared implementation ownership:** S3
`30b702b:specs/003-catalog/plan.md:1509–1573` and its task contracts define:

| S3 predecessor (exact task name) | N56 consumes; S3 keeps ownership |
| --- | --- |
| C41 S6 collection descriptor contract | Strict collection declaration, owner-relative placement and exact source-policy reference; no duplicate collection-local rules. |
| C43 S6 source-rule admission and provenance contract | Checked collection/source/rule closure, signed ownership-review evidence, current admission and private provenance contract. |
| C66 Knowledge sources and manifest-owned URL rules | Typed source-rule inventory with policy, decisions/promotions, expiry and `maestro-url-identity-migration/1`; owning core types and validators, not another parser. |
| C69 Signed independent and private packages | Admitted private-package delivery and scoped publisher/access evidence; missing private access cannot disable public selection. C42's restricted mount is unchanged. |

**Implementation boundary:** `crates/maestro/src/acquisition/catalog.rs` owns
`CatalogSource`; only this composition adapter knows both catalog and acquisition.
C66 consumes core wire types, so an acquisition-to-catalog dependency would risk
a crate cycle. Do not add it. N56 consumes the installed reader/admission APIs S3
hands off, not a second installer, trust store, registry or signature verifier.
N14 supplies the existing composition entry; caller code continues receiving
`&dyn PolicySource` / `&dyn ResourceSource`, with configuration selecting the
adapter. No API signature change, URL-engine change or consumer branch is needed.

For each resolution, use the caller-bound installed release pin and declared
source closure; map catalog ownership/inventory to the unchanged core wire refs,
never rewrite IDs/digests or use basename/last-wins lookup. Refuse missing or
ambiguous resources. Preserve exact original bytes for policy, decisions,
promotions, migrations, registries and referenced evidence. Admit only a signed,
digest-pinned release whose protected ownership approvals bind the exact source
revision/resource closure. Recheck current trust/freshness/revocation, review,
expiry, scope and principal access through S3 admission; metadata `reviewed` or a
policy authority field cannot supply that evidence. Feed the resulting resources
through existing `validate`; current runtime URL/authority controls remain in
force after resolution. Unsigned, unpinned, tampered, unreviewed, wrong-owner,
expired, revoked or inaccessible resources refuse, including changed companions.

Resolution is read-only and starts no source fetch, credential lookup, connector
or model; it neither installs content nor writes the trusted bundle or grants.
Private inventories resolve only from C69 packages under current local authority;
public catalogs, diagnostics and fixtures contain none of their real metadata.
Tests use independently authored synthetic packages and tagged privacy canaries.

**Local-first task boundary:** N15 and N30 keep `After: N03, N06` only. N15 builds
`ProfileRegistry.resolve/select` over the existing resource port, with local,
test-only substitute and disabled registry cases. A substitute is not production
catalog admission. N30 writes local proposal/activation overlays over immutable
`DirectFiles` or synthetic catalog baselines, testing trust changes, CAS and
zero bundle writes without claiming signed-catalog integration. N56 later passes
the same shared policy/resource contract tests as `DirectFiles`, through trait
objects, then N55 exercises the real installed baseline against N30 and plugs in
S4 host/scheduling adapters. N55 does not own N56 code or edit S3 documents.

N56 has S6 edges N03/N07/N14 and the four S3 edges above. C66's prerequisite is
the already-landed N03/N07 wire types, **not N56**; that distinction prevents a
cross-slice cycle. N55 adds N56 to its N30/N42/N43 predecessors. Neither N55 nor
N56 is an ancestor of N54, N15 or N30. No S3/S4 wait enters M6.

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

### Adaptation artifact schemas and resolution

**N57 owns this prerequisite to N29/N33/N34.** Evidence at S6 `d613dfb`:
`extraction/model.rs:93–103` uses bare processing IDs while
`adaptation/manifest.rs:18–82` uses `Ref` selections and an unmaterialized
candidate digest. `adaptation/storage.rs:70–144` already supplies typed encoding,
scoped retention and digest-checked reads; reuse it. All paths in this paragraph
are below `crates/maestro-acquisition/src/`. N32 supplies the pure
`EffectiveConfiguration`, `apply` and `admit` contracts, not storage or authority.

**Unreleased-format decision, 2026-10-01:** change the existing N15 `Processing`
to `cleanup: Ref, chunk: Ref, dedup: Ref`, in that order. Bump the existing
registry schema to `maestro-extraction-registry/2` and definition preimage prefix
to `maestro-profile-definition/2\n`; there is no separate extraction-profile
wire envelope to invent. Bump the N03/N30 manifest to
`maestro-acquisition-manifest/2`. Update affected synthetic goldens deliberately
with red/green evidence. S6 has not shipped, C66 has published no profiles and
N30 has no production proposals: **no migration machinery, legacy reader or
compatibility shim**. Do not change S1 canonical or corpus/1 identities.

#### Exact definition payloads

Use N03 `Resource<S>` flattened first, in its existing field order, with a closed
schema enum for each row. The following fields follow it in the listed order.
All are required; strict N03 duplicate/unknown-key, 4 MiB/depth/member limits
apply before allocation. Lists/maps have at most 1,000 entries unless a tighter
bound is specified. Definitions are immutable data; a qualification reference
is not self-approval. Generate schemas from these owning types, not copied JSON
validators. These are defensive format bounds, not new OA3 operating allowances.

| Type / exact schema | Additional fields, in serialization order |
| --- | --- |
| `CleanupRules`, `maestro-cleanup-rules/1` | `rules: Vec<CleanupRule>`, `qualification: Ref`. Empty rules explicitly mean no cleanup. Rule order is meaningful; duplicate rule IDs refuse. |
| `CleanupRule` (nested) | `id: Id`, `selector: Structure`, `action: omit_navigation`, `reason_code: Id`. Reuse N15 `Structure` and `Observation` unchanged: observed DOM path/JSON path/block/prefix, `all`, `any`; no new selector AST. |
| `S1ChunkStrategy`, `maestro-s1-chunk-strategy/1` | `chunker_version: Text`, `preparation_profile: Text`, `target_tokens: NonZeroU64`, `hard_max_tokens: NonZeroU64`, `model: Ref`, `tokenizer_qualification: Ref`, `model_limits: maestro_kernel::gateway::Limits`, `qualification: Ref`. The model pin names an existing S1 model card; no copied model identity type. |
| `DedupKeys`, `maestro-dedup-keys/1` | `exact: Vec<DedupKey>`, `prepared: Vec<DedupKey>`, `qualification: Ref`. `DedupKey` is the closed snake-case enum `original_digest`, `canonical_digest`, `prepared_input_digest`, `embedding_profile_digest`. No free-form field name or executable key expression. |

Cleanup evaluates the existing AST against each bounded extracted unit's
observations, not a document-wide match that deletes the whole document. Every
matching navigation unit is omitted once from derived text, retaining original
bytes, source mapping, rule ID and reason. No rewrite/substitution operation is
available. Reuse N15's node/depth/path bounds (1,000 total AST nodes across the
rule set, depth at most 32, subject to the enclosing JSON limit). Unsupported
observations or loss of required code/table/technical content hold under Q1;
calling content navigation does not qualify its removal.

The chunk payload resolves `ChunkProfile::named(chunker_version)` and requires
its exact `preparation_profile()`. At the inspected S1 seam
(`crates/maestro-canonicalization/src/chunk_split/limits.rs:3–7`), supported token
bounds are target 500 / hard 700; other budgets hold until S1 actually supports
and qualifies them, never create an S6 chunker. Require positive
`target_tokens <= hard_max_tokens <= model_limits.context_tokens`, embedder
role, no output-token budget, and exact equality with the current S1 model
card's qualified limits/tokenizer evidence. Count the complete prepared input,
including context, literal model formatting and special tokens. The effective
view's `protected_resources` must pin `s1_embedding_model` and
`s1_tokenizer_qualification`; every selected strategy must name those same
refs. Changing a chunk strategy cannot switch the model, tokenizer or its
qualified limit. A stored number is not model qualification. N32's qualified
maps are derived from these verified inputs; unavailable/forged inputs hold. OA1 governs quality and OA5
model/artifact adoption; no download or new profile qualification is implied.

The initial executable dedup tuples are exactly
`exact = [original_digest, canonical_digest]` and
`prepared = [prepared_input_digest, embedding_profile_digest]`, in that order,
as in S1's existing exact grouping and full-profile prepared cache. Other keys,
orders, duplicates or combinations hold as unsupported; they do not enable
arbitrary deduplication. Occurrences, permissions and near-duplicate thresholds
remain S1-owned. Selecting another immutable qualified key definition is allowed;
defining a new engine, lossy key tuple or similarity threshold is not.

New exclusion refs reuse N03 `Decisions` (`maestro-source-decisions/1`) with
exactly one `Decision` whose action matches the Change variant. No parallel
entry schema. Its decision ID must be new across the resolved collection;
selector, effective/expiry and narrowing checks remain N32's responsibility.

#### One stored effective view

`ProcessingSnapshot` is **only an envelope**, not another effective model:
flattened `Resource<SnapshotSchema>` (`maestro-processing-snapshot/1`) followed
by `effective: EffectiveConfiguration`. Add strict serialization/schema support
to N32's existing type. The effective fields stay in this declaration order:

| Existing N32 field | Wire type after N57's mechanical Ref change |
| --- | --- |
| `baseline`, `policy` | `Ref`, N03 `SourcePolicy` |
| `sources` | `BTreeMap<Id, (Ref, ProfileDefinition, Processing)>`; exactly one effective profile/default definition/processing triple per source |
| `profiles`, `decisions` | `BTreeMap<Id, Profile>`, `BTreeMap<Id, (Ref, Decision)>`; reuse N15/N03 definitions |
| `selected` | `(Option<Ref>, Option<Ref>, Option<Ref>)`, serialized as cleanup/chunk/dedup array slots; absent selection is explicit `null` |
| `approved_cleanup`, `approved_chunks`, `approved_dedup` | Each `BTreeMap<Id, Ref>`, keyed by that Ref's ID. N32's pre-amendment `(Ref, ID-valued component)` becomes the single effective Ref; no duplicated `(Ref, Ref)` or unrelated group values. |
| `qualified_chunk_tokens`, `qualified_model_limit` | `BTreeMap<Id, u64>` keyed by chunk selection Ref ID, and `u64`; all limits positive and independently verified, not trusted from storage |
| `protected_resources` | `BTreeMap<Id, Ref>` with exact identities of the remaining protected closure |

Tuple values serialize as arrays; maps are `BTreeMap`, never unordered maps.
Profile map keys equal `ProfileDefinition.id`; source keys equal N03 source IDs;
decision keys equal `Decision.id`. Ref-valued processing pins bind whole cleanup
and dedup sets, not individual bare rule IDs. Resource IDs inside stored envelopes
are bounded logical metadata, not a second lookup or activation selector.

**One resolver, owned by N57; N32 stays pure:**

1. Reread the baseline through the existing `ResourceSource`/validator and each
   scoped definition through N30 storage under current principal/grants. Check
   schema, digest, collection/owner/visibility, transitive scope and external
   approval/qualification, including platform/model. Preserve the immutable
   baseline; a derived snapshot is not an editable trusted-bundle copy.
2. Resolve exactly one profile per source from its declared eligible refs and
   pinned N15 selection evidence. Missing or ambiguous selection holds adaptation;
   never choose the first profile. Source membership cannot change automatically.
   Bind its exact definition digest and retain the immutable `ProfileDefinition`.
3. For each cleanup/chunk/dedup group, take `selected[group]` when non-null,
   otherwise that source profile's `processing[group]`. Resolve it only through
   the matching schema/group map. Set changes replace that group's selection for
   every source; a profile change affects defaults only in null slots. Some-to-null
   is held because no closed Change clears a selection. Validate every source's
   effective `Processing` equals this result, even if its stored claim differs.
4. Rebuild approved group maps, positive token-limit inputs and protected closure
   from current evidence, never from a caller's claim of qualification. Compare
   with the stored effective view; changed/unavailable inputs hold requalification.
   Feed this same type to `apply(old, candidate, changes, now)` and
   `admit(old, candidate, changes, now, not_applicable)`. Both keep N30's
   content-free `WriteError::Held`; no new control port or second evaluator.

Stored candidates may still be held by the quality matrix: storage/inspection
is not approval. N29 materializes an effective candidate only when its definition
closure resolves; inferred new definitions with proposed evidence remain held
reports until independently admitted. This adds no sixth automatic class.
OA4c gates private samples/retention; OA3 still bounds sampling and activation.

#### Storage, identity and lineage bindings

All new definitions, snapshots and singleton decision artifacts are retained via
N30 `storage::retain` over existing N06 `Receipts`; new artifact refs are
`{id: Handle.to_string(), digest: Digest::of(stored_bytes)}`. Read them only with
`storage::artifact`/`Receipts::read` and fresh grants, not `Database::get` or a
filesystem lookup. Link every transitive receipt handle and inherit all resource
scope tags; recheck externally admitted profile/model/qualification refs through
their existing resource ports. Dispatch the reader by the field's contract,
never try another store after refusal. No new directory, table, global ID index,
manifest file or content-bearing notification is added.

The exact new/changed preimages are typed structures encoded with
`serde_json::to_vec`, never `Value`, `Map`, `HashMap` or a JSON stringify step:

| Identity | Exact preimage |
| --- | --- |
| Definition/snapshot artifact digest | Entire corresponding typed payload above, Resource first and remaining fields in table order; store those same compact UTF-8 bytes. No digest field refers to its own artifact. |
| N15 profile definition | Bytes `maestro-profile-definition/2\n` followed by `to_vec(&ProfileDefinition)` in its existing declaration order, with Ref-valued Processing. |
| `Proposal.candidate` | Digest of the entire `ProcessingSnapshot` bytes, **not** policy bytes, activation history or an arbitrary digest. Exactly one handle in `Proposal.evidence` must resolve to that snapshot/digest; duplicate or missing matching handles refuse. |
| Manifest `effective_digest` | `to_vec(&EffectivePreimage { schema: "maestro-acquisition-effective/2", policy: &SourcePolicy, processing_baseline: &Ref, activations: &[Ref] })` in that order. Activation refs retain their existing order; proposals do not change effective identity. |

Pin bytes **and** SHA-256 vectors for every row (each definition kind separately),
plus changed manifest/rollback fixtures, in both default and workspace
`serde_json/preserve_order` builds. Insertion order of equivalent BTreeMap inputs
cannot change identity; altered selector/model/key/default bytes must change it.
Do not silently update an existing digest version or S1 identity golden.

The existing manifest's new `processing_baseline` pins the admitted initial
snapshot before initialization; `snapshot.effective.baseline == manifest.baseline`.
It is distinct from N03 `AdaptationPolicy.baseline` (comparison evidence), and
the snapshot contains no manifest/activation self-reference.
An empty overlay still has `active == baseline`. The active processing snapshot
is then the initial pin, or the current effective activation's proposal candidate
resolved through its evidence handles, following N30 `restores` lineage. Do not
add a second active pointer or reread irrelevant superseded payloads. On baseline
rollback, bind `Proposal.candidate` to `processing_baseline.digest` and include
its handle in evidence; other rollback targets bind their exact retained snapshot,
not a replay of old changes over new state. N57 updates N30's existing rollback
constructor and typed preimage/goldens; CAS, ancestry, lock and commit point stay.

N20/N24 consume the same typed cleanup data after N57; N27 consumes its verified
chunk/model-limit and dedup payloads through the existing S1 ports. These are
contracts for already-planned behavior, not additional engines. N29 persists
definitions/snapshot before `ConfigurationWriter::propose` and links
the candidate handle through existing evidence. N33's gate receipts bind that
snapshot and the old resolved snapshot with baseline, suite, gold, profiles and
cohorts. N34's `ActivationAuthority` adapter rereads them and current authority
on every check, including current reads, commit and committed recovery; a
previous gate pass cannot turn substituted or inaccessible bytes into authority.
Use existing under-lock read helpers, not a recursive `LocalWriter::current`
call from `ActivationAuthority::check`. N33's typed gate receipt must distinguish
forward activation from rollback and bind the exact expected baseline/active,
target snapshot and rollback target. Rollback verifies N30's earlier-target
lineage and current target authorization, not a replay of the old forward Change
list or permission to remove exclusions through ordinary activation. It must
not require revoked superseded payloads. Missing OA1 matrix evidence holds;
OA4a/OA4b grants and OA4d cutover remain separate. N35 still owns the first post-activation check.

### Proposal, activation and privacy transaction

Reuse the actual N30 `Proposal` wire record, not the earlier proposed second
`maestro-processing-proposal/1` envelope. Its exact field order is
`expected_baseline: Ref`, `expected_active: Ref`, `changes: Vec<Change>`,
`candidate: Digest`, `evidence: Vec<Handle>`, `report: Handle`, `rollback: Ref`.
It has no extra Resource/state fields: scoped reports carry observations,
uncertainty, cohorts and outcomes. N57 changes the meaning of `candidate` to the
stored snapshot digest above, not its type or a new proposal model. N30's six
`Change` variants and fields remain: `select_profile {source_id, profile: Ref}`,
`set_cleanup {rules: Ref}`, `set_s1_chunk_strategy {strategy: Ref}`,
`set_dedup_keys {keys: Ref}`, `add_knowledge_exclusion {entry: Ref}` and
`add_asset_only {entry: Ref}`. The three Set changes are collection-wide;
selection clearing is not a seventh variant. Existing exclusion edits, indirect
protected changes and mixed proposals hold in full. Unknown edits remain scoped
held reports, not an executable JSON Patch.

N30's direct-file implementation stages immutable proposal/evidence artifacts,
then locks the collection, rereads expected baseline/active digests and performs
an atomic manifest replacement with durable recovery marker. The effective-manifest
pointer is the commit point; only after it commits is the journal notification
emitted. Recovery completes a missing notification or discards an uncommitted
staging record, never exposes a half-activation. The report is immutable and exists
before the pointer references it. N30 starts on N03/N06's `DirectFiles` and
test-only immutable catalog fixtures; synthetic trust/revocation inputs are not
production admission evidence. N56 later changes baseline resolution, not this
write contract. N55 proves integrated catalog-baseline use makes zero
trusted-bundle writes.

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

### Learned term-alias candidates and later search seam

N29 learns aliases automatically only where source text defines a short form or
variant, for example an acronym and its expansion. Preserve the exact defining
span in immutable source evidence; similarity or global spelling normalization
alone is not evidence that terms mean the same thing. No hand-written per-product
list. Conflicting expansions retain separate scoped candidates with ambiguity,
not an automatically chosen identity. Private evidence inherits source scopes.

Persist candidates as kernel-scoped artifacts and append review/reversal records;
do not silently merge terms, document IDs or permissions. The versioned candidate
record above exposes an S2 `ALIAS_OF`-compatible evidence/identity seam without
requiring S2 delivery. S2's pinned spec (`edcce756`, `spec.md:408–410`) treats
`ALIAS_OF` as a reviewed identity record, not an extracted claim predicate;
S6 must not submit inferred candidates as approved S2 claims. Automatic candidate
discovery is not automatic approval and does not add a sixth configuration-change class. Later S1 search may consume
only approved, still-authorized records through that seam for query expansion;
S6 implements no query expansion, ranking change or hand-maintained product list.

### S1 mapped-ingestion seam and identity continuity

N26 factors `import/entry.rs` into a shared `import/ingest.rs` core-side call:
`ingest_mapped(target, input) → imported/unchanged/held/refused`. Its input carries
verified Markdown bytes/digest, stable collection/source/source_ref, S1 metadata,
existing typed extractor blocks/assets, raw-capture and fidelity refs plus approved
quality disposition. Both corpus/1 import and S6 call this transaction; corpus/1
supplies empty structural additions, preserving existing behavior and identities.

**Native semantic asset-inventory binding:** N26 computes a versioned digest over
sorted unique relevant-asset records `(stable destination, AssetStatus, content
Digest or null, byte length or null)`: SHA-256 of compact UTF-8 JSON containing
the `maestro.native_assets/1` version tag and the destination-sorted tuple array,
using the existing serde representation of AssetStatus. Relevant assets are those
referenced by Markdown/extractor blocks or required by the fidelity profile. Available records
must resolve to verified bytes/digest/length; missing/unchecked records explicitly
carry null when bytes are unknown. Conflicting duplicate destinations refuse.
Exclude run IDs, attempt times, machine-local storage paths, signed URLs and transient errors;
these are receipt observations, not semantic revision inputs. The existing
`CanonicalizeInput.assets` remains the destination/status map; full records stay
in the linked extraction evidence.

Bind the digest before the single canonicalization call using core-reserved
`SourceMetadata.extra["maestro.native_assets/1"]`; native input rejects
source-supplied collisions.
That map is already in S1's revision preimage (`pipeline.rs:44–52`), so no global
hash change or second canonicalizer is needed. Only native mapped ingestion adds
the key (including an empty-inventory digest); corpus/1 never adds it and keeps
its document/revision identities byte-for-byte. Repeating an equal semantic
inventory is unchanged; **missing → available**, changed asset bytes or any other
semantic inventory change produces a **new immutable revision**, even with equal
Markdown, metadata, blocks, permissions and profile. The old inventory remains
resolvable and is never overwritten by import's unchanged branch. Raw Markdown
and prepared-input content hashes stay distinct; asset changes need not force
embedding when prepared input is identical.

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
3. N57 completes the typed artifact/resolution contract after N15/N30/N32;
   N29–N35 implement US3 inference, proposals and automatic gated activation;
   N36–N42 implement US4 lifecycle, resource-safe concurrency and local timers.
4. N43–N48 implement US5 local connectors, sessions, browser/wiki and separately
   approved private origins. N49–N54 qualify US6 family parity/retrieval/cutover.
5. N56 delivers the catalog-backed read adapter after N03/N07/N14 and external
   S3 C41/C43/C66/C69 handoffs; N55 follows N56 for integrated baseline/write-port
   and S4 adapter conformance. Both remain outside M6's critical path. Required
   local/synthetic conformance starts now in N03/N15/N30/N43/N42, not after S3/S4.

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
nondeterminism use **δ = 0.01, approved 2026-09-30**, three paired repetitions
and seeded 2,000 question-level bootstrap resamples as the approved spec requires;
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
that need replaceable local/later adapters. No graph, adaptive statistics package,
scheduler runtime, converter chain or corpus/2 format is added. N29 learns
source-evidenced term-alias candidates under the dated owner decision; a search
query-expansion engine remains later S1 work, not an S6 dependency.
N09 introduces transport-side cumulative decode accounting; N16 extends that same
accounting contract across parser/asset IPC rather than introducing a second meter.
N33–N35 can land against synthetic gate receipts after N57's artifact contract,
before every real matrix producer exists: live activation remains held until N28, lifecycle/concurrency qualification,
N49 and N52 supply actual required evidence. This separates code dependencies from
runtime qualification and never permits a synthetic gate receipt in production.
N02/N17–N22 may discover a library/containment infeasibility; report the affected
adapter as blocked and request a design ruling, never weaken the spec. Pending
OA4c/OA4d receipts remain explicit execution blockers, not permission to stall
unrelated work or fabricate private comparison/retirement approval.
