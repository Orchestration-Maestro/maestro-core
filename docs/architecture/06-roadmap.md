# 06 Roadmap

Delivery in vertical slices. Each slice ships working, tested, released software
with measurable exit criteria; the next slice starts from real evidence, not
from a plan. Only the **active** slice has a detailed Spec Kit spec, plan and
tasks (`specs/NNN-*/`); later slices are described here and specified when they
become active. Every task of the earlier unified plan (U01–U18) and its folded
foundation, qualification and delivery obligations maps to a slice in
[08 §12](08-traceability.md#12-delivery-mapping); each slice's spec re-checks
that mapping before it starts.

## 1. Slices at a glance

| Slice | Delivers | Estimate* | Depends on | Milestone |
| --- | --- | --- | --- | --- |
| **S0 Foundation** | Clean `maestro-core` (canonicalization only) under org gates; the repositories S1 needs published; Spec Kit | 4–6 days (measured scope) | — | — |
| **S1 Knowledge kernel + hybrid RAG** | Kernel building blocks; Control-M collection imported, published and searchable through MCP; eval suites; first model bake-off | 12–15 days | S0 | **M1 "Ask Control-M"** |
| **S2 Knowledge graph** | Fact store, extraction, entity resolution, Neo4j projection, graph route, graph evals | 10–15 days | S1 | **M2 "Relationships answered"** |
| **S3 Catalog** | Copilot-native catalog v1, settings classes and overrides, compile/release/install/update with freshness and revocation, `maestro init` with presets and language overlays, host projection, intent routing, policies | 10–13 days | S1 (S2 for impact queries) | **M3 "Catalog installable"** |
| **S4 Orchestration runtime** | Workflow graphs, durable engine, daemon, Copilot SDK + llama.cpp sessions, Cedar broker, sandbox, contracts, interrupts, extension host and event stream, test kit | 18–24 days | S3 | **M4 "First governed workflow"** |
| **S5 Capabilities + InnerSource** | Monitoring, Product Owner and Control-M orchestration-planning capabilities; scaffolder; scenario runner; a contributed capability | 10–15 days | S4 | **M5 "First contributed capability"** |
| **S6 Native acquisition** | Frontier, fetchers, extraction, adaptive manifest policy and private connectors; Rust producers replace Python family by family; crawl4ai render-only exception | 15–25 days | S1 delivery only; not M1 release or S3/S4 | **M6 "Python retired"** (producers) |
| **S7 Intelligence backend** | I1 memory and continuity, I2 code intelligence, I3 governed knowledge, I4 workbench | I1 4–6 wk, I2 3–4 wk, I3 3–5 wk, I4 4–6 wk | S1–S4 | **M7 "Maestro remembers"** (I1) |
| **S8 Provenance and reverse engineering** | Provenance register, analyzer extensions, behaviour contracts, the clean-room boundary | R1 1–2 wk, R2 2–3 wk, R3 2–3 wk, R4 1 wk | S1, S4, S7-I2 | **M8 "Cleared to reimplement"** |

\*Engineering days of one developer working with coding agents, excluding waits
for external approvals. Estimates are replaced by measured velocity after S1.

### Two schedules

| Week | Sequential (one writer) | Two tracks after S1 (knowledge ∥ agents) |
| --- | --- | --- |
| 1 | S0 | S0 |
| 2–4 | S1 → **M1** | S1 → **M1** |
| 5–7 | S2 → **M2** | K: S2 → **M2** · A: S3 → **M3** |
| 7–10 | S3 → **M3** | K: S6 starts · A: S4 |
| 10–15 | S4 → **M4** | A: S4 → **M4** (≈ week 11) · K: S6 |
| 15–18 | S5 → **M5** | A: S5 → **M5** · K: S6 → **M6** |
| 18–23 | S6 → **M6** | S7-I1 → **M7** |
| 23+ | S7 | S7-I2 … |

The two-track schedule needs two concurrent pull-request streams (different
crates of `maestro-core`, plus `maestro-manifests`); review capacity is the
limit, not the code.

## 2. Slice details

### S0 Foundation

**Goal:** a clean repository that meets the organization's gates, with only the
canonicalization crate carried over, and the repositories S1 needs published.
Plan and tasks: [`specs/000-foundation/`](../../specs/000-foundation/plan.md).

| In scope | Out of scope |
| --- | --- |
| Archive snapshot of the current `maestro-core` outside the org directory (after confirmation); fresh history | Any new product feature |
| Root files to org standard (README, AGENTS.md, CONTEXT.md, MIT LICENSE, editor/format configs, toolbelt, justfile, prek hooks, `.github` with CI pinned to rust-workflows v1.2.1, Dependabot, Scorecard, CODEOWNERS; contributing, security and conduct come from the organization's defaults); the draft CI's Sonar inputs and secret, the crate's golden-workflow recipe and its Artifactory setting removed | Changing canonicalization behaviour |
| Canonicalization under workspace lints (measured 2026-09-24): 244 Clippy findings fixed (186 missing docs), four files over 500 counted lines split into modules, five production and six test functions over 100 lines or complexity 15 refactored, the six-parameter function given an input type; the crate's Python scripts leave (corpus sampling to the private collection, tokenizer qualification to the archive, replaced in S1) | |
| Repository policies as tests CI runs: counted lines per file, personal paths and private material, lint inheritance, Markdown links and anchors | |
| Mutation testing: 810 mutants do not fit the CI job's 45 minutes, so a full local run reaches zero survivors before the import pull request, which alone runs CI without mutation testing (recorded exception, removed by the next pull request) | |
| No personal paths or vendor-specific infrastructure in public files: native-tokenizer artifact paths move to a checked host binding (`maestro-native-binding/1`: model, counter, library directory, source root; `MAESTRO_NATIVE_BINDING`; 1 MiB bound; paths relative to the binding file), separate from the qualification profile (fingerprints, argv `-m MODEL --offline --stdin --no-escape --ids`, cleared environment, 30 s timeout with kill and reap, 16 MiB stdout and 1 MiB stderr bounds); Artifactory mentions removed | |
| The engineering baseline of [05 §9](05-platform-and-operations.md#9-engineering-baseline): lint inheritance explicit in every member, 90 % coverage enforced (not 80), MSRV decision recorded | |
| Spec Kit scaffolding (`.specify/`, Copilot prompts) with the constitution; architecture, ADRs and specs committed | |
| Repositories: `maestro-core` (public), `maestro-model-router` (public; imported on 2026-09-24 by its pull request #1, CI on rust-workflows v1.2.1), `ctm-collection` (private, outside the organization, whose rulesets allow only public repositories; it receives the vendor-specific sources leaving the public tree; the exporter lands in S1). `maestro-manifests` is created in S3 with its first file | |

**Exit criteria:** `maestro-core` CI green on `main` with coverage ≥ 90 %; all
canonicalization tests pass unchanged (native tests still marked for explicit
runs); a relocation under the same new profile keeps identities and a changed
profile creates new ones (explicit native run: ordered IDs, Unicode, NUL and
literal specials, 499/500/501/699/700/701 boundaries, context refusal,
structural continuations); no absolute personal path, secret or
vendor-infrastructure reference in any public repository (gitleaks + a path
check); the three repositories exist, the public ones with rulesets applied.

### S1 Knowledge kernel + hybrid RAG → M1

**Goal:** a developer or agent asks a Control-M question in Pi or Copilot and gets
cited passages from the full corpus, with measured quality.

| Deliverable | Detail |
| --- | --- |
| `maestro-kernel` | B1 scopes, B2 journal, B3 artifacts, B4 jobs, B5 documents, B6 generations, B7 evidence, B9 capability registry, B10 model gateway, B11 telemetry and health |
| `maestro-knowledge` | Collections (`collection.json`), import (`maestro-corpus/1`), the corpus quality gate with explicit outcomes, prepare (exact + near-duplicate dedup, chunking through `TokenCounter`), `RouterTokenizer` with parity qualification, representations (dense + BM25 with an explicit French/English analyzer policy), Qdrant generations with alias switch, independent `search_dense` / `search_bm25` diagnostics, search (R1–R3 and R6, RRF, rerank, evidence assembly with span unions), `ask` with guards and validation before delivery, eval runner |
| Journal | S1 ships and tests per-stream sequences, durable cursors and acknowledgements; built-in projection and telemetry consumers use them in S2 ([07 §7](07-extensibility.md#7-delivery-by-slice)) |
| `maestro` binary | CLI (`knowledge …`, `eval …`, `status`, `doctor`, `setup`, `backup`, `restore`) and MCP stdio server (`knowledge_collections`, `knowledge_search`, `knowledge_get`, `knowledge_ask`) |
| Private product collection | Owner-managed source scope, inventory, counts, quality ledger, evaluation set and receipts stay in the private collection repository; public CI uses synthetic fixtures only |
| Bake-off round 1 | Embedder, reranker, answerer (see [05 §3](05-platform-and-operations.md#3-model-selection)) |
| Services | Qdrant unit through `maestro setup` |

**Exit criteria:**

1. The current owner-pinned private receipt is fully accounted for, with every
   refusal explained in the private report and each revision carrying a quality
   outcome; only accepted documents are indexed.
2. Evidence recall per route is measured before fusion, and every failure in
   the golden set is classified (not retrieved, misranked, wrong answer).
3. A generation built with the bake-off winners is published; the ladder report
   shows each shipped rung paying for itself.
4. Command exactness 100 %; no-answer accuracy ≥ 80 % (initial target, revisited
   once the baseline is measured).
5. `knowledge_search` p95 < 1.5 s measured on the reference workstation.
6. MCP tools work from Pi, Codex, Claude Code and Copilot CLI.
7. Backup → restore → projection rebuild drill passes.
8. CI green, coverage ≥ 90 %, synthetic eval suite gating pull requests.

### S2 Knowledge graph → M2

| Deliverable | Detail |
| --- | --- |
| B8 fact store | Entities, aliases, mentions, relations, qualified claims with evidence spans (anchored to blocks, not chunks), world and record times, review queue |
| Extraction | Structural stage; Control-M rule pack (private); model extraction with verified evidence quotes (extractor from bake-off) |
| Resolution | Normalization, blocking, entity vectors, conservative auto-merge, review queue |
| Projection | Neo4j 2026.x Community with generation stamps; full rebuild by import; incremental `MERGE`; LadybugDB adapter spike |
| Retrieval | Graph route R4 (local, path, global-on-demand), independence rule, support groups, explanations in evidence; `knowledge_graph_neighbors`, `knowledge_graph_path`, `knowledge_entity_resolve`, `knowledge_evidence_trace` |
| Comparison | Qdrant only, Neo4j only and the pairing, on the graph suite, with the same generator and context budget |

**Exit criteria:** the graph route improves the `ctm-graph` suite by a recorded
margin without lowering `ctm-retrieval`; every stored relation has a verified
evidence span; deleting the Neo4j data and rebuilding from the kernel restores
identical query results; the LadybugDB spike has a written verdict.

### S3 Catalog → M3

| Deliverable | Detail |
| --- | --- |
| `maestro-catalog` | Parse, check, compile, verify (attestation, freshness, revocation, version floor), install, update, project, route, resolve, search, impact, explain; settings classes and override resolution; project lock |
| Bootstrap | `maestro init` with presets, base template and language overlays (Rust first; others as projects need them), preview, stop-on-collision apply, composed-output tests |
| Catalog v1 content | Written from zero ([03 §1.8](03-agent-orchestration.md#18-writing-the-first-catalog)): the `feature-delivery` and `ctm-question` workflows, the Maestro orchestrator and the owner's roles they need (coder, tester, reviewer; builder as a step), and only the skills, instructions, contracts, Cedar policies (default deny, destructive operations, protected paths, egress, each with allowed and denied fixtures), model profiles (`fast`, `balanced`, `deep`) for both providers, MCP server descriptors and discovery cards those workflows use; every file's pull request names its 08 rows |
| Release | Manifests CI: check, policy tests, attested bundle |
| Hosts | Projection to Copilot and Pi (dry run, apply, remove); native `preToolUse` hook → `maestro policy check` |
| Routing | Discovery cards as the `catalog` collection; `catalog_route`, `catalog_impact`; labelled intent suite |
| Spike | Copilot's tolerance of `metadata:` in `.agent.md` (decides sidecar or frontmatter) |
| Comparison pass (after M3) | The earlier catalog read once against the new one; each recovered item its own pull request citing it; the rest listed with the reason |

**Exit criteria:** a tagged bundle is attested by manifests CI and verified by
`maestro catalog install`, and a revoked or expired one is refused; an
unclassified setting is rejected; `maestro init` previews and applies every
language composition cleanly and stops on collisions; projection is idempotent
and removes only owned files; routing beats the structured baseline and reaches
top-3 accuracy ≥ 90 % on the intent suite (initial target); every policy rule
has passing allow and deny tests.

### S4 Orchestration runtime → M4

| Deliverable | Detail |
| --- | --- |
| Graph compiler | The twelve rules of [03 §2.3](03-agent-orchestration.md#23-compile-time-validation) |
| Engine + daemon | Event-sourced state, scheduler, recovery with uncertain-effect handling, interrupts, budgets, cancellation; systemd user unit; Unix-socket JSON-RPC |
| Sessions | Copilot SDK 1.0.14 with `copilot` and `llamacpp` providers, hooks, permission handler, `submit_result`, context assembler |
| Broker + sandbox | Cedar decisions on normalized arguments, approvals, Landlock + seccomp + network namespace + cgroups, worktrees |
| Acceptance | Contracts, semantic validators, evidence cross-check, bounded repair |
| Surface | `maestro run …` CLI; MCP run tools as long-running tasks; local HTTP API with server-sent events; schedules; OTel spans for runs |
| Extensions | Extension host, Maestro Extension Protocol, process extensions, outbound webhooks, public event catalogue with schema compatibility tests, a reference `echo` extension ([07](07-extensibility.md)) |
| Qualification | Provider qualification on both routes (direct endpoint, then SDK), the first model cards and the qualification registry, the test kit and released scenario runner with strict replay rules |

**Exit criteria:** `feature-delivery` completes on the maestro-release-canary repository
with an accepted result on each provider separately; killing the daemon
mid-run and restarting it resumes correctly with uncertain effects surfaced;
every denial test proves its stimulus reached the real guard; sandbox escape
tests fail closed; an extension is activated, receives events, survives a
restart without loss and is deactivated without restarting the daemon; L1–L4
suites in CI, L5 recorded as qualification cards.

### S5 Capabilities + InnerSource → M5

Three capability slices, each with its own owner, contracts, policies and eval
scenarios: read-only monitoring investigation (observations distinguished from
diagnoses), Product Owner drafting (no fabricated approvals or commitments),
and Control-M orchestration planning (plan-only, powered by the `ctm` knowledge
and graph; no apply). Plus the scaffolder, the released scenario runner, the
change-class review check, capability records (maintainer, backup, maturity,
environments, deprecation) and a contributor guide. **Exit:** each capability
passes its scenarios; a non-platform contributor ships one change (a capability
or an extension) through pull request → review → attested release → adoption
without building the runtime.

### S6 Native acquisition → M6

**Dependency: S1 delivery only, not M1 release or S3/S4 delivery.** The
[S6 spec](../../specs/006-native-acquisition/spec.md),
[plan](../../specs/006-native-acquisition/plan.md) and
[tasks](../../specs/006-native-acquisition/tasks.md) define the enforcing work.
N01 records the owner's 2026-09-30 amendments before code; ownership below is
planned, not a claim of implementation, live authorization or qualification.

| Deliverable | Required boundary and enforcing tasks |
| --- | --- |
| Local adapters now | Direct manifest-file policy, a qualified supervised subprocess connector using the shared extension protocol, and command-line/local timers invoking the same admitted sync operation (N03, N14, N42–N45). Small policy-source, registry, transport, connector and scheduling ports permit substitution/disablement without caller changes. Later S3 catalog and S4 host/scheduler adapters are N55, outside M6, with no second queue, general host or authority. An adapter without enforceable containment refuses; waiting for S4 is not a substitute. |
| Rust acquisition and extraction | Kernel-owned frontier, admitted HTTP/browser transports, typed immutable captures and receipts, strict source policy, offline profile/fidelity qualification, declarative wiki mapping, bounded decoding and protected preflight (N04–N28, N43–N48). Spider stays the Rust crawler, extras/chrome/chromey off. Only unavoidable browser work uses **crawl4ai (Python, out of process)**, replacing Spider's chromey renderer. N02 records its named ADR-0020 artifact/role exception; N46 qualifies it. It owns no frontier, conversion, inference, embedding or publication; no other production browser fallback or Python producer/extractor/connector is permitted. |
| Gated automatic adaptation | One extensible profile registry and manifest authority; evidence-backed proposals, closed five-class allow-list, all mandatory matrix gates, protected fields and whole-proposal refusal (N15, N29–N35). Among registry decisions, only new knowledge exclusions/asset-only entries may be machine-qualified; this is never human review or a `deny_fetch` change. Atomic baseline/active compare-and-swap, persistence/rate limits and first-run postcheck/rollback preserve the last qualified configuration/index. No suite, gold or baseline means held, not passed. |
| Parallel harvesting | Sources/families share fenced frontier ownership, no duplicate equivalent in-flight fetches, aggregate origin/resource budgets, idempotent captures and isolated staging. Serialize S1 publication and reject/reconcile stale source-revision sets; search/ask retain verified generations and interactive priority (N04, N10–N12, N34, N37, N40). No second production writer at cutover. |

The owner's same-day alias approval is source-evidenced discovery of term-alias
candidates (N29): exact supporting spans, scope/ambiguity preserved, reviewable
and reversible, no hand-written product list or silent merge. S2's reviewed
`ALIAS_OF` identity seam is reused without an S2 delivery dependency. Candidate
inference is neither approval nor a sixth automatic change class; query expansion
is later S1 work. Samples and all derived reports remain in protected scoped
storage; ordinary logs/notifiers contain only fixed status and access-checked
opaque handles (N06, N29, N30, N34, N54).

**Exit criteria (N49–N54):**

1. Each approved family passes frozen source-grounded fixture comparison and a
   separately authorized sampled live diff over full, incremental, resume,
   withdrawal and repair. All twelve legacy defects have regression evidence;
   known defects are independently adjudicated, not copied for parity. Required
   media/platform qualification and Q1–Q5 evidence remain mandatory. Each family
   and the final combined native generation pass retrieval qualification against
   the frozen current S1 baseline; a changed baseline requires rerunning both arms.
2. Independent review and explicit family retirement approval precede cutover;
   exactly one production writer remains. Observe one scheduled native refresh
   and a rollback exercise under current revocations/retention, retaining two
   verified generations only as authorized. Missing private-comparison (OA4c),
   retirement (OA4d), exact-source grants or qualification evidence holds the
   affected action; policy/tool approval alone grants no access.
3. Every approved migration family is cut over, with no production Python
   producer, extractor or connector. The sole pinned, audited crawl4ai browser
   adapter may remain in its render-only role without blocking M6; broader Python
   roles or another browser fallback fail cutover regardless of output parity.
   Native extraction keeps the unchanged Xberg/native docling.rs bake-off, tract
   layout/OCR default, dynamic offline table-only ONNX Runtime and individually
   licence-checked offline weights. Actual artifact/feature audits and qualified
   results, not this roadmap, select the path (N02, N21, N46, N50, N53, N54).

The approved OA2 named-origin policy is a separate N48 amendment with enforcing
code; the current private-network prohibition remains until it lands, and each
live origin still needs its own authenticated expiring grant. Synthetic/offline
qualification needs no protected login; it does not replace the actual platform
controls or later authorized live evidence. N55's pending adapters never block M6.

### S7 Intelligence backend → M7

Phases I1–I4 as described in [04](04-intelligence-backend.md), each gated by a
parity evaluation against the provider it replaces and by the hard, quality and
usefulness gates of [04 §10](04-intelligence-backend.md#10-quality-targets-and-gates).
The open decisions of [04 §12](04-intelligence-backend.md#12-decisions-still-open)
are settled before the phase that needs them.

### S8 Provenance and reverse engineering → M8

**Goal:** a provider or component can be studied, compared and replaced without
inheriting its code, its licence obligations or its defects, and the record
proves it afterwards. Design: [09](09-reverse-engineering.md), ADR-0019.

| Deliverable | Detail |
| --- | --- |
| **R1 Provenance register** | The `provenance` collection: component, origin, pinned revision, licence, usage class (`reused`, `modified`, `ported`, `specification-only`, `independent`), obligations, evidence and approver; a ScanCode-class analyzer proposing records; notices and SBOM material generated from the register rather than maintained beside it |
| **R2 Static analysis** | The `analyzer` extension kind and its contract (findings, evidence, coverage, limits, method); analysis jobs and `analysis/<target>` scopes; Joern and CodeQL analyzers; normalization to targets, components, surfaces and capabilities; the comparison report across analysed systems |
| **R3 Behaviour contracts** | Authorized capture through Playwright and mitmproxy, turned into executable scenario suites and an independent API specification; Frida and headless Ghidra analyzers for lawfully possessed native components; parity suites generated from the contracts |
| **R4 Clean-room promotion** | The `analysis/*` boundary as Cedar policy with allow and deny fixtures; `analysis.specification.promote` with a named approver; the verbatim-quotation check; the promotion audit trail |

**Exit criteria:** an implementation principal cannot reach an `analysis/*`
passage, event or artifact through any route or cache, proved by the isolation
suite; a specification quoting analysis material is refused and the refusal
names the span; a component without an origin, revision, licence and usage class
blocks the build that would ship it; the notices of every `reused`, `modified`
and `ported` component in a release are generated from the register and match
it; two analyzers answering the same question produce comparable findings, each
with its own coverage and limits, neither silently merged; one provider from
[04 §2](04-intelligence-backend.md#2-the-provider-analysis-distilled) has a
behaviour contract that its own release passes and that a native capability is
then measured against.

R1 may be pulled forward: the first S7 provider replacement needs its component
registered before it starts, so R1 ships with that phase if S8 has not begun.

## 3. Explicitly deferred

- The desktop workbench before I4 (it is native Rust, ADR-0016; there is no web
  UI).
- Streamable HTTP MCP transport and any remote access before S4 hardening.
- General orchestration-runtime macOS and Windows qualification (after S4 on
  Linux); S6 independently qualifies its claimed parser/connector/browser controls
  through N18, N19 and N46, without an S4 delivery prerequisite.
- Multi-user or team-server deployment of the kernel, SSO, high availability
  and replication.
- Learned sparse and late-interaction retrieval unless a bake-off selects them.
- Inbound webhooks, the broker bridge and WebAssembly extensions until a real
  integration asks for them.
- The structured-data (SQL) knowledge profile until a domain needs it.

## 4. Risk register

| # | Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- | --- |
| R1 | Planning outgrows delivery again | Medium | High | [ADR-0010](../adr/0010-spec-kit-and-executable-gates.md): specs only for the active slice; evals and tests are the only gates; every slice ends in a release |
| R2 | Copilot SDK or CLI changes break sessions | Medium | High | Pin SDK and CLI; L2/L3 tests catch protocol changes; the `llamacpp` provider keeps the engine usable |
| R3 | neo4rs incompatible with Neo4j 2026.x | Medium | Medium | Qualify in S2; fall back to 5.26 LTS; LadybugDB adapter |
| R4 | Model licences or quality disappoint | Medium | Medium | Bake-off with licence checks; managed route for agent roles when local models do not qualify |
| R5 | VRAM contention between embedder, reranker, answerer and agents | High | Medium | Router budget; residency plan per role; batch jobs scheduled off interactive hours |
| R6 | WSL2 specifics (systemd, Landlock, GPU) differ from native Linux | Medium | Medium | Reference environment explicit; native Linux qualified separately |
| R7 | Copilot runtime misbehaves inside the sandbox | Medium | High | S4 spike first; narrow the sandbox profile rather than drop it; broker remains authoritative |
| R8 | Eval set not representative | Medium | High | An independent reviewer checks every question; the owner decides flagged changes; group equivalent version copies and track per-type results |
| R9 | Vendor content or connectors leak publicly | Low | High | Private repository, gitleaks and path checks, scope tags, no corpus in public CI |
| R10 | Single maintainer bandwidth | High | High | Two-track schedule only if review capacity exists; slices small enough to finish |
| R11 | Requirements lost between plans | Medium | High | [08](08-traceability.md) is updated by every slice spec; a requirement changes status only with a stated reason |
| R12 | Bare identifiers collide across documents (C01, U03 mean different things in different sources) | High | Medium | Namespaced IDs in 08 (`chat.C01`, `delivery.U03`, `product.C01`) |
| R13 | An extension becomes an attack path | Medium | High | Out-of-process, sandboxed, least-privilege principals, declared effects, activation separate from publication (ADR-0013) |
| R14 | A studied component's licence obligations reach a public MIT repository unnoticed | Medium | High | The provenance register gates adoption on a resolved licence and a usage class; the clean-room boundary keeps analysis material away from implementation; notices are generated from the register and checked in the release ([09](09-reverse-engineering.md), ADR-0019) |
