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
| **S6 Native acquisition** | Frontier, fetchers, extraction, policy; private BMC connectors; Python retired source by source | 15–25 days | S1 | **M6 "Python retired"** |
| **S7 Intelligence backend** | I1 memory and continuity, I2 code intelligence, I3 governed knowledge, I4 workbench | I1 4–6 wk, I2 3–4 wk, I3 3–5 wk, I4 4–6 wk | S1–S4 | **M7 "Maestro remembers"** (I1) |

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
| Journal | Per-stream sequences, durable cursors and acknowledgements; projections and telemetry consume them ([07 §7](07-extensibility.md#7-delivery-by-slice)) |
| `maestro` binary | CLI (`knowledge …`, `eval …`, `status`, `doctor`, `setup`, `backup`, `restore`) and MCP stdio server (`knowledge_collections`, `knowledge_search`, `knowledge_get`, `knowledge_ask`) |
| `ctm-collection` | Corpus exporter from the existing Python outputs, `collection.json`, the quality ledger, a 100+ question golden set (FR/EN, ~15 % unanswerable) validated by the owner on a stratified sample |
| Bake-off round 1 | Embedder, reranker, answerer (see [05 §3](05-platform-and-operations.md#3-model-selection)) |
| Services | Qdrant unit through `maestro setup` |

**Exit criteria:**

1. The full corpus imports with every refusal explained in the report, and
   every document carries a quality outcome; only accepted documents are
   indexed.
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

**Exit criteria:** `feature-delivery` completes on the release-canary repository
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
| R8 | Eval set not representative | Medium | High | Owner validates a stratified sample; add questions from real usage; track per-type results |
| R9 | Vendor content or connectors leak publicly | Low | High | Private repository, gitleaks and path checks, scope tags, no corpus in public CI |
| R10 | Single maintainer bandwidth | High | High | Two-track schedule only if review capacity exists; slices small enough to finish |
| R11 | Requirements lost between plans | Medium | High | [08](08-traceability.md) is updated by every slice spec; a requirement changes status only with a stated reason |
| R12 | Bare identifiers collide across documents (C01, U03 mean different things in different sources) | High | Medium | Namespaced IDs in 08 (`chat.C01`, `delivery.U03`, `product.C01`) |
| R13 | An extension becomes an attack path | Medium | High | Out-of-process, sandboxed, least-privilege principals, declared effects, activation separate from publication (ADR-0013) |
