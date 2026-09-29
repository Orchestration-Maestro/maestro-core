# 06 Roadmap

Delivery in vertical slices. Each slice ships working, tested, released software
with measurable exit criteria; the next slice starts from real evidence, not
from a plan. Only the **active** slice has a detailed Spec Kit spec, plan and
tasks (`specs/NNN-*/`); later slices are described here and specified when they
become active. Every task of the earlier unified plan (U01–U18) and its folded
foundation, qualification and delivery obligations maps to a slice in
[08 §12](08-traceability.md#12-delivery-mapping); each slice's spec re-checks
that mapping before it starts.

**Exception (owner, 2026-09-28 08:12):** S2 and S3 start while S1 finishes;
for them, unfinished S1 prerequisites gate the affected live checks and exits,
not fixture work. This scoped exception is recorded in
[ADR-0010](../adr/0010-spec-kit-and-executable-gates.md#amendment-2026-09-28).

## 1. Slices at a glance

| Slice | Delivers | Estimate* | Depends on | Milestone |
| --- | --- | --- | --- | --- |
| **S0 Foundation** | Clean `maestro-core` (canonicalization only) under org gates; the repositories S1 needs published; Spec Kit | 4–6 days (measured scope) | — | — |
| **S1 Knowledge kernel + hybrid RAG** | Kernel building blocks; Control-M collection imported, published and searchable through MCP; eval suites; first model bake-off | 12–15 days | S0 | **M1 "Ask Control-M"** |
| **S2 Knowledge graph** | Fact store, extraction, entity resolution, Neo4j projection, graph route, graph evals | 10–15 days | S1 | **M2 "Relationships answered"** |
| **S3 Catalog** | Copilot-native catalog v1, settings classes and overrides, compile/release/install/update with freshness and revocation, `maestro init`, host projection, intent routing, policies and static graph checks | 191 lane-hours plus review/CI reserve ([plan](../../specs/003-catalog/plan.md#risks-and-estimates)) | S1 live evidence for C08/C28 and M1 release for M3 exit; S2 G25/G27 for impact | **M3 "Catalog installable"** |
| **S4 Orchestration runtime** | Workflow graphs, durable engine, daemon, Copilot SDK + llama.cpp sessions, Cedar broker, sandbox, contracts, interrupts, extension host and event stream, test kit | 18–24 days | S3 | **M4 "First governed workflow"** |
| **S5 Capabilities + InnerSource** | Monitoring, Product Owner and Control-M orchestration-planning capabilities; scaffolder; scenario runner; a contributed capability | 10–15 days | S4 | **M5 "First contributed capability"** |
| **S6 Native acquisition** | Frontier, fetchers, extraction, policy; private BMC connectors; Python retired source by source | 15–25 days | S1 | **M6 "Python retired"** |
| **S7 Intelligence backend** | I1 memory and continuity, I2 code intelligence, I3 governed knowledge, I4 workbench | I1 4–6 wk, I2 3–4 wk, I3 3–5 wk, I4 4–6 wk | S1–S4 | **M7 "Maestro remembers"** (I1) |
| **S8 Provenance and reverse engineering** | Provenance register, analyzer extensions, behaviour contracts, the clean-room boundary | R1 1–2 wk, R2 2–3 wk, R3 2–3 wk, R4 1 wk | S1, S4, S7-I2 | **M8 "Cleared to reimplement"** |

\*Engineering days of one developer working with coding agents, excluding waits
for external approvals. Estimates are replaced by measured velocity after S1.

### Two schedules

The 08:12 decision above supersedes only the S2/S3 implementation-start dates
in these estimates. The evidence prerequisite remains for every other slice;
the dates are not delivery commitments.

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

D1–D5 are decided in the [S3 spec](../../specs/003-catalog/spec.md#clarifications).
C00's [exact inventory](../../specs/003-catalog/traceability.json) is a planned
disposition, approved by the owner, 2026-09-28 (C00 inventory approval), not
delivery evidence.

**D1 checkpoints:** C02 first supplies a small reviewed-source knowledge seed;
C08 proves init → Copilot/Pi projection → knowledge search/get/ask → remove.
They are authoring convenience, not verified installation or governed execution.
`--catalog-dir` cannot mint an attested install or bypass install/update trust.
Integrated T034/T035 and T038 live evidence gate C08 and C28; the M1 release
gates M3 exit. C00 needs only the decided D1–D5 to start. No checkpoint removes
an M3 exit criterion, and no S3 task executes a workflow graph.

| Deliverable | Detail |
| --- | --- |
| `maestro-catalog` | Parse, check, compile, verify (attestation, freshness, revocation, version floor), install, update, project, route, resolve, search, impact, explain; settings classes and override resolution; project lock |
| Bootstrap | Branded, keyboard/plain/no-color `maestro init` and the same every-setting editor on no-argument `maestro config`, generated from S1 descriptors with current value, allowed values, description and source; existing class/layer/authority restrictions. Strict workspace config, language/tone, user-approved folder trust, updates and overrides; scripted flags plus `--yes`, explicit apply, base/Rust composition and owned-file safety |
| Session preferences and trust | Free flags > safely discovered workspace > user preferences > defaults; updates/budgets only narrow. Bounded canonical BCP 47 subset; question-language answers when unset, evaluation unchanged. en/fr/es interface, English fallback otherwise; English artifacts/logs. MCP --workspace or user-only snapshot, path-free source; static native English rules. Kernel-only path trust, immutable secret deny data |
| Startup updates | Off or daily offline-safe verified discovery; default propose, user-only catalog auto before work, mandatory widening/hook consent, receipts and trusted catalog rollback. Runtime propose-only; MCP never applies, fixed ID/target/version notice only. Never update trusted roots |
| Catalog v1 content | Written from zero ([03 §1.8](03-agent-orchestration.md#18-writing-the-first-catalog)): the `feature-delivery` and `ctm-question` workflows, the Maestro orchestrator and the owner's roles they need (planner, coder, tester, reviewer; builder as a step), and only the skills, instructions, contracts, Cedar policies (default deny, destructive operations, protected paths, egress, MCP allowlist, each with allowed and denied fixtures), model profiles (`fast`, `balanced`, `deep`) for `copilot` and `llamacpp`, MCP server descriptors and discovery cards those workflows use; every file's pull request names its 08 rows |
| Kinds and model cards | C03 finite floats/structured tables and named hooks permit descriptor-only lifecycle extension. C03a adds exact kernel v2 model-card declarations; C02a supplies owner-approved winners before C28. C16h explicitly registers admitted installed cards with local evidence, reusing kernel authority and one lock-bound lookup. No implicit registration, new role, download or fake selection/qualification |
| Release | Manifests CI: check, policy tests, bundle plus SPDX JSON SBOM of pinned components/digests, per-asset checksums, attestations and verification instructions (SEC-011) |
| Hosts | Copilot/Pi projection (preview, apply, owned removal); four-client local MCP registration (Pi, Codex, Claude Code, Copilot CLI); only Copilot `preToolUse` → `maestro policy check` in S3. Other hooks wait for S4 trusted event/identity adapter qualification |
| Static graph checks | C22a/C22b check all twelve [03 §2.3](03-agent-orchestration.md#23-compile-time-validation) rules; unsupported constructs are refused. Execution and live role qualification remain S4 |
| Routing | Exact-ID/local lexical baseline, scoped discovery cards, conditional measured hybrid; C27a's separate catalog edge schema/adapters over S2 G27's public typed-edge port after G25 qualification, then C27 exact impact. No evidence-span claims or implicit closure fallback |
| Spike | ADR-0005 retained; C03 uses integrated [C01 evidence](../../specs/003-catalog/research/hosts.md) (`0be954b`). Agent sidecars confirmed (Copilot 1.0.88 ignores agent metadata); skills use specification-backed `metadata`, not support inferred from the silent unknown-key control. A host warning reopens ADR-0005's sidecar decision |
| Comparison pass (after M3) | The earlier catalog read once against the new one; each recovered item its own pull request citing it; the rest listed with the reason |

**Exit criteria:** a tagged bundle is attested by manifests CI and verified by
pinned `gh` through `maestro catalog install`; revoked, expired or replayed
bundles are refused. D2 requires refresh within five minutes during use and
offline expiry within 24 hours. An unclassified setting is rejected;
`maestro init` previews and applies the base and base-plus-Rust compositions
cleanly and stops on collisions. Projection is idempotent and removes only
unchanged owned content. Every policy and static graph rule
has passing allowed and denied neighbours. C28 requires M1 release, S2 impact,
live host/release evidence (including Copilot `preToolUse` allow, deny and
hook-error-to-deny receipts in the OA2 host stage) and final three-platform CI.

**D5 routing exit — OA10 approved (owner, 2026-09-28):** the absolute bar is
held-out matchable **top-1 ≥ 90 %**, requiring a correct first selection rather
than shortlist inclusion. This is the owner's dated amendment to D5; its
original **top-3 ≥ 90 %** bar is retained as history, not current acceptance.
C23 freezes
100+ independently reviewed CORE public/synthetic cases (at least 20 tuning,
80 held-out, 60 held-out matchable), ten eligible synthetic workflow candidates
and a digest-pinned compiled bundle/eligibility fixture. Required dependency
completeness is 100 %. Report top-1/top-3, correct no-match, clarification, unnecessary
context, distractors and latency separately. Under [S3 D5's protocol](../../specs/003-catalog/plan.md#d5-baseline-routing-and-measured-hybrid),
hybrid ships only if its held-out top-1 difference has a strictly positive
lower bound in the seeded 95 % paired-bootstrap interval; otherwise ship the
passing baseline and retain the failed comparison. This explicitly replaces
the older unconditional "routing beats the structured baseline" exit. A real M3 install
returns `incompatible` (not qualified until S4) for executable workflows;
synthetic success does not qualify a live role or model.

**Shared migrations:** the supervisor allocates each next-free number at
landing above every landed or reserved number on main, S1/S2/S3 integration
branches and the deployment-modes track. C00 reserves no number or gapped block;
later schema changes receive new numbers, never edits to applied migrations.

The 11:25–12:05 owner amendments add S3 exit evidence for config discovery/
precedence, conversational/artifact separation, four-client instruction delivery,
menu accessibility/visual acceptance, trust-backed real file denials, and startup
off/propose/catalog-auto/approval/rollback/offline cases. Plain init serves C08
without TUI/OA9; branded visual acceptance is required before M3, not the first
owner loop. The revised task set is
[57 tasks / 191 lane-hours](../../specs/003-catalog/tasks.md), including the evening
amendment and C16h's 3 h split. C08's dependency closure remains 72 h; M3 requires
188 h, then C29 3 h. With the unchanged 16–24 h review/CI reserve, allow 207–215 h;
the older calendar estimate is not a commitment for this expanded scope. Configuration, path trust,
release sources and client delivery remain small modules with explicit ports;
new adapters do not change callers or weaken mandatory controls.

Before C17, the supervisor synchronizes S1's landed registry/kernel APIs into S3.
The 21:02 ruling makes S1 registry names canonical; C17 adds only missing catalog
descriptors through the shared port, never a second settings-key list. C05g/C05k
generate both editor entry points from that registry without per-setting screens.
Model-card registration is local-evidence-only in M3; each machine qualifies its
own card. Each answer returns a registry card ID resolving to an immutable card;
the kernel stores no answers. S1's latest-registration answerer lookup and the
fact that re-registering an earlier card does not restore it remain explicit
known gaps until the post-M1 selection fix. C16h tests A, then B, then A → B;
C18 explains the card an ask would use now. S2 G17 brings extractor before M3;
query_expander still refuses through the kernel's unknown-role path.

### S4 Orchestration runtime → M4

| Deliverable | Detail |
| --- | --- |
| Graph execution | Execute S3-validated graphs and enforce the twelve [03 §2.3](03-agent-orchestration.md#23-compile-time-validation) rules at runtime; static compilation belongs to S3 C22a/C22b |
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

**S4 session-preferences launch-adapter obligation:** consume S3 C05e's English
instruction fragment in every launched, resumed and delegated agent. Acceptance
inspects actual adapter instruction payloads on both provider routes for all
three tones and a language without built-in interface strings, retaining the
selected tag and English code/commit/name/identifier/log/documentation rule.
S3 construction/delivery tests are not this launch proof or proof of host obedience.

**S4 non-Copilot workspace-trust hook obligation:** qualify Pi, Codex and Claude
Code's trusted event/identity adapters against the same `WorkspaceTrust` port.
Actual hook/effect tests must deny outside writes, secret reads inside trust,
link escapes and agent-shell trust administration (even a correct
`--confirm-path`), with allowed neighbours and zero
executor calls on denial. These extend the existing S4 hook qualification,
not S3's claimed enforcement. S4 task/session admission must also hold the
shared update lifecycle lease so no activation/rollback happens mid-task.

### Named S3 follow-ups

- **Model-card evidence import after M1:** explicit local `--evidence DIR`,
  matching every imported file to its declared digest. No download or claim that
  another machine's backend/runtime/hardware qualification transfers. M3 requires
  evidence already in the local kernel; an answer journal is not part of S3.
- **Glossary and source-class catalog kinds:** after the glossary spike shows
  value, each becomes reviewed, versioned per-collection manifest content rather
  than only a kernel binding. Reuse C03's finite/structured fields and the kind
  descriptor contract; the synthetic glossary fixture is not this delivery.
- **Runtime auto-apply with the installer:** define released-installation layout,
  cross-platform startup handoff, idle exclusion, retained binaries/state,
  receipts and current-trust rollback before automatic runtime activation. S3
  only proposes a verified runtime release and exact approved install command;
  catalog off/propose/auto uses the existing lifecycle.
- **Roots-based workspace detection, once a client-qualified channel exists:**
  MCP roots arrive after initialization and there is no standard instructions-
  changed notification. S3 uses explicit --workspace or user preferences only;
  no per-client notification workaround is implied.

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

Frontier, HTTP and rendered fetchers (three transports), extraction with
fidelity receipts and the extractor contract, the strict executable JSON source
policy with its frozen exclusion registries, URL identity and decision order,
run receipts; private connectors as extensions for vendor docs and wiki, KB and
community, GitHub repositories, attachments and the distribution catalogue; the
bounded protected preflight before any broad live run. Offline qualification
(HTML fidelity, browser and session boundary, one extractor per media type,
policy) needs no protected login. **Exit:** each source family matches the
Python output on fixtures and a sampled live diff, including full, incremental,
resume, withdrawal and repair cases, then is cut over after independent review;
scheduled refresh runs natively; the Python producer for that family is retired.

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
- macOS and Windows qualification (after S4 on Linux).
- Multi-user or team-server deployment of the kernel, SSO, high availability
  and replication.
- Learned sparse and late-interaction retrieval unless a bake-off selects them.
- Inbound webhooks, the broker bridge and WebAssembly extensions until a real
  integration asks for them.
- The structured-data (SQL) knowledge profile until a domain needs it.
- **Notify-only update checks for owner-managed components (Qdrant, router runtime,
  host clients, models):** propose-only after each has an approved source; never
  apply or download those components. S3 manages only released Maestro and
  installed catalogs/component closures. Pinned `gh` and Cargo resolution stay
  owner/build-managed, outside that updater.

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
