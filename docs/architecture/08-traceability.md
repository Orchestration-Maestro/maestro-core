# 08 Traceability

**Rule:** the fresh start (ADR-0001) applies to code and history, never to
requirements. Every requirement, owner decision and technology choice found in
the organization's earlier material appears below with a status and the place
that now carries it. A status changes only with a stated reason; each slice's
spec re-checks the rows it touches.

**Scope:** sources inside the organization only (owner instruction,
2026-09-24). Repositories outside the organization are not sources; facts that
came only from them were removed from this design or restated as generic
requirements.

## 1. Sources

Identifiers are namespaced, because the same bare ID (`C01`, `U03`) means
different things in different sources.

| Prefix | Source | Contents |
| --- | --- | --- |
| `owner` | The owner's own requirements: design conversations and this design session | Primary authority |
| `chat` | Design conversation 1, platform and manifest (messages M001–M059) | Reference design, repository review, implementation plan, two repositories, catalog MCP, SDK controls, testing, providers and telemetry |
| `rag` | Design conversation 2, knowledge pipeline and graph (N001–N075) | Local Rust RAG, GraphRAG, Qdrant + Neo4j, fusion and reranking, extraction tools, canonicalization, chunking, tokenizer and indexing prompts, unified analysis |
| `core` | Fresh maestro-core design (2026-09-21) | Golden foundation, module rules, quality contract, selective migration, native portability, authorization and artifacts |
| `ingest` | Full ingestion design, source-policy proposal and acquisition research (2026-09-22) | Native acquisition lifecycle and sessions; the owner-approved inventory is private |
| `delivery` | Unified delivery plan (2026-09-22) | Tasks U01–U18 with folded F0–F8, Q1–Q4, D1–D5; dispositions C01–C30; corrections R01–R11 |
| `product` | Provider analysis: master product specification, capability and Graphify decisions made with the owner, native Rust product direction, visual workbench, native ingestion specification | C01–C16, M01–M11, U01–U15, J01–J08, A01–A20, N01–N10, D01–D11, CD1–CD8, GD1–GD5, V01–V12 |
| `revtools` | Reverse-engineering analysis supplied by the owner (2026-09-24): whether several tools can be studied and rebuilt as one, and which instruments do it | Legal boundary, licence and porting rules, per-project capability map, recommended architecture, instrument chain, defensible process (§19) |

The source documents stay archived with the S0 snapshot; those carrying vendor
specifics move to the private collection repository (ADR-0009).

## 2. Status legend

| Status | Meaning |
| --- | --- |
| **Kept** | In the design as stated in the source |
| **Adapted** | Kept with a change; §15 gives the reason |
| **Deferred** | Kept and scheduled for a later slice or a demand-driven profile |
| **Dropped** | Not kept; §16 gives the reason and who decided |
| **Open** | Needs an owner decision; listed in §17 |

These statuses are design dispositions, not implementation or test evidence. A
slice's delivery map must identify its exact requirement rows, delivered
portion, integrated code or test evidence, and remaining work.

## 3. Owner requirements

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| owner.m001.manifest | A central agentic framework defined through a manifest, for transparency and InnerSource | Kept | [03 §1](03-agent-orchestration.md#1-the-catalog-maestro-manifests), ADR-0005, ADR-0012 |
| owner.m001.load | Remove the framework's cognitive load from developers | Adapted: no framework plumbing; judgement and approvals stay human | [03 §1.6–1.7](03-agent-orchestration.md#16-configuration-and-overrides) |
| owner.m001.orchestrator | A central main agent as orchestrator | Kept | [03 §2.5–2.6](03-agent-orchestration.md#25-the-orchestrator-maestro-agent) |
| owner.m001.team | A simple DevOps team: coder, tester, builder, reviewer | Kept (builder is a deterministic step) | [03 §2.6](03-agent-orchestration.md#26-roles) |
| owner.m001.capabilities | Capability agents (monitoring, orchestration, PO) in sections owned by their codeowners; anyone may open a pull request | Kept | [03 §10](03-agent-orchestration.md#10-innersource-flow-s5), [06 S5](06-roadmap.md#s5-capabilities--innersource--m5) |
| owner.m001.cli | An internal CLI that bootstraps projects | Kept | [03 §1.7](03-agent-orchestration.md#17-project-bootstrap-maestro-init) |
| owner.m001.sdk | The Copilot SDK in Rust | Kept | [03 §3.1](03-agent-orchestration.md#31-copilot-sdk-github-copilot-sdk-1014) |
| owner.m001.llamacpp | An OpenAI-compatible llama.cpp endpoint | Kept | [03 §3.2](03-agent-orchestration.md#32-providers-and-profiles), [05 §3.4](05-platform-and-operations.md#34-provider-configuration) |
| owner.m001.laptop | Runs on laptops; installation and integration through the Copilot directory | Kept | [03 §1.5](03-agent-orchestration.md#15-native-projection-convenience-mode), [05 §1](05-platform-and-operations.md#1-reference-environment) |
| owner.m001.guardrails | A central guardrails directory and destructive-commands manifest; every pre-tool use validated; native event hooks | Kept: the broker is authoritative, hooks are defence in depth | [03 §3.3–§4](03-agent-orchestration.md#4-the-policy-broker-cedar) |
| owner.m001.contracts | Validate agents' output contracts; block bad behaviour | Kept | [03 §6](03-agent-orchestration.md#6-handoff-contracts-and-acceptance) |
| owner.m020 | A concrete enterprise plan; fully Rust, Python only when there is no choice | Kept | [06](06-roadmap.md), [04 §1](04-intelligence-backend.md#1-scope-and-stance), [05 §1](05-platform-and-operations.md#1-reference-environment) |
| owner.m024 | Two repositories: manifest definition and a detached laptop runtime | Kept | ADR-0012, [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install) |
| owner.m028 | An MCP with RAG over a large catalog so the orchestrator finds the best workflow, agents and skills from intent | Kept (workflow first) | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow) |
| owner.m032 | Concrete enforceable cases; persona injection; handoff validation with hooks; every SDK lever (guardrails, models, thinking); defaults users can override | Adapted: only the *free* class is freely overridable | [03 §1.6](03-agent-orchestration.md#16-configuration-and-overrides), [§3](03-agent-orchestration.md#3-agent-sessions), [§6](03-agent-orchestration.md#6-handoff-contracts-and-acceptance) |
| owner.m037 | Testing with the SDK, mocking, a test framework | Kept | [03 §9](03-agent-orchestration.md#9-test-layers) |
| owner.m058 | Two providers (Copilot, llama.cpp); telemetry on everything; benchmarks; improvement | Kept | [05 §3–§5](05-platform-and-operations.md#3-model-selection) |
| owner.n001 | Understand uncensored models | Kept as a policy | [05 §3.2](05-platform-and-operations.md#32-candidate-pools) |
| owner.n007 | The best fully local Rust pipeline from crawl to answer, automated | Kept | [01](01-knowledge-pipeline.md), [02](02-retrieval-and-knowledge-graph.md) |
| owner.n012 | A graph database and how agents consume it (MCP, agents) | Kept | [02 §8–§9](02-retrieval-and-knowledge-graph.md#8-knowledge-graph-s2) |
| owner.n017 | Is Oxigraph dead; what is SurrealDB | Answered: Oxigraph is maintained but RDF is not needed; SurrealDB not chosen (§16) | §16 |
| owner.n020 | Qdrant and Neo4j combined with fusion | Kept | ADR-0003, ADR-0004, [02 §4](02-retrieval-and-knowledge-graph.md#4-fusion) |
| owner.n024 | An Archify diagram of the whole architecture | Deferred: regenerate the atlas from this design once S0 publishes it | [06 S0](06-roadmap.md#s0-foundation) |
| owner.n029, n065 | BM25 versus BGE-M3; why not BM25 | Kept: both, at different layers | [01 §8](01-knowledge-pipeline.md#8-l6-representations), [02 §3](02-retrieval-and-knowledge-graph.md#3-retrieval-routes) |
| owner.n031 | Source to answer step by step, with fusion, reranking and deduplication | Kept | [01 §6](01-knowledge-pipeline.md#6-l4-deduplication), [02 §4–§6](02-retrieval-and-knowledge-graph.md#4-fusion) |
| owner.n039 | Are Spider, Crawl4AI and Docling needed; native Rust preferred | Kept | [01 §2.2.2](01-knowledge-pipeline.md#222-transports), [§3](01-knowledge-pipeline.md#3-l2-extraction-and-normalization) |
| owner.n047, n053 | The next step once content is Markdown | Kept | [01 §5–§7](01-knowledge-pipeline.md#5-l3-canonicalization) |
| owner.n057 | Acceptance criteria before declaring a stage complete | Kept | §11.4 |
| owner.n062 | The tokenizer contract for chunking | Adapted (§15 A6) | ADR-0008 |
| owner.n071 | A unified analysis | Kept | This design |
| owner.ext | Entry and exit points; integrations plug in and out on events, without core changes, at any scale | Kept | [07](07-extensibility.md), ADR-0013 |
| owner.catalog | The catalog's content is written from zero, without bias from an earlier catalog; items may be recovered afterwards (2026-09-24) | Kept | [03 §1.8](03-agent-orchestration.md#18-writing-the-first-catalog), [06 S3](06-roadmap.md#s3-catalog--m3), ADR-0001 |
| owner.session | Fresh start with canonicalization only; value slices; Copilot-native formats; public core, manifests and router, private vendor material; organization directory only; SQLite + artifacts + Qdrant; Spec Kit; no preselected model; intelligence backend later with its building blocks now | Kept | ADR-0001–0012, [04 §3](04-intelligence-backend.md#3-the-kernel-building-blocks) |
| owner.product | Decisions taken during the provider analysis (CD1–CD8, GD1–GD5, native desktop) | Kept | §13.2, [04 §2.1](04-intelligence-backend.md#21-decisions-already-taken-with-the-owner) |

## 4. Platform: catalog, bundle and configuration

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| chat.M006 principle | The main agent coordinates; deterministic software authorizes, gates and accepts | Kept | [03](03-agent-orchestration.md) principles |
| chat.M006 layers | Definitions → deterministic compiler → immutable bundle → laptop runtime → enforcement and evidence | Kept | [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install) |
| chat.M006 vocabulary | Agent, capability, tool, skill, policy, workflow, team; infrastructure orchestration ≠ agent orchestration | Kept | CONTEXT.md, [03 §2.6](03-agent-orchestration.md#26-roles) |
| chat.M006 layout, M023 layout | Repository layout with agents, teams, workflows, capabilities, guardrails, tools, skills, models, compatibility, presets | Adapted (§15 A10) | [03 §1.1](03-agent-orchestration.md#11-layout) |
| chat.M006 compiler | Structure, references, duplicate IDs, cycles, permission analysis, contract compatibility, normalization, native generation; never last-file-wins | Kept: S3 static checks, S4 runtime execution (§21) | [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install), [§2.3](03-agent-orchestration.md#23-compile-time-validation), [§1.6](03-agent-orchestration.md#16-configuration-and-overrides) |
| chat.M006 explain | Explain every effective setting (decision, reason, source, requester) | Kept | [03 §1.6](03-agent-orchestration.md#16-configuration-and-overrides) |
| chat.M006 lockfile | Pin SDK and runtime, component digests, model identity, template, llama.cpp build, sandbox and OS profiles | Kept | [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install) (project lock) |
| chat.M006 project file | Tiny project configuration that cannot redefine hooks, orchestration, authentication or destructive rules | Kept | [03 §1.7](03-agent-orchestration.md#17-project-bootstrap-maestro-init) |
| chat.M006 transparency | Declared, effective and observed views; sanitized external export | Kept | [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install), [§10](03-agent-orchestration.md#10-innersource-flow-s5) |
| chat.M006 release | Signed immutable bundles; evaluation → canary → stable; emergency revocation; offline validity | Kept | ADR-0015, [05 §5](05-platform-and-operations.md#5-evaluations-and-benchmarks) |
| chat.M019 maturity, M023 step 4 | Placeholder → authored → reviewed → qualified → retired; status never grants tools | Kept | [03 §1.2](03-agent-orchestration.md#12-formats-copilot-native-first) |
| chat.M019 descriptor | Agent schema metadata restrictions; descriptor beside the Markdown | Adapted (§15 A11) | [03 §1.2](03-agent-orchestration.md#12-formats-copilot-native-first), [06 S3](06-roadmap.md#s3-catalog--m3) spike |
| chat.M019 ownership | Section schema; ownership spans skills, contracts, policies, fixtures, implementation; CODEOWNERS generated from one model; dual approval by a separate check | Kept | [03 §10](03-agent-orchestration.md#10-innersource-flow-s5) |
| chat.M027 detached | The laptop consumes a verified bundle, never clones or runs the manifest repository; no checkout, Rust or Python needed | Kept | [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install) |
| chat.M027 validation split | Definition validity in the catalog; load safety, evidence and enforcement in the runtime | Kept | [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install) |
| chat.M027 invariants | A bundle cannot disable verification, invent identity, bypass the broker or forge receipts | Kept | [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install) |
| chat.M027 contract crate | A small versioned contract published by the runtime and consumed by the catalog compiler | Adapted (§15 A23) | [05 §8](05-platform-and-operations.md#8-cicd) |
| chat.M027 versions | Runtime, bundle and bundle-protocol versions; required features and tool contracts; refusal before governed execution | Kept | [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install) |
| chat.M027 change impact | Which changes need a runtime release | Kept | [03 §10](03-agent-orchestration.md#10-innersource-flow-s5) (integration ladder) |
| chat.M027 pipelines | Separate release pipelines and publisher identities; untrusted jobs without credentials | Kept | [03 §10](03-agent-orchestration.md#10-innersource-flow-s5), ADR-0015 |
| chat.M027 TUF | Authentic is not the same as currently permitted: freshness, expiry, rollback protection, offline window | Kept | ADR-0015 |
| chat.M036 classes, M039 policy | Free, bounded, additive, locked; exactly one class per setting; unknown and unclassified keys rejected; preference precedence; role-local capability values | Kept | [03 §1.6](03-agent-orchestration.md#16-configuration-and-overrides) |
| chat.M036 objects | Persona, agent, team, workflow, handoff, skill, instruction, tool, MCP server, model profile, policy, execution profile, result contract, project preset | Kept | [03 §1.1–1.2](03-agent-orchestration.md#11-layout) |
| chat.M036 defaults | Bootstrap defaults (profile, reasoning, output, concurrency, depth, tool calls, repairs, routing candidates, MCP timeout, memory, extensions, fallback, evidence, hooks) | Kept | [03 §1.6](03-agent-orchestration.md#16-configuration-and-overrides) |
| delivery.§2.2 | Four separate contracts: catalog release, ingestion policy, completion manifest, runtime envelopes | Kept | [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install), [01 §2.2.7](01-knowledge-pipeline.md#227-source-policy-robots-and-url-identity), [01 §12](01-knowledge-pipeline.md#12-commands), [03 §6](03-agent-orchestration.md#6-handoff-contracts-and-acceptance) |

## 5. Agents, workflows, handoffs and acceptance

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| chat.M006 roles, M023 step 12 | Main, coder, tester, builder, reviewer, product owner, monitoring, infrastructure with default boundaries | Kept | [03 §2.6](03-agent-orchestration.md#26-roles) |
| chat.M019 roles | Maestro and Steward complementary; Bootstrapper and Translator authored | Deferred: roles of an earlier catalog wait for the comparison pass (owner.catalog) | [03 §1.8](03-agent-orchestration.md#18-writing-the-first-catalog) |
| chat.M006 change workflow | Scope → criteria → plan → approval → change/test → build → specification review → standards review → validation → publication approval; smallest sufficient workflow | Kept | [03 §2.2](03-agent-orchestration.md#22-example) |
| chat.M019 gauntlet | The comparative improvement loop stays optional | Kept | [03 §2.6](03-agent-orchestration.md#26-roles) |
| chat.M006 workers | Host-managed worker sessions with explicit identity, tools, contracts, budgets; delegation is a bounded grant | Kept | [03 §3.1](03-agent-orchestration.md#31-copilot-sdk-github-copilot-sdk-1014) |
| chat.M006 results, M036 result_submit | A typed result submission tool; model payload versus trusted metadata; structural, referential, snapshot, workflow and semantic checks; bounded repair | Kept | [03 §6](03-agent-orchestration.md#6-handoff-contracts-and-acceptance) |
| chat.M036 handoff_submit, M039 criteria | Host envelope; the host keeps reference criteria; readiness depends on the workflow | Kept | [03 §6](03-agent-orchestration.md#6-handoff-contracts-and-acceptance) |
| chat.M019 machine contracts | Agent result, handoff packet, review result, execution receipt, policy decision | Kept | [03 §6](03-agent-orchestration.md#6-handoff-contracts-and-acceptance) |
| chat.M006 durable state | Planned, authorized, started, completed, failed, unknown; no blind retries; resume is a fresh authorization | Kept | [03 §2.4](03-agent-orchestration.md#24-the-engine-durable-event-sourced-execution) |
| chat.M039 superpowers | Framing, design, implementation, verification, review, completion as verifiable stages; imported approvals kept or named variants | Kept | [03 §1.2](03-agent-orchestration.md#12-formats-copilot-native-first), [§2.2](03-agent-orchestration.md#22-example) |
| chat.M006 memory | Task memory separate from project knowledge; no silent cross-project memory | Kept | [03 §7](03-agent-orchestration.md#7-memory-inside-runs), [04 §4](04-intelligence-backend.md#4-phase-i1--memory-and-continuity) |
| delivery.R02 | Decision subjects; exit zero with zero relevant tests fails a test criterion | Kept | [03 §4](03-agent-orchestration.md#4-the-policy-broker-cedar), [§6](03-agent-orchestration.md#6-handoff-contracts-and-acceptance) |
| delivery.R08 | Imported approvals and separate review contexts | Kept | [03 §1.2](03-agent-orchestration.md#12-formats-copilot-native-first), [§2.6](03-agent-orchestration.md#26-roles) |

## 6. SDK sessions, providers, models and hooks

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| chat.M006 packaging | Certified SDK and runtime assets, out-of-process stdio, empty client mode is not a sandbox | Kept | [03 §3.1](03-agent-orchestration.md#31-copilot-sdk-github-copilot-sdk-1014) |
| chat.M036 persona | Persona composed from identified fragments with provenance and digests | Kept | [03 §3.1](03-agent-orchestration.md#31-copilot-sdk-github-copilot-sdk-1014) |
| chat.M036 hooks, delivery.§3.3 | The ten Rust hooks with their exact limits; file hooks separate; CLI-only events need adapters | Kept | [03 §3.3](03-agent-orchestration.md#33-hook-mapping) |
| chat.M039 SDK defaults | Tools default to all, model falls back to parent, skills not inherited, managed settings not persisted, no handler ≠ deny | Kept | [03 §3.1](03-agent-orchestration.md#31-copilot-sdk-github-copilot-sdk-1014) |
| chat.M036 MCP | MCP server descriptors; new tools not approved; resources and prompts gated; `_meta` not identity; MCP Apps off | Kept | [03 §3.3](03-agent-orchestration.md#33-hook-mapping) |
| chat.M036 managed settings | A restrictive layer re-supplied on resume; no approve-all | Kept | [03 §3.1](03-agent-orchestration.md#31-copilot-sdk-github-copilot-sdk-1014) |
| chat.M036 models | Model, allowed models, reasoning effort and summary, context tier; `fast`, `balanced`, `deep` profiles; unsupported effort is a diagnostic; no fake universal temperature | Kept | [03 §1.6](03-agent-orchestration.md#16-configuration-and-overrides), [05 §3.3](05-platform-and-operations.md#33-protocol) |
| chat.M036 interaction | User question, elicitation, plan-exit and mode-switch handlers wired to a real interface; a business question is not a security approval | Kept | [03 §2.4](03-agent-orchestration.md#24-the-engine-durable-event-sourced-execution) (interrupts) |
| chat.M036 raw config | No raw SDK configuration escape hatch; unqualified options refused or experimental | Kept | [03 §3.1](03-agent-orchestration.md#31-copilot-sdk-github-copilot-sdk-1014) |
| chat.M059 providers | `copilot` and `llamacpp` routes; experimental registry not relied on; one profile per worker; mixed roles only with authorized transfer; no implicit fallback | Kept | [05 §3.4](05-platform-and-operations.md#34-provider-configuration) |
| chat.M059 model profile | Model, conversation, generation, server, execution and qualification records; two quantizations or templates are two profiles | Kept | [05 §3.3](05-platform-and-operations.md#33-protocol) |
| chat.M006 compatibility | Full tool round trip, streaming, call IDs, malformed arguments, truncation, cancellation, context exhaustion, provider errors | Kept | [05 §3.3](05-platform-and-operations.md#33-protocol) |
| chat.M059 capitalize A | A qualification registry by role, model and hardware | Kept | [05 §3.3](05-platform-and-operations.md#33-protocol) |
| delivery.U04, rag.N075 MSRV | SDK MSRV 1.94 versus the crate's 1.85 | Kept as a rule | [05 §9](05-platform-and-operations.md#9-engineering-baseline) |
| rag.N075 profiles | Strict local versus managed Copilot | Kept | [05 §1](05-platform-and-operations.md#1-reference-environment) |
| product.GD1 | Provider configuration add, list, show, remove in one registry | Kept | [05 §3.4](05-platform-and-operations.md#34-provider-configuration) |

## 7. Enforcement and containment

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| chat.M006 broker | Typed broker path; hooks and permission handler share one evaluator; final arguments authorized | Kept | [03 §4](03-agent-orchestration.md#4-the-policy-broker-cedar) |
| chat.M006, M023 step 8 | Typed operations before shell strings | Kept | [03 §4](03-agent-orchestration.md#4-the-policy-broker-cedar) |
| chat.M006 destructive | A destructive-operation catalogue by effect and scope, with fixtures | Kept | [03 §4](03-agent-orchestration.md#4-the-policy-broker-cedar) |
| chat.M006 approvals | Bound to operation, arguments, target, actor, snapshot, policy digest, expiry, single use; hard prohibitions not approvable; break-glass separate | Kept | [03 §4](03-agent-orchestration.md#4-the-policy-broker-cedar) |
| chat.M006 containment | Scratch workspace, no home or credential access, network rules, protected bundles and evidence, process-tree kill, bounded output, path-race resistance | Kept | [03 §5](03-agent-orchestration.md#5-sandbox) |
| chat.M023 step 7 | Cedar in Rust; deny on evaluation errors and missing facts; scenarios as fixtures | Kept | [03 §4](03-agent-orchestration.md#4-the-policy-broker-cedar), ADR-0007 |
| chat.M023 step 9 | A real sandbox profile; refuse to start when containment is missing; inference access separate from build network | Kept | [03 §5](03-agent-orchestration.md#5-sandbox) |
| chat.M006 threat model | Honest boundary: not tamper-proof against the laptop's administrator | Kept | [03 §5](03-agent-orchestration.md#5-sandbox), [05 §6.3](05-platform-and-operations.md#63-threat-model-boundary) |
| chat.M006 audit | Audit on the broker path; no hidden reasoning or raw prompts by default | Kept | [05 §4](05-platform-and-operations.md#4-observability) |

## 8. Catalog discovery and routing

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| chat.M031 principle | Search discovers; the manifest defines the arrangement; the runtime authorizes; evidence decides | Kept | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow) |
| chat.M031 workflow first | Retrieve workflows, then resolve declared roles, skills, steps and checks | Kept | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow) |
| chat.M031 API | `catalog_route`, `catalog_resolve`, `catalog_search`; typed outputs; statuses including no match and clarification; context bound to the principal | Kept | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow), [§8](03-agent-orchestration.md#8-mcp-surface) |
| chat.M031 cards | Discovery cards with use-when and avoid-when; dependencies as exact data; one card per entry | Kept | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow) |
| chat.M031 hybrid | Dense + BM25 + RRF; eligibility filters in every branch; optional rerank | Adapted: D5 enables hybrid only on measured paired gain; otherwise lexical (§21) | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow) |
| chat.M031 optimal | Smallest qualified workflow; scores are not probabilities; feedback never rewrites routing automatically | Kept | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow) |
| chat.M031 separation | Catalog discovery separate from knowledge RAG (collections, pipelines, scopes) | Kept | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow), [02 §1](02-retrieval-and-knowledge-graph.md#1-admission-and-scope) |
| chat.M031 offline | Cached bundle and cards; exact-ID and local lexical fallback | Kept | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow) |
| chat.M031 publication | Complete index generation checked before a release becomes discoverable; responses name snapshot and generation | Kept | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow), [01 §9](01-knowledge-pipeline.md#9-l7-indexing-and-publication) |
| chat.M031 security | Credentials behind the service; reader and writer identities; no token passthrough; cache partitioning; DNS-rebinding protection for HTTP | Kept | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow), [05 §6.1](05-platform-and-operations.md#61-threat-model-by-trust-boundary) |
| chat.M031 delivery | Baseline structured routing before Qdrant; evals with adversarial cases | Kept | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow) |
| chat.M031 service | A separately deployable catalog MCP service | Adapted (§15 A24) | [06 §3](06-roadmap.md#3-explicitly-deferred) |

## 9. Bootstrap, native projection and InnerSource

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| chat.M019 bootstrap, M023 step 13 | Deterministic bootstrap: inspect without running scripts, preset, bundle, preview, apply, validate, record ownership and lock | Kept | [03 §1.7](03-agent-orchestration.md#17-project-bootstrap-maestro-init) |
| chat.M019 overlays | Language overlays; test the composed output; strict-JSON conflicts in templates | Kept (Rust first; another language when a project needs it) | [03 §1.7](03-agent-orchestration.md#17-project-bootstrap-maestro-init), [06 S3](06-roadmap.md#s3-catalog--m3) |
| chat.M023 step 13 | Three-way merge for generated files | Deferred: stop-on-collision first, three-way updater specified separately | [03 §1.7](03-agent-orchestration.md#17-project-bootstrap-maestro-init) |
| chat.M006 native | Governed runs versus convenience native integration; generated `.github/agents`, skills and hooks; collision and drift detection; owned files only | Kept | [03 §1.5](03-agent-orchestration.md#15-native-projection-convenience-mode) |
| chat.M006 CLI | `init`, `run`, `explain`, plus doctor, config, capability, policy, integrate, session, update, rollback, uninstall, audit export | Kept (noun-verb `maestro` commands) | [03 §1.3–1.7](03-agent-orchestration.md#13-check-compile-release-install), [01 §12](01-knowledge-pipeline.md#12-commands) |
| chat.M023 operating model | Platform, security, capability maintainers and application teams; contribution path | Kept | [03 §10](03-agent-orchestration.md#10-innersource-flow-s5) |
| chat.M006 capability package | Maintainer, backup, maturity, environments, evaluations, deprecation, limitations; scaffolder | Kept | [03 §10](03-agent-orchestration.md#10-innersource-flow-s5) |
| chat.M027 ladder, delivery.R10 | Compose tools → out-of-process tool or extension → core release | Kept | [03 §10](03-agent-orchestration.md#10-innersource-flow-s5), [07](07-extensibility.md) |
| product.GD2, GD4, GD5 | Four initial clients; hook administration; local-only access | Kept: four-client MCP and static ten-point hook maps in S3; Deferred: all live hooks including C20 Copilot to S4, owner v4 amendment 2026-09-30 (A28, §21) | [03 §1.5](03-agent-orchestration.md#15-native-projection-convenience-mode), [04 §1](04-intelligence-backend.md#1-scope-and-stance) |

## 10. Testing, observability, benchmarks and improvement

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| chat.M048, M057 layers, delivery.§6.1 | Deterministic, SDK transport, real runtime with scripted model, real services and containment, live qualification | Kept | [03 §9](03-agent-orchestration.md#9-test-layers) |
| chat.M048 tools | Tokio tests, nextest, insta, proptest, mockall, wiremock, testcontainers, cargo-mutants | Adapted (§15 A20) | [03 §9](03-agent-orchestration.md#9-test-layers) |
| chat.M048 SDK seams | `Client::from_streams`, test-support helpers, request handler with every unmatched path refused | Kept | [03 §9](03-agent-orchestration.md#9-test-layers) |
| chat.M048 real controls | Never mock the control under test; positive neighbours; spy executor; two frontiers | Kept | [03 §9](03-agent-orchestration.md#9-test-layers) |
| chat.M057 scenarios | Scenario documents as data; precompiled runner; contributor path | Kept | [03 §9](03-agent-orchestration.md#9-test-layers) |
| chat.M048 replay, delivery.R03 | Strict record and replay; simulation receipts rejected in governed runs | Kept | [03 §9](03-agent-orchestration.md#9-test-layers), [§6](03-agent-orchestration.md#6-handoff-contracts-and-acceptance) |
| chat.M057 CI | Zero retries for deterministic suites; mandatory suites from the trusted baseline; distinct statuses | Kept | [03 §9](03-agent-orchestration.md#9-test-layers), [05 §8](05-platform-and-operations.md#8-cicd) |
| chat.M059 streams | Observability, execution audit, benchmark results kept separate | Kept | [05 §4](05-platform-and-operations.md#4-observability) |
| chat.M059 layers, delivery.R07 | Fifteen measurement layers | Kept | [05 §4](05-platform-and-operations.md#4-observability) |
| chat.M059 provenance | Observed, reported, estimated, unavailable; missing is not zero | Kept | README invariant 5, [05 §4](05-platform-and-operations.md#4-observability) |
| chat.M059 pitfalls | Distinct latencies, one accounting source, credits are not money, shared counters, telemetry loss | Kept | [05 §4](05-platform-and-operations.md#4-observability) |
| chat.M059 tracing | OpenTelemetry, W3C trace context, Copilot runtime export, launcher control of overrides | Kept | [05 §4](05-platform-and-operations.md#4-observability) |
| chat.M059 benchmarks | Engine, provider and workflow levels; corpus, conditions, repetitions, indicators, no magic score | Kept | [05 §5](05-platform-and-operations.md#5-evaluations-and-benchmarks) |
| chat.M059 capitalize B–H | Organizational corpus, failure taxonomy, skill and agent ablations, context measurement, post-delivery feedback, governed promotion loop, platform health | Kept | [05 §4–§5](05-platform-and-operations.md#4-observability) |
| chat.M059 views | Platform health, model comparison, workflow and skill quality, security and evidence | Kept | [05 §4](05-platform-and-operations.md#4-observability) |
| chat.M006 metrics | Setup friction, project-owned configuration, completion, false blocks, manual interventions; no individual surveillance | Kept | [05 §4](05-platform-and-operations.md#4-observability) |

## 11. Knowledge pipeline, retrieval and graph

### 11.1 Acquisition

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| rag.N011 two circuits | Ingestion and question answering are separate; never crawl per question | Kept | [01](01-knowledge-pipeline.md) introduction |
| ingest approach | Staged native engine with adapters; Python harness only for comparison; no big-bang rewrite; Python kept until per-source parity | Kept | [01 §2.2](01-knowledge-pipeline.md#22-s6--native-acquisition) |
| ingest scope | Owner-approved source rules and collection inventory are private; public docs describe only the generic acquisition contract | Kept | [01 §1](01-knowledge-pipeline.md#1-collections-sources-and-scopes), [§2.2](01-knowledge-pipeline.md#22-s6--native-acquisition) |
| ingest policy | Strict executable JSON; review-only proposal refused; frozen exclusion registries; historical exceptions as promotion decisions | Kept | [01 §2.2.7](01-knowledge-pipeline.md#227-source-policy-robots-and-url-identity), ADR-0014 |
| ingest sessions | Session reuse before credentials; KeePass entry pair; TOTP and e-mail MFA with human completion; owned browser lifecycle; bindings per source role; second synthetic collection | Kept | [01 §2.2.3](01-knowledge-pipeline.md#223-sessions-and-authentication-private-connectors) |
| ingest preflight | Bounded protected preflight (180 s, two reads, one login) | Kept | [01 §2.2.3](01-knowledge-pipeline.md#223-sessions-and-authentication-private-connectors) |
| ingest precedence | Authority → network, robots, denials → promotion → cache bypass | Kept | [01 §2.2.7](01-knowledge-pipeline.md#227-source-policy-robots-and-url-identity) |
| ingest URLs, egress | URL identity rules; policy before every hop and subresource; private addresses refused | Kept | [01 §2.2.7](01-knowledge-pipeline.md#227-source-policy-robots-and-url-identity) |
| ingest modules | Policy, frontier, capture, extract, connectors; no second scheduler | Kept | [01 §2.2](01-knowledge-pipeline.md#22-s6--native-acquisition), [07 §4.1](07-extensibility.md#41-what-an-extension-can-be) |
| ingest modes | Full, incremental, resume; caps with partial receipts; limits; cancellation | Kept | [01 §2.2.1](01-knowledge-pipeline.md#221-frontier-and-scheduling) |
| ingest discovery | Links from verified content; typed API discovery; partition subdivision and coverage | Kept | [01 §2.2.1](01-knowledge-pipeline.md#221-frontier-and-scheduling), [§2.2.4](01-knowledge-pipeline.md#224-discovery-and-enumeration) |
| ingest states, captures | Item state machine; one durable store; capture envelope; representation labels | Kept | [01 §2.2.1](01-knowledge-pipeline.md#221-frontier-and-scheduling), [§2.2.8](01-knowledge-pipeline.md#228-captures) |
| ingest assets, archives | Signed-URL minting, ranges, validators, safe archives | Kept | [01 §2.2.5–2.2.6](01-knowledge-pipeline.md#225-assets-and-catalogues) |
| ingest updates | Watermarks after commit; withdrawals; repairs; stale derivatives invalidated | Kept | [01 §2.2.1](01-knowledge-pipeline.md#221-frontier-and-scheduling), [§10](01-knowledge-pipeline.md#10-lifecycle) |
| ingest receipts | Unique run receipts with counts and coverage | Kept | [01 §2.2.1](01-knowledge-pipeline.md#221-frontier-and-scheduling) |
| ingest audit defects | Twelve defects of the current route not reproduced | Kept | [01 §2.2.9](01-knowledge-pipeline.md#229-defects-of-the-current-route-that-the-native-engine-must-not-reproduce) |
| ingest spike, rag.N046 | Spider as the engine behind our frontier; smaller HTTP/CDP driver if its hooks fall short | Kept | [01 §2.2.2](01-knowledge-pipeline.md#222-transports) |
| rag.N046 filter | No query-specific filtering at ingestion | Kept | [01 §3](01-knowledge-pipeline.md#3-l2-extraction-and-normalization) |
| product.CD2 | Synchronization policy per source | Kept | [01 §1](01-knowledge-pipeline.md#1-collections-sources-and-scopes) |
| product.KIS | Internal wikis through their API | Kept | [01 §3](01-knowledge-pipeline.md#3-l2-extraction-and-normalization) |

### 11.2 Extraction and quality

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| rag.N046 extractors | Xberg and docling.rs as alternatives behind one interface; Python Docling as oracle; pdf-inspector optional; escalation on structure | Kept | [01 §3](01-knowledge-pipeline.md#3-l2-extraction-and-normalization) |
| rag.N046 contract | Extractor contract fields; one qualified path per media type; offline model assets | Kept | [01 §3](01-knowledge-pipeline.md#3-l2-extraction-and-normalization) |
| rag.N046 small tools | htmd, dom_smoothie (careful), pulldown-cmark, calamine, Tree-sitter; no format conversion to PDF | Kept | [01 §3](01-knowledge-pipeline.md#3-l2-extraction-and-normalization) |
| rag.N011 normalization | unicode-normalization; ammonia only for display; never strip accents, case or symbols globally; injected instructions stay data | Kept | [01 §3](01-knowledge-pipeline.md#3-l2-extraction-and-normalization) |
| rag.N011 mining | Deterministic metadata first; model extraction only where needed; every item traced to a passage | Kept | [02 §8.3](02-retrieval-and-knowledge-graph.md#83-construction-pipeline) |
| rag.N052 quality | Accept, accept with warnings, re-extract, quarantine; format-aware; no model rewriting | Kept | [01 §4](01-knowledge-pipeline.md#4-corpus-quality-gate) |
| product.CD2 multimedia | OCR, transcription, visual interpretation as derived records | Kept | [01 §3](01-knowledge-pipeline.md#3-l2-extraction-and-normalization) |

### 11.3 Canonicalization, deduplication and chunking

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| rag.N054–N056 | Canonical document contract and completion test | Kept (implemented by the crate) | [01 §5](01-knowledge-pipeline.md#5-l3-canonicalization) |
| rag.N060 A01–A34 | Canonicalization acceptance gates (identity, structure, spans, safety, persistence) | Kept; re-verified on the migrated crate in S0 | [06 S0](06-roadmap.md#s0-foundation) |
| rag.N038 dedup | Exact binary, exact canonical, prepared-input fingerprints; near-duplicate grouping with exact confirmation; never delete | Kept | [01 §6](01-knowledge-pipeline.md#6-l4-deduplication) |
| rag.N060 near-dup | Near-duplicate detection optional and off | Adapted (§15 A22) | [01 §6](01-knowledge-pipeline.md#6-l4-deduplication) |
| rag.N060 B01–B13 | Chunking gates, chunk record contract, manifests, restarts | Kept | [01 §7](01-knowledge-pipeline.md#7-l5-chunking) |
| rag.N052 chunk policy | Prose, short sections, lists, code, tables; parents as references; deterministic prefixes | Kept | [01 §7](01-knowledge-pipeline.md#7-l5-chunking) |
| rag.N011 sizes | Child 512, parent 1,500, overlap 0–15 % | Adapted (§15 A9) | [01 §7](01-knowledge-pipeline.md#7-l5-chunking) |
| rag.N064 tokenizer | Match the GGUF encoder; ordered-ID parity; contract ID in every manifest | Adapted (§15 A6) | ADR-0008, [01 §7](01-knowledge-pipeline.md#7-l5-chunking) |
| rag.N067 contracts | Embedding tokenizer contract and BM25 analyzer contract kept separate | Kept | [01 §8](01-knowledge-pipeline.md#8-l6-representations) |
| rag.N011 techniques | Contextual retrieval, late chunking, late interaction tested one at a time | Kept | [01 §7–§8](01-knowledge-pipeline.md#7-l5-chunking) |

### 11.4 Representations, indexing and publication

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| rag.N070 EmbeddingProfile | Complete profile; validation of vectors; batch versus single; cache key | Kept | [01 §8](01-knowledge-pipeline.md#8-l6-representations) |
| rag.N070 Bm25Profile | Versioned lexical contract; French/English policy; identifiers in payload; empty-term policy | Kept | [01 §8](01-knowledge-pipeline.md#8-l6-representations) |
| rag.N070 index | Staging generation; named vectors; payload and indexes; deterministic point IDs; unresolved permission never public | Kept | [01 §9](01-knowledge-pipeline.md#9-l7-indexing-and-publication) |
| rag.N070 publication | Idempotent writes, persistent progress, pre-publication checks beyond counts, one pointer, rollback | Kept | [01 §9](01-knowledge-pipeline.md#9-l7-indexing-and-publication) |
| rag.N070 diagnostics | Independent dense and BM25 search before fusion | Kept | [02 §10](02-retrieval-and-knowledge-graph.md#10-evaluation) |
| rag.N038 statistics | IDF statistics isolated per generation; a filter is not a snapshot; Qdrant 1.19 corpus filter | Kept | [01 §9](01-knowledge-pipeline.md#9-l7-indexing-and-publication) |
| rag.N011, N016 automation | Durable state machine, outbox, change-driven jobs, deletion propagation, separate budgets, interactive priority | Kept | [01 §10](01-knowledge-pipeline.md#10-lifecycle) |
| rag.N052 recompute | Dependency-driven recomputation table | Kept | [01 §10](01-knowledge-pipeline.md#10-lifecycle) |
| rag.N075 first deliverable | Admit, prepare with the qualified counter, persist, record, survive interruption, expose after validation | Kept | [06 S1](06-roadmap.md#s1-knowledge-kernel--hybrid-rag--m1) |

### 11.5 Retrieval, fusion, reranking, evidence and answers

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| rag.N038 context | Trusted request context; routes by question type; counts through a structured query | Kept | [02 §1](02-retrieval-and-knowledge-graph.md#1-admission-and-scope), [§3](02-retrieval-and-knowledge-graph.md#3-retrieval-routes) |
| rag.N038 identity dedup | Candidate identity rules; per-route dedup before ranking; oversampling | Kept | [02 §4](02-retrieval-and-knowledge-graph.md#4-fusion) |
| rag.N038 RRF | One-based ranks, K = 60, one fusion, stable ties, Qdrant convention mapping; DBSF and learned ranking later | Kept | [02 §4](02-retrieval-and-knowledge-graph.md#4-fusion) |
| rag.N038 budgets | 100 dense, 100 BM25, up to 50 graph items, 80–120 reranked, 8–12 final | Kept | [02 §3–§6](02-retrieval-and-knowledge-graph.md#3-retrieval-routes) |
| rag.N038 rerank | Cross-encoder; mapping by returned index; reranker tokenizer; windows instead of truncation; scores never summed or read as confidence | Kept | [02 §5](02-retrieval-and-knowledge-graph.md#5-reranking) |
| rag.N038 context dedup | Mirrors once, span unions, redundancy penalty, support groups with reserved budget | Kept | [02 §6](02-retrieval-and-knowledge-graph.md#6-evidence-assembly) |
| rag.N038 EvidenceBundle | Passages, claims, paths, contradictions, known gaps, trace; answer-support plan | Kept | [02 §6](02-retrieval-and-knowledge-graph.md#6-evidence-assembly) |
| rag.N038 answer | Structured output; citations from stored evidence; validation before delivery; buffered verified answers | Kept | [02 §7](02-retrieval-and-knowledge-graph.md#7-grounded-generation-ask) |
| rag.N038 degradation | Degrade by question; caches keyed by publication and permission context | Kept | [02 §1](02-retrieval-and-knowledge-graph.md#1-admission-and-scope), [§3](02-retrieval-and-knowledge-graph.md#3-retrieval-routes) |
| rag.N016 interfaces | Rust functions, local HTTP API, MCP, integrated agent over one core | Kept | [02 §9](02-retrieval-and-knowledge-graph.md#9-mcp-tools-knowledge), [07 §2](07-extensibility.md#2-entry-points) |
| rag.N016 tools | knowledge search, read, entity resolve, graph expand, evidence trace, answer, research; resources by URI; no generic query tool | Kept | [02 §9](02-retrieval-and-knowledge-graph.md#9-mcp-tools-knowledge) |
| rag.N016 agent | Optional bounded research loop; limits; no recursion; same-model critique is not validation | Kept | [02 §9](02-retrieval-and-knowledge-graph.md#9-mcp-tools-knowledge) |
| rag.N016 local | The whole chain local in the strict profile | Kept | [02 §9](02-retrieval-and-knowledge-graph.md#9-mcp-tools-knowledge), [05 §1](05-platform-and-operations.md#1-reference-environment) |
| rag.N016 permissions | Checked at search, traversal, passage read, output; caches and communities; revocation on old publications | Kept | [02 §1](02-retrieval-and-knowledge-graph.md#1-admission-and-scope) |
| rag.N011 evaluation | 200–500 questions; ladder; diagnosis by failure type | Kept | [02 §10](02-retrieval-and-knowledge-graph.md#10-evaluation) |
| product.CD3 | Default scope current project plus explicit shares | Kept | [02 §1](02-retrieval-and-knowledge-graph.md#1-admission-and-scope) |

### 11.6 Graph

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| rag.N016 claims | Qualified claims; three logical graphs; world and record time; contradictions visible | Kept | [02 §8.2](02-retrieval-and-knowledge-graph.md#82-graph-model) |
| rag.N016 resolution | Reliable identifiers first; reversible merges; answers never evidence | Kept | [02 §8.2–8.3](02-retrieval-and-knowledge-graph.md#83-construction-pipeline) |
| rag.N052 layers | Structural graph first, extracted knowledge later; claims anchored to spans; coverage visible | Kept | [02 §8.2–8.3](02-retrieval-and-knowledge-graph.md#82-graph-model) |
| rag.N023 roles | Qdrant for passages, Neo4j for relations, Rust for fusion; graph results mapped to passages before fusion | Kept | [02 §8.4–8.5](02-retrieval-and-knowledge-graph.md#84-authority-and-projection) |
| rag.N023 access | neo4rs or the official HTTP Query API behind a graph interface; application IDs | Kept | [02 §8.4](02-retrieval-and-knowledge-graph.md#84-authority-and-projection) |
| rag.N023 variants | Compare Qdrant only, Neo4j only and the pairing | Kept | [02 §8.4](02-retrieval-and-knowledge-graph.md#84-authority-and-projection) |
| rag.N023 editions | Community single instance, GPL, GDS limits; no global analytics interactively | Kept | [02 §8.4](02-retrieval-and-knowledge-graph.md#84-authority-and-projection) |
| rag.N016 methods | GraphRAG global and DRIFT, HippoRAG 2, LightRAG, LazyGraphRAG, HyperGraphRAG, in order and on evidence | Kept | [02 §8.5](02-retrieval-and-knowledge-graph.md#85-graph-retrieval-route-r4) |
| rag.N052 publication | Graph and indexes published together; claims in later publications | Kept | [01 §9](01-knowledge-pipeline.md#9-l7-indexing-and-publication), [02 §8.4](02-retrieval-and-knowledge-graph.md#84-authority-and-projection) |
| rag.N016, N052 authority | Neo4j as the authority for jobs, metadata and claims | Adapted (§15 A1) | ADR-0002 |

## 12. Delivery mapping

| Earlier task | Content | Slice now |
| --- | --- | --- |
| delivery.U01 (F0–F6) | Effective core controls, strict lints, 90 % coverage, file splits, fresh checks | S0 |
| delivery.U02 | Catalog consistency: composed overlays, eligibility, owners, translation rules | S3; translation rules in the comparison pass |
| delivery.U03 (F7–F8) | Portable native qualification profile and host binding | S0 (binding), S1 (parity) |
| delivery.U04 | Two provider routes, SDK and CLI pair, MSRV | S0 (MSRV rule), S4 (qualification) |
| delivery.U05 | Catalog consumption contract, compiler, closure, overrides | S3 |
| delivery.U06 | Trusted operations, containment, durable acceptance, audit | S4 |
| delivery.U07 | SDK sessions, hooks, defaults, MCP boundaries | S4 |
| delivery.U08 | First end-to-end governed workflow | S4 (M4) |
| delivery.U09 | Deterministic bootstrap and native projection | S3 |
| delivery.U10 (Q1–Q3, D1) | Adapter qualification and executable policy | S6 (offline part can start after S1) |
| delivery.U11 (D2–D3) | Durable ingestion and the first knowledge CLI | S1 (local prepared-document operation), S6 (frontier) |
| delivery.U12 (Q4, D4–D5) | All source slices, protected preflight, per-source cutover | S6 |
| delivery.U13 | Workflow discovery and Qdrant projection; embedding qualification | S1 (Qdrant adapter, embedding bake-off), S3 (catalog routing) |
| delivery.U14 | Branch-scoped knowledge retrieval; reusable memory gate | S1 (retrieval), S7-I1 (memory) |
| delivery.U15 | Privacy-safe telemetry at every stage | S1 onward, completed in S4–S5 |
| delivery.U16 | Qualification and benchmarks on a protected corpus | S1 (bake-off round 1), S4 (runner, provider cards), ongoing |
| delivery.U17 | InnerSource releases, trust, laptop lifecycle | S0 (release pipeline), S3 (bundle trust), S5 |
| delivery.U18 | Monitoring, PO and orchestration-planning capabilities; a non-platform contribution | S5 |

Order change: the owner chose the knowledge kernel and Control-M RAG as the
first slice; the earlier plan's first governed workflow (U08) moves to S4.

| Earlier disposition | Where now |
| --- | --- |
| delivery.C01 transparent platform, minimal plumbing | [03 §1.7](03-agent-orchestration.md#17-project-bootstrap-maestro-init), [06](06-roadmap.md) |
| delivery.C02 detached two-repository releases | ADR-0012, [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install) |
| delivery.C03 Rust-first, explicit native dependencies, justified Python | [04 §1](04-intelligence-backend.md#1-scope-and-stance), [05 §6.2](05-platform-and-operations.md#62-supply-chain) |
| delivery.C04 readiness, owners, dependencies without invalid frontmatter | [03 §1.2](03-agent-orchestration.md#12-formats-copilot-native-first) |
| delivery.C05 InnerSource and domain capabilities | [03 §10](03-agent-orchestration.md#10-innersource-flow-s5), [06 S5](06-roadmap.md#s5-capabilities--innersource--m5) |
| delivery.C06 closure, merge, explanation, one override class | [03 §1.6](03-agent-orchestration.md#16-configuration-and-overrides) |
| delivery.C07 persona, models, reasoning, sampling, no escape hatch | [03 §3.1](03-agent-orchestration.md#31-copilot-sdk-github-copilot-sdk-1014), [05 §3](05-platform-and-operations.md#3-model-selection) |
| delivery.C08 explicit instructions and skills with provenance | [03 §1.2](03-agent-orchestration.md#12-formats-copilot-native-first), [§3.1](03-agent-orchestration.md#31-copilot-sdk-github-copilot-sdk-1014) |
| delivery.C09 small development workflow, deterministic build, separate reviews | [03 §2.2](03-agent-orchestration.md#22-example), [§2.6](03-agent-orchestration.md#26-roles) |
| delivery.C10 typed broker, trusted facts, host acceptance | [03 §4](03-agent-orchestration.md#4-the-policy-broker-cedar), [§6](03-agent-orchestration.md#6-handoff-contracts-and-acceptance) |
| delivery.C11 containment, durable state, reconciliation | [03 §2.4](03-agent-orchestration.md#24-the-engine-durable-event-sourced-execution), [§5](03-agent-orchestration.md#5-sandbox) |
| delivery.C12 preview/apply bootstrap | [03 §1.7](03-agent-orchestration.md#17-project-bootstrap-maestro-init) |
| delivery.C13 native projection | [03 §1.5](03-agent-orchestration.md#15-native-projection-convenience-mode) |
| delivery.C14–C16 catalog MCP, shared Qdrant, separate knowledge ACLs | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow), [02 §1](02-retrieval-and-knowledge-graph.md#1-admission-and-scope) |
| delivery.C17 signed releases, freshness, revocation, transparency | ADR-0015, [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install) |
| delivery.C18–C21 layered tests, runner, real controls, honest tiers | [03 §9](03-agent-orchestration.md#9-test-layers) |
| delivery.C22–C24 telemetry streams, coverage, units | [05 §4](05-platform-and-operations.md#4-observability) |
| delivery.C25–C29 provider qualification, benchmarks, corpus, taxonomy, promotion loop | [05 §3–§5](05-platform-and-operations.md#3-model-selection) |
| delivery.C30 unavailable attachments | §18 |
| delivery.R01–R11 corrections | R01 [06 S5](06-roadmap.md#s5-capabilities--innersource--m5); R02 [03 §4](03-agent-orchestration.md#4-the-policy-broker-cedar); R03 [03 §9](03-agent-orchestration.md#9-test-layers); R04 [03 §3.3](03-agent-orchestration.md#33-hook-mapping); R05 [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow); R06 [04 §4](04-intelligence-backend.md#4-phase-i1--memory-and-continuity); R07 [05 §4](05-platform-and-operations.md#4-observability); R08 [03 §2.6](03-agent-orchestration.md#26-roles); R09 ADR-0015; R10 [07](07-extensibility.md); R11 [05 §3.3](05-platform-and-operations.md#33-protocol) |

## 13. Intelligence backend

### 13.1 Product requirements

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| product.C01–C16 | Capability families | Kept, mapped family by family | [04 §2](04-intelligence-backend.md#2-the-provider-analysis-distilled) |
| product.M01–M11 | Memory and continuity requirements | Kept | [04 §4](04-intelligence-backend.md#4-phase-i1--memory-and-continuity) |
| product.U01–U13 | Governed knowledge additions | Kept | [04 §2](04-intelligence-backend.md#2-the-provider-analysis-distilled), [§6](04-intelligence-backend.md#6-phase-i3--governed-semantic-and-temporal-knowledge) |
| product.U07–U08 | Structured-data mappings and read-only SQL querying | Deferred: optional profile on demand | [04 §2](04-intelligence-backend.md#2-the-provider-analysis-distilled), [06 §3](06-roadmap.md#3-explicitly-deferred) |
| product.U14–U15 | Human review workflows, decision replay, scenario overlays; SSO, HA, replication | Kept (review, replay) and Deferred (team profiles) | [04 §2](04-intelligence-backend.md#2-the-provider-analysis-distilled) |
| product.J01–J08 | End-to-end journeys | Kept: J01–J02 I1, J03–J04 I2, J05 S6 and I2, J06 I3, J07 optional, J08 operations | [04 §4–§9](04-intelligence-backend.md#4-phase-i1--memory-and-continuity) |
| product.A01–A20 | Functional acceptance scenarios | Kept: each S7 phase spec imports its scenarios (A01–A06, A15, A18 for I1; A07–A11 for I2 and S6; A12–A14, A16 for I3; A17 optional; A19–A20 operations) | [06 S7](06-roadmap.md#s7-intelligence-backend--m7) |
| product.N01–N10 | Non-functional requirements and proposed targets | Kept | [04 §10](04-intelligence-backend.md#10-quality-targets-and-gates) |
| product.§12 gates | Hard, quality and usefulness gates; evaluation sets; actual baselines B01–B07 | Kept | [04 §10](04-intelligence-backend.md#10-quality-targets-and-gates) |
| product.§9 | Visible lifecycle, recovery table, retries, deployment profiles | Kept | [04 §9](04-intelligence-backend.md#9-operating-model) |
| product.§10 | Migration steps (inventory, snapshot, scope mapping, staged import, non-equivalence, reconcile, shadow read, cutover, retention) | Kept | [04 §11](04-intelligence-backend.md#11-transition-from-existing-providers) |
| product.D01–D11 | Open product decisions | Kept with current status | [04 §12](04-intelligence-backend.md#12-decisions-still-open) |
| product.V01–V12 | Visual workbench outcomes | Kept | [04 §8](04-intelligence-backend.md#8-phase-i4--workbench) |
| product.surfaces | 1,432 surfaces, overlays (672 include, 518 open, 182 conditional, 57 defer, 3 exclude), 489 undecided context surfaces | Kept | [04 §2](04-intelligence-backend.md#2-the-provider-analysis-distilled), [§7](04-intelligence-backend.md#7-context-management-the-undecided-489-surfaces) |
| product.scopes | 14 code families and 15 relationship modes; 17 memory families; ingestion, operations and context crosswalks | Kept as the I1–I3 requirement catalogue, re-read when each phase spec starts | [04 §4–§7](04-intelligence-backend.md#4-phase-i1--memory-and-continuity) |

### 13.2 Owner decisions from the provider analysis

| ID | Decision | Status | Where |
| --- | --- | --- | --- |
| product.CD1 | Full code-analysis block with SCIP and LSP; propose and apply structural transformations after approval | Kept | [04 §5](04-intelligence-backend.md#5-phase-i2--code-intelligence) |
| product.CD2 | Per-source synchronization; multimedia interpretation retained | Kept | [01 §1](01-knowledge-pipeline.md#1-collections-sources-and-scopes), [§3](01-knowledge-pipeline.md#3-l2-extraction-and-normalization) |
| product.CD3 | Retrieval scope: project plus explicit shares | Kept | [02 §1](02-retrieval-and-knowledge-graph.md#1-admission-and-scope) |
| product.CD4 | Admission by people or approved rules; marked hypotheses | Kept | [04 §6](04-intelligence-backend.md#6-phase-i3--governed-semantic-and-temporal-knowledge) |
| product.CD5 | No automatic expiry of originals | Kept | [04 §4](04-intelligence-backend.md#4-phase-i1--memory-and-continuity) |
| product.CD6 | Adaptive L1 budget under a ceiling; L0 never truncated; optional model-traffic relay | Kept | [04 §4](04-intelligence-backend.md#4-phase-i1--memory-and-continuity), [§7](04-intelligence-backend.md#7-context-management-the-undecided-489-surfaces) |
| product.CD7 | Views keep manual adjustments; frozen snapshots | Kept | [04 §8](04-intelligence-backend.md#8-phase-i4--workbench) |
| product.CD8 | Graceful shutdown by default, real stop-all, forced termination | Kept | [04 §9](04-intelligence-backend.md#9-operating-model), [07 §4.3](07-extensibility.md#43-the-extension-host) |
| product.GD1–GD5 | Provider configuration, four initial clients, non-blocking graph advice, hook administration, local access | Kept: four-client MCP and static hook maps in S3; all live hooks, including Copilot, in S4 (owner v4 amendment, 2026-09-30; §9, A28) | §6, §9, [04 §7](04-intelligence-backend.md#7-context-management-the-undecided-489-surfaces) |
| product.NP | Native Rust desktop; Rust wherever feasible; justified Python exceptions; no framework reuse; one backend; three platforms | Kept | ADR-0016, [04 §1](04-intelligence-backend.md#1-scope-and-stance), [§8](04-intelligence-backend.md#8-phase-i4--workbench) |

## 14. Foundation and engineering

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| core ownership | `maestro-core` is the complete production home; no separate runtime repository | Kept | README §7 |
| core no stubs | No empty command handlers, interface crates or stub supervisor | Kept | FND-002 and P-001 of the golden rules, README §8 |
| core dependency rule | Adapters → application → processing → I/O; no transport types in processing; no cycles | Kept | README §8 |
| core abstractions | New crates, traits and registries only for a real variation | Kept | P-004 and P-005 of the golden rules |
| core baseline | Golden controls, 24 tools, 90 % coverage, size limits, hooks, security, release evidence, principles and mandates | Kept | [05 §9](05-platform-and-operations.md#9-engineering-baseline), [06 S0](06-roadmap.md#s0-foundation) |
| core Sonar | Sonar as a scoped capability | Dropped (§16) | — |
| core migration | 43 reviewed candidates, fixtures byte-preserved, a second synthetic topic | Adapted (§15 A25) | ADR-0001 |
| core native | Qualification profile separate from host binding; explicit native tests | Kept | [06 S0](06-roadmap.md#s0-foundation) |
| core storage | Directory-handle storage, no-follow checks, synchronized no-overwrite publication, completion manifest last | Kept | [04 §3](04-intelligence-backend.md#3-the-kernel-building-blocks) (B3), [01 §9](01-knowledge-pipeline.md#9-l7-indexing-and-publication) |
| core scale | Bounded selections, backpressure, coverage receipts; accepted, started, completed and cancelled distinct; no second scheduler | Kept | [01 §2.2.1](01-knowledge-pipeline.md#221-frontier-and-scheduling), [§12](01-knowledge-pipeline.md#12-commands) |
| core verification | Relocated disposable checkout; independent review; historical evidence is not fresh acceptance | Kept | [06 S0](06-roadmap.md#s0-foundation) |
| delivery.§1.5 | Known defects (lint inheritance, zero-SHA CI pins, bootstrap JSON, workflow activation, translation rules, knowledge directory, MSRV, stale counts) | Kept: core defects are S0 work items; inert copied workflows and strict JSON in templates are rules S3 writes to; defects in earlier catalog content go to the comparison pass | [06 S0](06-roadmap.md#s0-foundation), [S3](06-roadmap.md#s3-catalog--m3) |

## 15. Adapted decisions

| # | Earlier choice | Now | Reason |
| --- | --- | --- | --- |
| A1 | Neo4j holds jobs, publication metadata and claims (rag.N038, N052) | The kernel (SQLite + artifacts) is the only authority; Neo4j is a rebuildable projection (ADR-0002) | One authority; the later unified analysis (rag.N075) chose SQLite for lifecycle state itself |
| A2 | SurrealDB as the graph store (rag.N016) | Neo4j (ADR-0004) | The owner asked for Qdrant + Neo4j (rag.N020); SurrealDB's licence and storage caveats |
| A3 | redb or apalis as the job registry (chat.M023, rag.N011) | Kernel jobs on SQLite | One store; SQLite selected by the unified plan |
| A4 | Qwen3-Embedding-0.6B, gte reranker and Qwen3.8-27B as starting models (rag.N011, N030, N038) | Candidates in a recorded bake-off (ADR-0011) | Owner decision: no preselected model |
| A5 | text-splitter for chunking (rag) | The existing canonicalization chunker | Already built, tested and span-mapped |
| A6 | Count with the GGUF encoder's own tokenizer (rag.N064) | The router's `/tokenize` for the selected embedder, with the native counter as the parity reference (ADR-0008) | Same vocabulary, no machine paths, no process per count |
| A7 | TEI for embeddings and reranking (rag) | The llama.cpp router first; TEI a justified alternative | One local serving path already running |
| A8 | mistral.rs for generation (rag) | The router first; mistral.rs an alternative | Same |
| A9 | Child 512 / parent 1,500 tokens, 0–15 % overlap (rag.N011) | Target 500, maximum 700, zero primary overlap; parents are section objects | The canonicalization contract (rag.N060) supersedes the first study |
| A10 | YAML platform and capability manifests, `org-agent` names (chat) | Copilot-native Markdown, TOML sidecars, JSON policies, `maestro` noun-verb commands | ADR-0005, ADR-0014 |
| A11 | Empty agent metadata with a separate descriptor (chat.M019) | Agent sidecars confirmed; skills use specification-backed `metadata`, not support inferred from a silent unknown-key control; a host warning reopens ADR-0005's sidecar decision | ADR-0005 retained; integrated [C01 report](../../specs/003-catalog/research/hosts.md) at `0be954b` |
| A12 | Ordered workflow steps (chat) | Workflow graphs with bounded loops and joins (ADR-0006) | Expresses reviews, repairs and fan-out |
| A13 | Rig as the agent abstraction (rag.N016) | The in-house engine and sessions | Semantics are host-specific |
| A14 | Tantivy for lexical search (rag.N011) | Qdrant sparse vectors from maestro's `bm25-en-fr/1` analyzer (R7); Tantivy only if tests show gaps | One engine |
| A15 | Crawl4AI in the production path | Comparison oracle until per-source cutover | Rust-first; parity gates |
| A16 | kreuzberg 4.10 as the document extractor (first draft of this design) | Xberg or docling.rs per media type by bake-off | rag.N046 |
| A17 | dom_smoothie readability by default (first draft) | htmd first; readability only if it helps | delivery.U10 (Q1) |
| A18 | A fixed monthly refresh | Per-source synchronization policy with schedules | product.CD2 |
| A19 | Workbench confirmed against a web front end at I4 (first draft) | Native Rust desktop confirmed (ADR-0016) | Owner-confirmed direction |
| A20 | A list of test libraries to install | Tokio, nextest and insta first; others only per real seam | delivery.C19 |
| A21 | Five repetitions per benchmark task | A pilot value, not a constant | delivery §4.3 |
| A22 | Near-duplicate detection off by default (rag.N060) | Built in S1 as non-destructive grouping with exact confirmation | Needed for version collapse at retrieval |
| A23 | A `maestro-contracts` crate published for the catalog compiler (chat.M027) | Manifests CI runs the released, pinned `maestro` binary as the compiler | One implementation, no source dependency |
| A24 | A separately deployable catalog MCP service (chat.M031) | The same `maestro` MCP server; a service deployment waits for a team profile | Laptop first |
| A25 | Recopy 43 reviewed migration candidates (core) | The crate stays as is with its fixtures; S0 only brings it to the gates | Already present; delivery.§7.2 |
| A26 | First governed workflow first (delivery.U08) | Knowledge kernel and Control-M RAG first (S1) | Owner decision |
| A27 | Unconditional hybrid win for M3 (chat.M031 hybrid) | Passing baseline ships unless hybrid's seeded 95 % paired-bootstrap top-1 gain interval is strictly positive; held-out matchable top-1 ≥ 90 % is the absolute gate | OA10 approved by the owner, 2026-09-28: dated amendment to D5. The original top-3 ≥ 90 % bar is retained as history, not current acceptance (§21) |
| A28 | Four-client hook administration in S3 (product.GD2, GD4, GD5) | Four-client MCP and static hook maps stay S3; C20 live Copilot plus other hooks are S4, retaining C20's 4 h | Owner v4 amendment 2026-09-30 supersedes the 2026-09-28 Copilot-only exception. Trusted event/identity adapters need S4 qualification; no reduction of MCP scope |

## 16. Dropped

| Item | Reason | Decided by |
| --- | --- | --- |
| Sonar code-quality integration | The organization removed Sonar and every mirror or tier notion | Owner |
| Artifactory or JFrog registry configuration | Same | Owner |
| SurrealDB, HelixDB, CozoDB, Oxigraph as graph stores | Neo4j chosen; RDF and Datalog not required; licences and maturity caveats | Owner (rag.N020) |
| Graphiti's Python runtime | Principles kept (bitemporal facts), runtime not reused | NP rule |
| Embedded provider engines (Headroom, Context Mode, OpenViking, Archify renderer) | Outcomes reimplemented; no framework code reuse | Owner (NP) |
| The three frozen provider exclusions (no-op, demonstration, idle launcher) | Nothing to deliver | Provider review overlays |
| Graphify's `hook-check` success claim | A documented no-op | Owner (GD4) |
| Jina v5 and SPLADE v3 as defaults | Non-commercial licences | Licence rule |
| Native dynamic-library plugins | Unsafe ABI, core crashes | ADR-0013 |
| Retired migration rituals (approval receipts, maintained-file inventories, migration journals) | Superseded by the unified plan's start instructions | delivery |
| A third runtime repository | Two repositories plus private collection | chat.M027, delivery |
| "Remove 100 % of cognitive load" as a literal guarantee | Replaced by "no framework plumbing" (§3) | chat.M006 interpretation, kept by the owner |

## 17. Open decisions

| Decision | Needed by | Proposed default |
| --- | --- | --- |
| Where `ctm-collection` lives: the organization allows only public repositories (`visibility-is-frozen`, CodeQL required on every default branch), so ADR-0009's private repository cannot be created in it | S0 | A private repository on the maintainer's account; the organization's settings unchanged |
| The SDK and CLI pair to pin | S4 | The latest release at S4 start, qualified by U04's suite |
| Operational bindings: endpoints, accounts, data scopes, exclusion registries, budgets | S1 (import), S6 (live) | Supplied by the owner per source |
| Publisher identities, trust roots, key rotation procedure | S3 C09 records OA4 bindings/evidence; missing evidence blocks C28 | D2 decided: pinned gh, separate catalog/runtime publishers, five-minute refresh and ≤24-hour offline validity. With real `cli/cli` v2.98.0 public bundles, `gh 2.98.0` offline verification accepts the deployment workflow signer and rejects a changed signer; fixture outputs/digests are in `tests/fixtures/catalog/trust/`. One unauthenticated REST list request returned both bundles. Online `gh` lookup still exits 4 without credentials, so OA4 must supply the checksum-pinned standalone gh and confirm the proposed fine-grained token (public repositories, read-only, no permissions; untested). Measure online lookup calls and least-scope binding against 60 REST requests/hour unauthenticated and 5,000/hour authenticated; the five-minute budget is `12 × requests per refresh × active laptops per root`. Exact OA4 identities, online call count and live rotation evidence block C28, not implementation; not an open D1–D5 product choice |
| Product decisions D01, D03–D05, D07–D08, D10–D11 | S7 phases | [04 §12](04-intelligence-backend.md#12-decisions-still-open) |
| egui/eframe as the workbench toolkit | I4 start | egui/eframe |
| Neo4j versus the LadybugDB spike on laptops | S2 exit | Neo4j unless the spike wins |
| Qdrant server versus Qdrant Edge on laptops | Before laptop rollout | Server (ADR-0003) |

## 18. Source limits

- The chat's downloadable attachments (the 70-control catalogue, the
  16-scenario review, the 57-instrument dictionary, example ZIPs) were never
  retrievable; their visible descriptions are traced, their exact contents are
  not claimed.
- Historical inventory hashes (golden baseline, migration and ingestion
  inventories) were not revalidated; they are references, not evidence.
- The earlier catalog lives outside the organization. It is not a source for
  S3; it is read once, in the comparison pass after M3 (owner.catalog).
- Nothing here was executed against a live model, endpoint, sandbox, browser or
  source; statuses describe the design, not delivered behaviour.
- The `revtools` source's licence statements about named projects were not
  established against their repositories; §19 records them as claims to
  re-establish, not as facts.

## 19. Reverse-engineering analysis

Supplied by the owner on 2026-09-24 with the instruction to fit it into the
stack as a later phase, using the knowledge core, connecting through the
extension points, and losing nothing. Every item of the source appears below.
The transcript stays with the archived design sources.

### 19.1 Architecture recommendations already held

The source recommends an internal architecture for a unified code-and-memory
product. Maestro decided each of these earlier and for its own reasons, so they
are **Kept, already in the design**; the value of the row is that an independent
analysis converges on it.

| ID | Recommendation | Where it already lives |
| --- | --- | --- |
| revtools.arch1 | Canonical immutable event store; agents submit events or proposals through one controlled write path, never rewriting canonical knowledge | B2 journal, [04 §3](04-intelligence-backend.md#3-the-kernel-building-blocks); [07 §1](07-extensibility.md#1-principles). S1 ships/tests the durable cursor primitive; built-in projection and telemetry consumers are S2 ([07 §7](07-extensibility.md#7-delivery-by-slice)) |
| revtools.arch2 | One graph model over code, memory, documents and operational state, with node and edge fields for source, validity, confidence, extraction method and security scope | [02 §8.2](02-retrieval-and-knowledge-graph.md#82-graph-model), B8 |
| revtools.arch2-types | Node types (person, project, conversation, message, document, file, class, function, variable, decision, task, requirement, claim, event, agent) and relations (mentions, calls, imports, implements, depends on, derived from, decided in, supersedes, contradicts, assigned to, related to) | [02 §8.2](02-retrieval-and-knowledge-graph.md#82-graph-model) for knowledge; [04 §5](04-intelligence-backend.md#5-phase-i2--code-intelligence) for code; §19.3 adds the analysis types |
| revtools.arch2-prov | Provenance mandatory: where a fact came from, when it was true, extracted or inferred | Invariant 2 of [README §6](README.md#6-invariants); B7 |
| revtools.arch3 | Several coordinated indexes rather than one vector database: lexical, vector, code graph, knowledge graph, temporal, permission | Routes R1–R6, [02 §3](02-retrieval-and-knowledge-graph.md#3-retrieval-routes) |
| revtools.arch3-planner | A retrieval planner: classify intent, gather candidates, expand the graph, filter by time and permission, detect conflicts, rerank, package with provenance | [02 §2](02-retrieval-and-knowledge-graph.md#2-query-understanding) through [§6](02-retrieval-and-knowledge-graph.md#6-evidence-assembly) |
| revtools.arch4 | Explicit conflict handling: a correction supersedes with validity bounds; obsolete facts stay recallable and are not treated as current | [04 §6](04-intelligence-backend.md#6-phase-i3--governed-semantic-and-temporal-knowledge), bitemporal facts |
| revtools.arch5 | Language-independent intermediate representation; one parser adapter per language emitting the same schema | I2, [04 §5](04-intelligence-backend.md#5-phase-i2--code-intelligence) |
| revtools.arch6 | Safety and operational policy: human confirmation for destructive work, read-only and write modes, per-agent capabilities, delegation depth, budgets, audit logs, review gates, recovery checkpoints before compaction | [03 §4](03-agent-orchestration.md#4-the-policy-broker-cedar), [§5](03-agent-orchestration.md#5-sandbox), [§6](03-agent-orchestration.md#6-handoff-contracts-and-acceptance), [04 §4](04-intelligence-backend.md#4-phase-i1--memory-and-continuity) |
| revtools.arch7 | Stable external interfaces: MCP for AI clients, HTTP or gRPC for applications, CLI for people, a plugin SDK; contracts language-neutral | [07 §2](07-extensibility.md#2-entry-points), ADR-0013 (gRPC not chosen; local HTTP with a Unix socket) |
| revtools.lang | Rust for parsing, watching, indexing, graph traversal and local storage; adapters at the edges; one language is not required everywhere | [04 §1](04-intelligence-backend.md#1-scope-and-stance) |

### 19.2 The method, which is new

| ID | Item | Status | Where |
| --- | --- | --- | --- |
| revtools.cleanroom | Porting is derivative and keeps the original licence; only an implementation written from documented observable behaviour, without the original source, expressions, names or distinctive structure, is separate | Kept, and made enforceable rather than procedural | [09 §3](09-reverse-engineering.md#3-the-clean-room-boundary), ADR-0019 |
| revtools.process | The nine-step defensible process: freeze repository, commit, licence and dependencies; keep a licence and SBOM register of reused, modified, ported and independent code; specify before porting; import permissive components deliberately with attribution; separate analysis from implementation so decompiled code never reaches tickets, prompts, tests or the repository; use our own names, structures, UI, documentation and branding; record provenance per file and algorithm; test against expected behaviour rather than copied internals; licence and IP review before distribution | Kept whole | [09 §11](09-reverse-engineering.md#11-the-process-end-to-end) (steps 1–11, expanded with the scope grant and the promotion gate) |
| revtools.register | A provenance register with component, origin, commit, licence, usage and notice requirement | Adapted: usage becomes five named classes, and obligations are recorded rather than a notice flag | [09 §4](09-reverse-engineering.md#4-the-provenance-register) |
| revtools.contracts | Behavioural tests built from authorized observation become the specification for the rewritten product | Kept, and promoted to a gate: a provider is replaced when the native capability passes the contract derived from the provider | [09 §7](09-reverse-engineering.md#7-behaviour-contracts), [04 §11](04-intelligence-backend.md#11-transition-from-existing-providers) |
| revtools.legal | Canada s. 30.61 and s. 41.12, United States 17 U.S.C. §1201(f), EU study and decompilation limits; concept safer than implementation; API and format reimplementation safer than internal code; line-by-line translation of decompiled code high risk; circumvention of activation, subscription, DRM or licence enforcement a separate problem; names, logos, artwork, documentation, messages, sample data, prompts, keys and user data all carry risk; copyright is not the only exposure | Kept as the recorded boundary, with the source's own caveat that it is not legal advice | [09 §10](09-reverse-engineering.md#10-legal-boundary) |
| revtools.risk | A product can clear copyright and still meet a patent, trademark, trade-secret, privacy or contract problem | Kept | [09 §10](09-reverse-engineering.md#10-legal-boundary), risk R14 of [06 §4](06-roadmap.md#4-risk-register) |

### 19.3 Instruments

All of them are analyzers behind one contract ([09 §8](09-reverse-engineering.md#8-analyzers-are-extensions)),
never runtime dependencies and never shipped.

| ID | Instrument | Status | Where |
| --- | --- | --- | --- |
| revtools.joern | Joern as the comparison platform: code property graphs joining syntax, calls, control flow and data flow, queryable across languages, with its x86 frontend using Ghidra | Kept as the R2 analyzer | [09 §5](09-reverse-engineering.md#5-static-analysis) |
| revtools.joern-q | The questions to ask it: entry points, public APIs, storage layers, retrieval pipelines, graph construction, ingestion, ranking, background jobs, MCP commands, agent hooks, security boundaries | Kept | [09 §5](09-reverse-engineering.md#5-static-analysis) |
| revtools.joern-export | Export normalized findings (project, module, class, function, endpoint, command, database table, graph node and edge types, external dependency) | Adapted: normalized to target, component, surface, capability, finding and obligation, reusing the surface vocabulary the provider inventory already counts | [09 §5](09-reverse-engineering.md#5-static-analysis) |
| revtools.codeql | CodeQL for precise data-flow questions across mixed-language repositories; its terms differ for public and private repositories | Kept, with the terms recorded per target | [09 §5](09-reverse-engineering.md#5-static-analysis) |
| revtools.treesitter | Tree-sitter as the foundation of the new indexer: concrete syntax trees, incremental updates, error tolerance, many bindings | Kept, and already chosen for I2 before this analysis | [04 §5](04-intelligence-backend.md#5-phase-i2--code-intelligence), [09 §5](09-reverse-engineering.md#5-static-analysis) |
| revtools.scancode | ScanCode Toolkit for licences, copyright, package metadata, dependencies and attribution material | Kept as the R1 analyzer proposing register records | [09 §4](09-reverse-engineering.md#4-the-provenance-register) |
| revtools.ghidra | Ghidra for compiled components: disassembly, decompilation, graphing, scripting, headless operation | Adapted: bounded to lawfully possessed native components, headless, inside the analysis scope, output never promoted | [09 §6](09-reverse-engineering.md#6-native-binaries) |
| revtools.ghidra-loss | What compilation destroys: names, comments, module boundaries, generic types, tests, build configuration, history, error structure, optimized-away code, server-side logic; and the design questions it cannot answer | Kept as the reason Ghidra is not the primary instrument | [09 §6](09-reverse-engineering.md#6-native-binaries) |
| revtools.frida | Frida for runtime tracing of authorized native applications: which function runs, with which arguments, returning what | Kept, R3 | [09 §7](09-reverse-engineering.md#7-behaviour-contracts) |
| revtools.mitmproxy | mitmproxy to record and replay authorized HTTP, HTTPS and WebSocket conversations into an independent API specification | Kept, R3, with capture hygiene | [09 §7](09-reverse-engineering.md#7-behaviour-contracts) |
| revtools.playwright | Playwright for end-to-end behavioural contracts across browsers | Kept, R3 | [09 §7](09-reverse-engineering.md#7-behaviour-contracts) |
| revtools.hygiene | Captured credentials, tokens, private user information and proprietary server responses are not reusable product material | Kept as a test | [09 §7](09-reverse-engineering.md#7-behaviour-contracts), [§13](09-reverse-engineering.md#13-tests) |
| revtools.stack | The eleven-step recommended setup (repositories and documentation, ScanCode, Joern, CodeQL, tree-sitter, behavioural benchmarks, Ghidra only for missing native components, Frida when runtime tracing is required, mitmproxy for authorized interoperability testing, Rust core, adapters) | Kept, sequenced as R1–R4 | [09 §12](09-reverse-engineering.md#12-delivery), [06 S8](06-roadmap.md#s8-provenance-and-reverse-engineering--m8) |

### 19.4 Targets and licences

The source's capability map names what each studied project contributes. Those
contributions are already the provider inventory of
[04 §2](04-intelligence-backend.md#2-the-provider-analysis-distilled), which
counted them surface by surface; the rows below record the correspondence and
the licence question each one still carries.

| ID | Project | Contribution as the source describes it | Licence claim | Status |
| --- | --- | --- | --- | --- |
| revtools.utopia | Utopia | Persistent operational state, human-confirmation gates, review before canonicalization, multi-agent handoffs, session recovery, safety controls | MIT | Claim to re-establish at R1; the outcomes are already inventoried (172 surfaces) |
| revtools.codegraph | CodeGraph | Incremental code indexing, symbol, call, import and dependency relationships, cross-file resolution, file watching, focused context retrieval | MIT | Claim to re-establish at R1; 33 surfaces inventoried |
| revtools.cgc | CodeGraphContext | Not named by this source; the inventory holds it | — | 132 surfaces inventoried |
| revtools.graphify | Graphify | Evidence-linked graph over code and documents, extracted versus inferred provenance, path queries, communities, exploration | Apache-2.0 with a `NOTICE` file | Claim to re-establish at R1; an attribution obligation that a public MIT repository must resolve before any reuse |
| revtools.mempalace | MemPalace | Verbatim memory, scoped semantic retrieval, pluggable storage backends, temporal entity relationships, MCP memory operations | MIT | Claim to re-establish at R1; 125 surfaces inventoried |
| revtools.semantica | Semantica | Not identifiable from the source | Unknown | Blocked until identified: an unresolved licence refuses the analysis ([09 §2](09-reverse-engineering.md#2-stance)); the inventory's 134 surfaces stand on their own |

revtools.licences: "a new language does not erase the original licence". MIT
requires the notice in copies and substantial portions; Apache-2.0 adds
attribution, modification notices and `NOTICE`. Kept, as the reason the register
records obligations rather than licence names alone
([09 §4](09-reverse-engineering.md#4-the-provenance-register),
[§10](09-reverse-engineering.md#10-legal-boundary)).

### 19.5 Dropped

| Item | Reason |
| --- | --- |
| Ghidra as the primary method for rebuilding five existing implementations | The source itself rejects it; source analysis keeps what compilation destroys |
| Merging the internal code of several projects into one codebase | Against "outcomes, not engines" ([04 §1](04-intelligence-backend.md#1-scope-and-stance)) and against the clean-room boundary |
| gRPC as an application interface | Local HTTP on a Unix socket plus MCP already cover it ([07 §2](07-extensibility.md#2-entry-points)); a remote profile is a separate decision |
| Python or TypeScript adapters for AI and agent integrations as a starting assumption | Rust wherever feasible; an exception must record its justification ([04 §1](04-intelligence-backend.md#1-scope-and-stance)) |

## 20. S1 delivery evidence

This section maps each S1 requirement and success criterion of the
[S1 spec](../../specs/001-knowledge-kernel/spec.md) to the commits and tests
that deliver it on the integration branch `feat/s1-integration`, at head
`6a4a3b8` (2026-09-28). **Delivered** means integrated with tests; **Partial**
names the delivered portion and the open remainder; **OPEN** names the owner
of the missing work; **Measured, below target, owner-accepted for M1** means a
measured value misses its preregistered floor and the owner accepted it for M1
as a tuning target after S1 (2026-09-28). Private receipts (corpus accounting,
golden set, model evaluations, ladder runs) live in the private collection
(ADR-0009) and are cited by name and counts only. §20.4 maps the
owner-approved exact rows of the other sections (T039 step 3).

Test paths are relative to the repository root.

### 20.1 Functional requirements

| ID | State | Commits | Tests and checks | Remainder and owner |
| --- | --- | --- | --- | --- |
| FR-S1-001 | Delivered | B3 `b771872`; database `9c4463e`; B10 `b738975`; B9, B11 `0c69714`; B2 `852c4e7`; B1 `8dcab4b`; B5, B6 `e24a02b`; B4 `6379c7a`; B7 `0495f5f`; spans `3af7d66` | `crates/maestro-kernel/src/{artifact,store,gateway,capability,telemetry,journal,scope,document,generation,job,evidence}/tests` | B8 facts: S2 |
| FR-S1-002 | Delivered | `9405140` contracts; `e634ff8` import; `7d1ff9a` bounded bookkeeping | `crates/maestro-knowledge/src/import/tests/streaming.rs`, `crates/maestro-knowledge/tests/it/import_contract/` | None |
| FR-S1-002a | Delivered | `02c806e`; CLI `87b2aeb` | `crates/maestro-knowledge/src/quality/tests/`, `crates/maestro-knowledge/tests/it/quality_gate/` | None |
| FR-S1-003 | Delivered | Seam `2f83cbc`; router counter `fed0d24`; prepare `73faa04` | `crates/maestro-knowledge/src/prepare/tests/{duplicates,near,parity,counting}.rs`, `crates/maestro-knowledge/tests/it/router_parity.rs` | None |
| FR-S1-004 | Delivered | Lexical `1c081c5`; projection `fd9414e`; publish `2144186`; identifiers `4613ec8` | `crates/maestro-knowledge/tests/it/qdrant_projection/`, `crates/maestro-knowledge/tests/it/lexical_golden.rs`, `.github/workflows/integration.yml` | None |
| FR-S1-005 | Delivered | Fusion `5e3769e`; routes `17bfa42`; query `db14f92`; fused search `6bc3ee5`; rerank `cb9b200`; evidence `b1bc8bb`, `887b612` | `crates/maestro-knowledge/src/search/tests/`, `crates/maestro-knowledge/src/search/evidence/tests/`, `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes/` | None; warm p95 in SC-S1-004 |
| FR-S1-005a | Delivered | `17bfa42` (`search_dense`, `search_bm25`); route configuration `46e1dec` | `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes/route_behavior.rs`, `crates/maestro-knowledge/src/search/tests/stages.rs`; private T039 single-route dense-only and lexical-only receipts on golden v2.2 (SC-S1-008) | None; dense and BM25 recall measured separately, without reranking |
| FR-S1-006 | Delivered | `8dcab4b`; route scope filters `17bfa42`, `6bc3ee5` | `crates/maestro-kernel/src/scope/tests/`, `crates/maestro-kernel/src/retrieval/tests/identifier_scope.rs`, `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes/scope_index.rs` | Identifier-count side channel: post-M1 queue |
| FR-S1-007 | Partial | Chat bounds `311eb85`; ask `f0cfc95`; pinned embedder `016be66` | `crates/maestro-knowledge/src/answer/tests/guardrails.rs`, `crates/maestro/tests/it/knowledge_ask.rs`, `crates/maestro/tests/it/knowledge_get/ask_republish.rs` | Delivered: citation markers resolved from stored evidence, verbatim command check with one regeneration, refusal with the closest passages, the route status of each search route in `ask --explain`. Queued as the post-M1 "T035 extras": the answer-language check with one regeneration and structured output bound by a JSON Schema (02 §7); flags on uncited sentences (02 §7); a default rerank threshold, which is off and skipped when the rerank did not run (02 §7); refusing on an unavailable reranker unless the caller's policy accepts degraded evidence, where `ask` answers from the fused order today (02 §5). False refusals seen live: OPEN, ask live fix lane |
| FR-S1-008 | Delivered | `750e7d6`, `2410855`, `56eae40`, `f0cfc95` | `crates/maestro/tests/it/mcp_stdio.rs`, `crates/maestro/tests/it/knowledge_get/` | None; four real clients in SC-S1-005 |
| FR-S1-008a | Delivered | `852c4e7`; scope ruling `2a2fddb` | `crates/maestro-kernel/src/journal/tests/{cursors,crash,concurrency}.rs` | Built-in cursor consumers: S2 (owner, MR-03) |
| FR-S1-008b | Delivered | `4826626`; predecessor check `580c044` | `crates/maestro-kernel/src/journal/tests/{schemas,predecessor}.rs`, `.github/workflows/event-schemas.yml` | None |
| FR-S1-009 | Delivered | `6a6df77`; grouping `c1ecb3b`; report v2 `bdf52ca`; ladder floors `90e8e56`; citation spans `0edc402` | `crates/maestro-knowledge/src/eval/tests/`, `crates/maestro-kernel/src/eval/tests/` | None |
| FR-S1-010 | Delivered | `0146bf9`; rebuild `1f6ca8b`; drill `8ee5db4`; portable paths `1e80c1f` | `crates/maestro/tests/it/backup_restore.rs`, `crates/maestro/tests/it/rebuild_drill/` | None |
| FR-S1-011 | Delivered | `6379c7a`; publication resume `2144186` | `crates/maestro-kernel/src/job/tests/{leases,resume}.rs`, `crates/maestro/src/cli/tests/publication_resume.rs` | None |
| FR-S1-012 | Delivered | `dc7d3c9`; setup `86b9580`; backup `0146bf9`; search `56eae40`; `maestro eval ladder` `80a7ad1` | `crates/maestro/tests/it/cli_contract.rs`, `crates/maestro/tests/it/job_waits.rs`, `crates/maestro/src/cli/eval/tests/` | None: for M1, `eval ladder` replaces `eval run` and `eval compare` |
| FR-S1-013 | Delivered | Wording `8465a42` | Private: golden set v2.1 and its review receipt | None |
| FR-S1-014 | Delivered | Registry `bf58adb`; qualification `bdf52ca`; card evidence `dd298cd`; v2 embedder cards in prepare `5520789`; answerer per rung `dac543c` | `crates/maestro-kernel/src/model/tests/`, `crates/maestro-knowledge/src/eval/tests/v2.rs`, `crates/maestro/src/cli/eval/tests/rung_answerer.rs` | None; the private receipts are the v2 embedder, reranker and answerer cards and the embedder comparison |
| FR-S1-015 | Partial | `86b9580`; Qdrant client check `ddb67e2`; temporaries `f1ed1a2` | `crates/maestro/tests/it/{setup_installs,doctor_checks}.rs`, `crates/maestro/src/cli/health/tests/` | Per-role card checks in `doctor`: post-M1 queue |
| FR-S1-015a | Delivered | Router free room (T002, `maestro-model-router`); rerank fallback `cb9b200` | `crates/maestro-knowledge/src/search/tests/{admission,rerank}.rs` | None |
| FR-S1-016 | Partial | Synthetic fixtures `a6a27e5`; synthetic-only CI `d30573d` | `.github/workflows/integration.yml`, `crates/maestro-conventions/tests/policies.rs` (paths and settings only) | Content check (MR-06): paused until after M1 by the owner |

### 20.2 Success criteria

| ID | State | Evidence | Remainder and owner |
| --- | --- | --- | --- |
| SC-S1-001 | Delivered | Import and quality accounting (`e634ff8`, `02c806e`). Private v22 import receipt (the collection's one `maestro.knowledge.import.completed.v1` event, 2026-09-27): 2,205 manifest entries, 2,159 imported, 46 held, 0 refused, 0 unchanged. Every entry is a recorded revision (2,205 revisions of 2,182 documents) with exactly one quality outcome. The 46 held are 23 `source_ref` values that two entries each gave different digests; `import` quarantines both revisions with `import.shared-source-ref`, so they are counted among the 50 quarantined. The 2,159 imported revisions: 2,133 accepted, 2 with warnings, 20 needs re-extraction, 4 quarantined (3 by the gate's secret check, 1 by the owner's ledger rule); 2,159 + 46 = 2,205 | None |
| SC-S1-002 | Partial | `maestro eval ladder` (`80a7ad1`, floors `90e8e56`) on published generation 3 (BGE-M3 embedder). On golden v2.1, private run 1 compares hybrid without reranking (`r1-dense`, four routes, RRF 60) with depth 30 reranking (`r3-rerank-30`): top-10 72 → 79 of 84, ranked first 44 → 68; the adopted configuration scores 81 and 69 in run 3. These are not dense-only scores; the private T039 single-route receipts on golden v2.2 are in SC-S1-008. The Qwen3-Embedding-4B generation did not beat BGE-M3 (the embedder comparison). On golden v2.1 (run 4), thinking answers 65 of 84 right against 63 without, and 12 wrong against 19. On golden v2.2, both answer 65 of 84 right (v2-base by rescoring); thinking's gain is citation precision, 65 of 78 against 65 of 83, not answers right (private T039 v22-thinking and golden-v22 score-effect receipts). Winners, live since `e665da7` and `6826de6`: BGE-M3 embedder, `bge-reranker-v2-m3` at depth 30, Qwen3-4B thinking answerer with prompt v2 | The ladder reports each rung's change as a count; the paired interval per shipped rung is not computed. Closed by the owner without the interval (2026-09-28, 08:12); the interval is queued after S1 |
| SC-S1-003 | Delivered | Guarded ask (`f0cfc95`, `0319633`), literal check (`c6cea31`). Private T039 v22-thinking receipt (golden v2.2, `dc96370`, 2026-09-28 13:05, live default): 0 invented command literals in delivered answers by the literal check (command exactness 100 %); unanswerable questions refused 15 of 16 (93.8 %, floor 80 % in the spec; the ladder applies 90 %, met); ask p95 7.09 s (floor 10 s). Run 4 v2-thinking on golden v2.1 has the same counts, ask p95 7.18 s. The ask rungs on `ctm-retrieval` stand in for `ctm-answers` | None for the M1 substitute; no separate reference-answer suite was run |
| SC-S1-003, answer floor | Measured, below target, owner-accepted for M1 | Private T039 v22-thinking on golden v2.2: answerable questions answered with a right-section citation 65 of 84 (77.4 %, floor 80 %); v2-base rescored on golden v2.2 also gives 65 of 84. Run 4 v2-thinking on golden v2.1: the same 65 of 84; run 3 v2-base on golden v2.1: 63 of 84 | Tuning target after S1 (owner, 2026-09-28; post-M1 queue) |
| SC-S1-003, citation floor | Measured, below target, owner-accepted for M1 | Private T039 v22-thinking on golden v2.2: right-section citations 65 of 78 answered (83.3 %, floor 90 %); v2-base rescored on golden v2.2: 65 of 83 (78.3 %). Run 4 v2-thinking on golden v2.1: the same 65 of 78; run 3 v2-base on golden v2.1: 63 of 83 | Tuning target after S1 (owner, 2026-09-28; post-M1 queue) |
| SC-S1-004 | Delivered | Stage spans (`3af7d66`); warm-ups and per-question timing in the ladder (`80a7ad1`). Private T039 search-only rung on golden set v2.2, binary `dc96370`, generation 3, rerank depth 30, the search models loaded and warmed by 5 questions: `knowledge_search`'s search and evidence assembly p95 940 ms (p50 618 ms, maximum 1,456 ms) over 100 questions; 0 observed load waits (the embedder and reranker processes stayed resident in the process watch); 0 ran without a route (100 of 100 ranked with dense, lexical and rerank); load average 1.4 to 2.6. Full cold-start results within the 30 s safety cap were observed in the coldcaps report (5 searches with all routes and 1 full answer) and the supervisor's 2026-09-28 12:25 live check on `dc96370` (2 searches with all routes); the ask output has no route statuses | The ladder cannot see a dense load wait (post-M1 queue); this run showed none by a process watch. Per-call card lookup, scope recheck, response formatting and stdio framing are not timed |
| SC-S1-005 | Delivered | Synthetic stdio tests (`crates/maestro/tests/it/mcp_stdio.rs`); client guide and stdio smoke (`1dcea41`). Private T038 live client report: Pi, Claude Code, Codex CLI and Copilot CLI each discovered the server, called `knowledge_search` on `ctm` and cited the expected source in the final 2 cases each, one English and one French. A first French case missed its expected source in Pi and Claude Code and was replaced. Copilot's French search failed once at the then-default 1.5 s deadline and passed on its own retry with 10 s; the default has since been raised | None |
| SC-S1-006 | Delivered | `8ee5db4`, `a597417`; `crates/maestro/tests/it/rebuild_drill/backup_loss_drill/backup_loss.rs`, run by `.github/workflows/integration.yml` | None |
| SC-S1-007 | Partial | Synthetic gate `d30573d` and its frozen baseline `e6bb538` (`tests/fixtures/synthetic/evals/baseline.json`); line coverage 95.4 % on the audited run at `ddb67e2` | A complete green CI run with zero missed mutants on the final head (MR-01) |
| SC-S1-008 | Delivered | Private T039 single-route receipts on golden v2.2, `dc96370`, generation 3, 5 warm-ups per rung, search-only, reranking off: dense-only top-10 74 of 84 (88.1 %), lexical-only 70 of 84 (83.3 %), each with 100 ranked rows and no failed searches. These measure dense and BM25 recall separately before fusion; neither alone meets the 90 % top-10 floor. Earlier golden v2.1 run 1 scores were fused: lexical + identifier + structured 59 of 84, hybrid without reranking 72 of 84, not dense-only. Each ladder row carries search and ask outcomes, expected-document rank and refusal code (`80a7ad1`); the manual audit graded only run 3 v2-base's 19 wrong answers on golden v2.1, not the thinking answerer's failures | None |
| SC-S1-009 | Partial | Public CI reads only synthetic fixtures | Content check (MR-06): paused until after M1 by the owner |

### 20.3 Remaining merge-readiness items

The S1 merge-readiness audit (2026-09-27) raised MR-01 to MR-12. State at
`6a4a3b8`:

| Item | Subject | State | Owner and next step |
| --- | --- | --- | --- |
| MR-01 | Whole-diff CI and mutation testing | OPEN | Supervisor: sharded mutation runs on draft #49; the final head needs a complete run with zero missed mutants and zero timeouts |
| MR-02 | Event schemas against their released predecessor | Closed | `580c044`, `.github/workflows/event-schemas.yml` |
| MR-03 | Built-in cursor consumers | Closed | Owner moved them to S2; `2a2fddb` |
| MR-04 | Stage instrumentation and load-wait evidence | Closed | Spans `3af7d66`; the ladder warms the models up and counts timed-out searches apart (`80a7ad1`); SC-S1-004 |
| MR-05 | Import memory bound | Closed | `7d1ff9a` |
| MR-06 | Content check for private vendor material | OPEN | Owner paused it until after M1; `AGENTS.md` now states what the checks do today |
| MR-07 | Exact 08 row inventory and delivery map | Closed | Owner approved the 94 keys (2026-09-28); §20.4 maps them and `crates/maestro-conventions/tests/s1_traceability/` checks them |
| MR-08 | Delivery instructions against the S1 workflow | Closed | `da92520` |
| MR-09 | Corpus and question-review descriptions | Closed | `da92520` |
| MR-10 | Architecture tables and operator examples | Closed | `da92520` |
| MR-11 | Entry documentation still describing S0 | Closed | `da92520` |
| MR-12 | Task checkboxes against landings | Closed | The landed steps of [tasks.md](../../specs/001-knowledge-kernel/tasks.md) are ticked; open steps name their owner here |

### 20.4 Approved S1 row keys

The owner approved these 94 keys on 2026-09-28 (MR-07) as the 08 rows S1
answers for: 93 delivered in whole or in part, and row 53 kept as a visible
deferral. The [S1 spec](../../specs/001-knowledge-kernel/spec.md#traceability)
lists the same keys. **Whole** means S1 delivers the row; **Part** names what
S1 delivers, and the last column names the rest and its slice. No row changes
its status above. Commits are on `feat/s1-integration` at `7b15845` unless
another repository or branch is named; a task named `All` is carried by every
S1 commit. `crates/maestro-conventions/tests/s1_traceability/` refuses a
missing, duplicate or extra key here or in the spec, a key that is not exactly
one row of its section above, an empty cell, an unknown task and a cited test
path that does not exist. Whole and Part rows need a task, a commit hash,
`Every S1 commit` or a named receipt, and an existing test path or named
receipt; Part rows also need a non-placeholder remainder. A fully deferred
row must name its later slice.

| Section | Row | S1 portion | Tasks | Commits | Tests | Remainder and slice |
| --- | --- | --- | --- | --- | --- | --- |
| §3 | `owner.m001.llamacpp` | Part: embedder, reranker and answerer through the llama.cpp router | T002, T006, T018 | `8f75d8e` in maestro-model-router, `b738975`, `fed0d24` | `crates/maestro-kernel/src/gateway/tests/router.rs`, `crates/maestro-knowledge/tests/it/router_parity.rs`, the free-room admission tests of maestro-model-router | Agent sessions on llama.cpp: S4 |
| §3 | `owner.m058` | Part: knowledge telemetry, spans and the ladder benchmark | T007, T021, T037 | `0c69714`, `3af7d66`, `6a6df77`, `c1ecb3b`, `90e8e56`, `46e1dec`, `80a7ad1`, `dac543c` | `crates/maestro-kernel/src/telemetry/tests`, `crates/maestro-knowledge/src/eval/tests`, `crates/maestro-knowledge/src/eval/tests/ladder.rs` | Copilot provider, run telemetry: S4 |
| §3 | `owner.n007` | Part: local chain from prepared Markdown to answer | T019–T037 | `e634ff8`, `02c806e`, `73faa04`, `2144186`, `6bc3ee5`, `f0cfc95` | `crates/maestro-knowledge/tests/it/synthetic_gate/pipeline`, `crates/maestro/tests/it/knowledge_ask.rs` | Native crawl and extraction: S6 |
| §3 | `owner.n020` | Part: Qdrant with fusion | T026, T029 | `fd9414e`, `5e3769e`, `17bfa42`, `db14f92`, `6bc3ee5` | `crates/maestro-knowledge/tests/it/qdrant_projection`, `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes` | Neo4j and graph fusion: S2 |
| §3 | `owner.n029, n065` | Whole: BM25 and dense at their layers | T004, T040, T026, T029 | `1c081c5`, `fd9414e`, `5e3769e`, `17bfa42`, `db14f92`, `6bc3ee5` | `crates/maestro-knowledge/tests/it/lexical_golden.rs`, `crates/maestro-knowledge/tests/it/qdrant_projection`, `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes` | None |
| §3 | `owner.n031` | Whole: source to answer with fusion, reranking, deduplication | T023, T029, T031, T032, T035 | `73faa04`, `5e3769e`, `17bfa42`, `db14f92`, `6bc3ee5`, `cb9b200`, `b1bc8bb`, `887b612`, `311eb85`, `f0cfc95`, `0319633` | `crates/maestro-knowledge/src/prepare/tests/duplicates.rs`, `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes`, `crates/maestro-knowledge/src/search/tests/rerank.rs`, `crates/maestro-knowledge/src/search/evidence/tests`, `crates/maestro-knowledge/src/answer/tests/guardrails.rs` | None |
| §3 | `owner.n047, n053` | Whole: the step after Markdown (import, quality, prepare, publish) | T019, T020, T023, T028 | `e634ff8`, `7d1ff9a`, `02c806e`, `73faa04`, `2144186` | `crates/maestro-knowledge/tests/it/import_contract`, `crates/maestro-knowledge/tests/it/quality_gate`, `crates/maestro-knowledge/src/prepare/tests/duplicates.rs`, `crates/maestro/tests/it/knowledge_publish.rs`, `crates/maestro-kernel/src/generation/tests/publication_events.rs` | None |
| §3 | `owner.n062` | Whole, as adapted (§15 A6): the router tokenizer contract | T013, T018 | `2f83cbc`, `fed0d24` | `crates/maestro-knowledge/src/prepare/tests/counting.rs`, `crates/maestro-knowledge/tests/it/router_parity.rs` | None |
| §3 | `owner.session` | Part: canonicalization carried over, a value slice, public core | T001, T009 | `b771872`, `9405140` | `crates/maestro-kernel/src/artifact/tests.rs`, `crates/maestro-knowledge/tests/it/collection_contract.rs` | Copilot-native formats: S3 |
| §10 | `chat.M048, M057 layers, delivery.§6.1` | Part: deterministic and real-service layers for knowledge | T014, T026, T036 | `a6a27e5`, `fd9414e`, `d30573d`, `e6bb538` | `crates/maestro-knowledge/tests/it/synthetic_collection.rs`, `crates/maestro-knowledge/tests/it/qdrant_projection`, `crates/maestro-knowledge/tests/it/synthetic_gate` | SDK transport and scripted-model layers: S4 |
| §10 | `chat.M048 tools` | Part: Tokio tests under nextest, cargo-mutants | All | `8bf9636`, `ce2ef79` on main | Tokio tests under nextest (`.cargo/mutants.toml`) in every crate; `crates/maestro-knowledge/tests/it/synthetic_gate` | insta snapshots for prompts and exposed tools (§15 A20): post-M1 queue |
| §10 | `chat.M057 CI` | Part: the synthetic suite gates pull requests with a frozen baseline | T036 | `d30573d`, `e6bb538` | `crates/maestro-knowledge/tests/it/synthetic_gate` | Scenario suites for workflows: S4 |
| §10 | `chat.M059 streams` | Part: journal, spans and eval reports kept apart | T007, T010, T021 | `0c69714`, `3af7d66`, `852c4e7`, `6a6df77`, `c1ecb3b` | `crates/maestro-kernel/src/telemetry/tests`, `crates/maestro-kernel/src/journal/tests`, `crates/maestro-knowledge/src/eval/tests` | Workflow execution audit: S4 |
| §10 | `chat.M059 provenance` | Part: observed, estimated and unavailable in tokens, floors and cards | T013, T021, T037 | `2f83cbc`, `6a6df77`, `c1ecb3b`, `90e8e56`, `46e1dec`, `80a7ad1`, `dac543c` | `crates/maestro-knowledge/src/prepare/tests/counting.rs`, `crates/maestro-knowledge/src/eval/tests`, `crates/maestro-knowledge/src/eval/tests/ladder.rs` | Provider usage accounting: S4 |
| §10 | `chat.M059 pitfalls` | Part: distinct latencies; timed-out searches counted apart | T021, T037 | `6a6df77`, `c1ecb3b`, `90e8e56`, `46e1dec`, `80a7ad1`, `dac543c` | `crates/maestro-knowledge/src/eval/tests`, `crates/maestro-knowledge/src/eval/tests/ladder.rs` | Credits and shared counters: S4 |
| §10 | `chat.M059 tracing` | Part: OpenTelemetry spans for knowledge stages | T007 (MR-04) | `3af7d66` | `crates/maestro-kernel/src/telemetry/tests/stages.rs`, `crates/maestro-knowledge/tests/it/synthetic_gate/stage_spans.rs` | Copilot runtime export: S4 |
| §10 | `chat.M059 benchmarks` | Part: engine-level knowledge benchmark (the ladder, bake-off round 1) | T030, T037 | `bf58adb`, `bdf52ca`, `90e8e56`, `46e1dec`, `80a7ad1`, `dac543c` | `crates/maestro-kernel/src/model/tests`, `crates/maestro-knowledge/src/eval/tests/ladder.rs` | Provider and workflow levels: S4 |
| §10 | `chat.M059 capitalize B–H` | Part: organizational corpus, golden set, failure classes | T024, T027, T021 | `a070e11`, `3b5eaac`, `8465a42`, `6a6df77`, `c1ecb3b` | `crates/maestro-knowledge/tests/it/suite_check`, `crates/maestro-knowledge/src/eval/tests`, Private: the golden-set review receipt | Skill and agent ablations: S4, S5 |
| §11.1 | `rag.N011 two circuits` | Whole: import is separate from question answering | T019, T034 | `e634ff8`, `7d1ff9a`, `750e7d6`, `2410855`, `56eae40` | `crates/maestro-knowledge/tests/it/import_contract`, `crates/maestro/tests/it/mcp_stdio.rs` | None |
| §11.1 | `ingest scope` | Whole: source rules and inventory stay private | T003 | Private: the corpus mapping (ADR-0009) | Private: the mapping checks (ADR-0009); public contract `crates/maestro-knowledge/tests/it/corpus_contract.rs` | None |
| §11.1 | `rag.N046 filter` | Whole: no query-specific filtering at import | T019 | `e634ff8`, `7d1ff9a` | `crates/maestro-knowledge/tests/it/import_contract` | None |
| §11.1 | `product.CD2` | Part: one-off manual import per source | T019 | `e634ff8`, `7d1ff9a` | `crates/maestro-knowledge/tests/it/import_contract` | Per-source synchronization: S6 |
| §11.2 | `rag.N052 quality` | Whole: accept, warn, re-extract, quarantine | T020 | `02c806e` | `crates/maestro-knowledge/src/quality/tests`, `crates/maestro-knowledge/tests/it/quality_gate` | None |
| §11.3 | `rag.N054–N056` | Whole: the canonical document contract | T013, T023 | `2f83cbc`, `73faa04` | S0 gate still run in S1: `crates/maestro-canonicalization/tests/it/document_contract.rs`; the counter seam and prepare preserve the canonical contract | None |
| §11.3 | `rag.N060 A01–A34` | Whole: gates re-verified on the migrated crate | T013, T023 | `2f83cbc`, `73faa04` | S0 gate still run in S1: `crates/maestro-canonicalization/tests/it/phase_a_acceptance.rs`; the counter seam and prepare preserve these acceptance fixtures | None |
| §11.3 | `rag.N038 dedup` | Whole: exact and prepared-input fingerprints, near grouping | T023 | `73faa04` | `crates/maestro-knowledge/src/prepare/tests/duplicates.rs` | Release-copy storage dedup: after M1 |
| §11.3 | `rag.N060 near-dup` | Whole, as adapted (§15 A22): non-destructive grouping | T023 | `73faa04` | `crates/maestro-knowledge/src/prepare/tests/near.rs` | None |
| §11.3 | `rag.N060 B01–B13` | Whole: chunking gates, records, manifests | T023 | `73faa04` | `crates/maestro-knowledge/src/prepare/tests/chunk_sets.rs`, `crates/maestro-canonicalization/tests/it/chunk_contract.rs` | None |
| §11.3 | `rag.N052 chunk policy` | Whole: the existing chunker's policy | T023 | `73faa04` | `crates/maestro-canonicalization/tests/it/chunk_contract.rs` | None |
| §11.3 | `rag.N011 sizes` | Whole, as adapted (§15 A9) | T023 | `73faa04` | `crates/maestro-knowledge/src/prepare/tests/oversized.rs` | None |
| §11.3 | `rag.N064 tokenizer` | Whole, as adapted (§15 A6): ordered-ID parity | T018 | `fed0d24` | `crates/maestro-knowledge/tests/it/router_parity.rs` | None |
| §11.3 | `rag.N067 contracts` | Whole: embedding tokenizer and BM25 analyzer apart | T018, T040 | `fed0d24`, `1c081c5` | `crates/maestro-knowledge/tests/it/router_parity.rs`, `crates/maestro-knowledge/tests/it/lexical_golden.rs` | None |
| §11.4 | `rag.N070 EmbeddingProfile` | Whole | T006, T026 | `b738975`, `fd9414e` | `crates/maestro-kernel/src/gateway/tests`, `crates/maestro-knowledge/tests/it/qdrant_projection` | None |
| §11.4 | `rag.N070 Bm25Profile` | Whole | T040 | `1c081c5` | `crates/maestro-knowledge/tests/it/lexical_golden.rs` | None |
| §11.4 | `rag.N070 index` | Whole | T026 | `fd9414e` | `crates/maestro-knowledge/tests/it/qdrant_projection` | None |
| §11.4 | `rag.N070 publication` | Whole | T028 | `2144186` | `crates/maestro-knowledge/tests/it/qdrant_projection/publication_verification.rs`, `crates/maestro/src/cli/tests/publication_resume.rs` | None |
| §11.4 | `rag.N070 diagnostics` | Whole: dense and BM25 searched apart before fusion | T029 | `5e3769e`, `17bfa42`, `db14f92`, `6bc3ee5` | `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes/route_behavior.rs` | None |
| §11.4 | `rag.N038 statistics` | Whole: IDF per generation | T026, T040 | `fd9414e`, `1c081c5` | `crates/maestro-knowledge/tests/it/qdrant_projection/alias_moves.rs`, `crates/maestro-knowledge/tests/it/lexical_sample.rs` | None |
| §11.4 | `rag.N011, N016 automation` | Part: durable jobs, leases, resume, separate budgets | T016, T028 | `6379c7a`, `2144186` | `crates/maestro-kernel/src/job/tests`, `crates/maestro/tests/it/knowledge_publish.rs`, `crates/maestro/src/cli/tests/publication_resume.rs` | Change-driven jobs, deletion propagation: S6; interactive work has priority over ingestion (01 §10): post-M1 |
| §11.4 | `rag.N052 recompute` | Part: a new generation recomputes everything | T012, T028 | `e24a02b`, `2144186` | `crates/maestro-kernel/src/generation/tests/lifecycle.rs` (a new generation starts without points or publication), `crates/maestro-kernel/src/generation/tests/publication_events.rs` | Dependency-driven recomputation: S6 |
| §11.4 | `rag.N075 first deliverable` | Whole | T019, T023, T028 | `e634ff8`, `7d1ff9a`, `73faa04`, `2144186` | `crates/maestro-knowledge/tests/it/import_contract`, `crates/maestro-knowledge/src/prepare/tests/duplicates.rs`, `crates/maestro/tests/it/knowledge_publish.rs`, `crates/maestro-kernel/src/generation/tests/publication_events.rs` | None |
| §11.5 | `rag.N038 context` | Whole: trusted scope, routes, identifier route | T011, T029 | `8dcab4b`, `5e3769e`, `17bfa42`, `db14f92`, `6bc3ee5` | `crates/maestro-kernel/src/scope/tests`, `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes` | None |
| §11.5 | `rag.N038 identity dedup` | Whole | T029 | `5e3769e`, `17bfa42`, `db14f92`, `6bc3ee5` | `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes/route_behavior.rs`, `crates/maestro-knowledge/src/search/tests/fusion.rs` | None |
| §11.5 | `rag.N038 RRF` | Whole | T029 | `5e3769e`, `17bfa42`, `db14f92`, `6bc3ee5` | `crates/maestro-knowledge/src/search/tests/fusion.rs` | None |
| §11.5 | `rag.N038 budgets` | Part: dense, BM25, rerank and final budgets | T029, T031 | `5e3769e`, `17bfa42`, `db14f92`, `6bc3ee5`, `cb9b200` | `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes`, `crates/maestro-knowledge/src/search/tests/rerank.rs` | Graph items: S2 |
| §11.5 | `rag.N038 rerank` | Whole | T031 | `cb9b200` | `crates/maestro-knowledge/src/search/tests/rerank.rs` | None |
| §11.5 | `rag.N038 context dedup` | Part: mirrors once, span unions, redundancy | T032 | `b1bc8bb`, `887b612` | `crates/maestro-knowledge/src/search/evidence/tests` | Support groups for graph paths: S2 |
| §11.5 | `rag.N038 EvidenceBundle` | Part: passages, conflicts, known gaps, trace | T017, T032 | `0495f5f`, `b1bc8bb`, `887b612` | `crates/maestro-kernel/src/evidence/tests`, `crates/maestro-knowledge/src/search/evidence/tests` | Claims and paths: S2 |
| §11.5 | `rag.N038 answer` | Whole: cited, validated answers | T035 | `311eb85`, `f0cfc95`, `0319633` | `crates/maestro-knowledge/src/answer/tests/guardrails.rs` | None |
| §11.5 | `rag.N038 degradation` | Whole: explicit route status, rerank fallback | T029, T031 | `5e3769e`, `17bfa42`, `db14f92`, `6bc3ee5`, `cb9b200` | `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes/route_errors.rs`, `crates/maestro-knowledge/src/search/tests/rerank.rs` | None |
| §11.5 | `rag.N016 interfaces` | Part: Rust functions, CLI, MCP | T034 | `750e7d6`, `2410855`, `56eae40` | `crates/maestro/tests/it/mcp_stdio.rs` | Local HTTP API: S4 |
| §11.5 | `rag.N016 tools` | Part: search, get, ask | T034, T035 | `750e7d6`, `2410855`, `56eae40`, `311eb85`, `f0cfc95`, `0319633` | `crates/maestro/tests/it/mcp_stdio.rs`, `crates/maestro-knowledge/src/answer/tests/guardrails.rs` | Entity resolve, graph expand, evidence trace, research: S2 |
| §11.5 | `rag.N016 agent` | None delivered; kept as a key so the deferral is visible | None | None | None | Research loop: S2 |
| §11.5 | `rag.N016 local` | Whole: the strict local chain | T002, T006, T035 | `8f75d8e` in maestro-model-router, `b738975`, `311eb85`, `f0cfc95`, `0319633` | `crates/maestro-kernel/src/gateway/tests/router.rs`, `crates/maestro-knowledge/src/answer/tests/guardrails.rs`, the free-room admission tests of maestro-model-router | None |
| §11.5 | `rag.N016 permissions` | Part: checked at search, read and output | T011, T034 | `8dcab4b`, `750e7d6`, `2410855`, `56eae40` | `crates/maestro-kernel/src/scope/tests`, `crates/maestro/tests/it/mcp_stdio.rs` | Traversal, communities: S2 |
| §11.5 | `rag.N011 evaluation` | Whole: 100 questions, the ladder, failure classes | T021, T024, T037 | `6a6df77`, `c1ecb3b`, `a070e11`, `3b5eaac`, `90e8e56`, `46e1dec`, `80a7ad1`, `dac543c` | `crates/maestro-knowledge/src/eval/tests`, `crates/maestro-knowledge/src/eval/tests/ladder.rs` | None |
| §11.5 | `product.CD3` | Part: scoped search with grants | T011 | `8dcab4b` | `crates/maestro-kernel/src/scope/tests` | Project default scope: S3 |
| §12 | `delivery.U03 (F7–F8)` | Part: tokenizer parity | T018 | `fed0d24` | `crates/maestro-knowledge/tests/it/router_parity.rs` | Binding done in S0 |
| §12 | `delivery.U11 (D2–D3)` | Part: local prepared-document operation, knowledge CLI | T019, T022 | `e634ff8`, `7d1ff9a`, `dc7d3c9`, `87b2aeb` | `crates/maestro-knowledge/tests/it/import_contract`, `crates/maestro/tests/it/cli_contract.rs` | Durable native ingestion: S6 |
| §12 | `delivery.U13` | Part: Qdrant adapter, embedding bake-off | T026, T030 | `fd9414e`, `bf58adb`, `bdf52ca` | `crates/maestro-knowledge/tests/it/qdrant_projection`, `crates/maestro-kernel/src/model/tests` | Workflow discovery: S3 |
| §12 | `delivery.U14` | Part: scoped knowledge retrieval | T029 | `5e3769e`, `17bfa42`, `db14f92`, `6bc3ee5` | `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes` | Memory gate: S7-I1 |
| §12 | `delivery.U15` | Part: knowledge stage telemetry | T007 | `0c69714`, `3af7d66` | `crates/maestro-kernel/src/telemetry/tests` | S4–S5 |
| §12 | `delivery.U16` | Part: bake-off round 1 on the private corpus | T030, T037 | `bf58adb`, `bdf52ca`, `90e8e56`, `46e1dec`, `80a7ad1`, `dac543c` | `crates/maestro-kernel/src/model/tests`, `crates/maestro-knowledge/src/eval/tests/ladder.rs` | Runner and providers: S4 |
| §12 | `delivery.C03 Rust-first, explicit native dependencies, justified Python` | Whole for S1 code | All | Every S1 commit: Rust only, no Python adapter | The gate: dependency and licence checks (`maestro-quality.toml`) | None |
| §12 | `delivery.C14–C16 catalog MCP, shared Qdrant, separate knowledge ACLs` | Part: shared Qdrant, knowledge ACLs | T011, T026 | `8dcab4b`, `fd9414e` | `crates/maestro-kernel/src/scope/tests`, `crates/maestro-knowledge/tests/it/qdrant_projection` | Catalog MCP: S3 |
| §12 | `delivery.C18–C21 layered tests, runner, real controls, honest tiers` | Part: knowledge test layers | T014, T036 | `a6a27e5`, `d30573d`, `e6bb538` | `crates/maestro-knowledge/tests/it/synthetic_collection.rs`, `crates/maestro-knowledge/tests/it/synthetic_gate` | Workflow runner: S4 |
| §12 | `delivery.C22–C24 telemetry streams, coverage, units` | Part: knowledge streams, 90 % coverage | T007, T036 | `0c69714`, `3af7d66`, `d30573d`, `e6bb538` | `crates/maestro-kernel/src/telemetry/tests`, `crates/maestro-knowledge/tests/it/synthetic_gate` | Run telemetry: S4 |
| §12 | `delivery.C25–C29 provider qualification, benchmarks, corpus, taxonomy, promotion loop` | Part: knowledge-role qualification, corpus, failure taxonomy | T006, T030, T037 | `b738975`, `bf58adb`, `bdf52ca`, `90e8e56`, `46e1dec`, `80a7ad1`, `dac543c` | `crates/maestro-kernel/src/gateway/tests`, `crates/maestro-kernel/src/model/tests`, `crates/maestro-knowledge/src/eval/tests/ladder.rs` | Provider qualification, promotion loop: S4 |
| §6 | `chat.M059 model profile` | Part: model cards for knowledge roles | T006, T030 | `b738975`, `bf58adb`, `bdf52ca` | `crates/maestro-kernel/src/gateway/tests`, `crates/maestro-kernel/src/model/tests` | Agent model profiles: S4 |
| §6 | `chat.M059 capitalize A` | Part: qualification by knowledge role | T006, T030 | `b738975`, `bf58adb`, `bdf52ca` | `crates/maestro-kernel/src/gateway/tests`, `crates/maestro-kernel/src/model/tests` | Agent roles: S4 |
| §6 | `rag.N075 profiles` | Part: strict local only | T035 | `311eb85`, `f0cfc95`, `0319633` | `crates/maestro-knowledge/src/answer/tests/guardrails.rs` | Managed Copilot profile: S4 |
| §7 | `chat.M006 audit` | Part: the journal as audit for knowledge | T010, T015 | `852c4e7`, `4826626` | `crates/maestro-kernel/src/journal/tests`, `crates/maestro-kernel/src/journal/tests/schemas.rs` | Broker path: S4 |
| §8 | `chat.M031 separation` | Part: knowledge collections, pipelines and scopes | T009, T011 | `9405140`, `8dcab4b` | `crates/maestro-knowledge/tests/it/collection_contract.rs`, `crates/maestro-kernel/src/scope/tests` | Catalog discovery: S3 |
| §9 | `chat.M006 CLI` | Part: noun-verb `maestro knowledge`, `eval`, `backup` commands | T022, T025, T033, T034, T037 | `dc7d3c9`, `87b2aeb`, `86b9580`, `0146bf9`, `750e7d6`, `2410855`, `56eae40`, `80a7ad1` | `crates/maestro/tests/it/cli_contract.rs`, `crates/maestro/tests/it/doctor_checks.rs`, `crates/maestro/tests/it/backup_restore.rs`, `crates/maestro/tests/it/mcp_stdio.rs`, `crates/maestro/src/cli/eval/tests` | `init`, `run`, `explain`: S3, S4 |
| §9 | `product.GD2, GD4, GD5` | Part: knowledge MCP in four clients, local only | T034, T038 | `750e7d6`, `2410855`, `56eae40`, `1dcea41` | `crates/maestro/tests/it/mcp_stdio.rs`, `crates/maestro/tests/it/mcp_clients.rs` | Hook administration: S4 |
| §11.6 | `rag.N023 roles` | Part: Qdrant for passages, Rust fusion | T026, T029 | `fd9414e`, `5e3769e`, `17bfa42`, `db14f92`, `6bc3ee5` | `crates/maestro-knowledge/tests/it/qdrant_projection`, `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes` | Neo4j relations: S2 |
| §11.6 | `rag.N052 publication` | Part: index generations published atomically | T012, T028 | `e24a02b`, `2144186` | `crates/maestro-kernel/src/generation/tests/publication.rs`, `crates/maestro-kernel/src/generation/tests/publication_events.rs` | Graph published with them: S2 |
| §13.2 | `product.CD2` | Part: one-off import | T019 | `e634ff8`, `7d1ff9a` | `crates/maestro-knowledge/tests/it/import_contract` | Per-source synchronization, multimedia: S6 |
| §13.2 | `product.CD3` | Part: scoped retrieval | T011 | `8dcab4b` | `crates/maestro-kernel/src/scope/tests` | Project default scope: S3 |
| §13.2 | `product.GD1–GD5` | Part: four initial clients for knowledge | T038 | `1dcea41` | `crates/maestro/tests/it/mcp_clients.rs` | Provider configuration, graph advice, hooks: S2, S4 |
| §14 | `core storage` | Whole for the kernel's artifact store | T001 | `b771872` | `crates/maestro-kernel/src/artifact/tests.rs` | None |
| §14 | `core scale` | Whole for import and publication | T016, T019, T028 | `6379c7a`, `e634ff8`, `7d1ff9a`, `2144186` | `crates/maestro-kernel/src/job/tests`, `crates/maestro-knowledge/tests/it/import_contract`, `crates/maestro/tests/it/knowledge_publish.rs`, `crates/maestro/src/cli/tests/publication_resume.rs` | None |
| §14 | `core native` | Whole: tokenizer qualification separate from binding | T018 | `fed0d24` | `crates/maestro-knowledge/tests/it/router_parity.rs` | None |
| §14 | `core baseline` | Whole for S1: gates, 90 % coverage, hooks | T036 | `8bf9636`, `d30573d`, `e6bb538`, `ce2ef79` on main | `.github/workflows/integration.yml`, `crates/maestro-knowledge/tests/it/synthetic_gate` | Zero missed mutants before the merge (MR-01) |
| §15 | `A1` | Whole: the kernel holds jobs and publication metadata | T005, T012, T016, T028 | `9c4463e`, `e24a02b`, `6379c7a`, `2144186` | `crates/maestro-kernel/src/store/tests/migrations.rs`, `crates/maestro-kernel/src/job/tests`, `crates/maestro-kernel/src/generation/tests/publication.rs`, `crates/maestro-kernel/src/generation/tests/publication_events.rs` | Claims: S2 |
| §15 | `A4` | Whole: candidates in a recorded bake-off | T030, T037 | `bf58adb`, `bdf52ca`, `90e8e56`, `46e1dec`, `80a7ad1`, `dac543c` | `crates/maestro-kernel/src/model/tests`, `crates/maestro-knowledge/src/eval/tests/ladder.rs` | None |
| §15 | `A5` | Whole: the existing chunker | T023 | `73faa04` | `crates/maestro-canonicalization/tests/it/chunk_contract.rs`, `crates/maestro-knowledge/src/prepare/tests/chunk_sets.rs` | None |
| §15 | `A6` | Whole: the router's tokenizer | T018 | `fed0d24` | `crates/maestro-knowledge/tests/it/router_parity.rs` | None |
| §15 | `A7` | Whole: the llama.cpp router first | T002, T006 | `8f75d8e` in maestro-model-router, `b738975` | `crates/maestro-kernel/src/gateway/tests/router.rs`, the free-room admission tests of maestro-model-router | None |
| §15 | `A9` | Whole: target 500, maximum 700 | T023 | `73faa04` | `crates/maestro-knowledge/src/prepare/tests/oversized.rs` | None |
| §15 | `A14` | Whole: Qdrant sparse vectors from `bm25-en-fr/1` | T040, T026 | `1c081c5`, `fd9414e` | `crates/maestro-knowledge/tests/it/lexical_golden.rs`, `crates/maestro-knowledge/tests/it/qdrant_projection` | None |
| §15 | `A22` | Whole: near-duplicate grouping built in S1 | T023 | `73faa04` | `crates/maestro-knowledge/src/prepare/tests/near.rs` | None |
| §15 | `A26` | Whole: knowledge kernel and Control-M RAG first | All | Every S1 commit | `crates/maestro-knowledge/tests/it/synthetic_gate`, `crates/maestro-knowledge/src/eval/tests/ladder.rs` | None |
| §17 | `Operational bindings: endpoints, accounts, data scopes, exclusion registries, budgets` | Part: supplied as machine configuration for import | T003, T019 | Private: the corpus mapping (ADR-0009), `e634ff8`, `7d1ff9a` | `crates/maestro-knowledge/tests/it/corpus_contract.rs`, `crates/maestro-knowledge/tests/it/import_contract` | Live bindings: S6 |

## 21. S3 contract inventory (C00, not delivery evidence)

The [S3 inventory](../../specs/003-catalog/traceability.json) records the 85
included exact row keys in the [S3 spec](../../specs/003-catalog/spec.md#traceability),
including combined keys verbatim, and six reasoned S5-only exclusions.
Every included row names its S3 portion, implementation tasks and remaining work
by slice. All rows are **planned**, not delivered;
C28 supplies integrated evidence. This frozen inventory, with its exact keys,
S3 and remaining portions and exclusions, is approved by the owner, 2026-09-28
(C00 inventory approval). This does not approve S1's separate T039 inventory or waive any M3 exit.

The runnable check is
`crates/maestro-conventions/tests/catalog_traceability/`. It compares the
frozen exact keys against this document and the S3 table. Completeness is
also derived from source tables: every row whose Where/Where now column links
`03-agent-orchestration.md#1…` or `06-roadmap.md#s3-catalog--m3`, or whose
Status/Slice now column names S3, must be included or explicitly excluded with
a reason. The broad `#1` prefix catches §10 too; its six S5-only rows have
explicit exclusions, not silent omissions. Checks reject missing, duplicate or
extra rows, absent portions, unknown/empty tasks and unnamed or lost remaining
slices. Negative fixtures mutate both JSON and spec rows; successful checks
prove consistency and candidate coverage, not owner approval or runtime delivery.

| Contract | Disposition and evidence still required |
| --- | --- |
| D1 authoring boundary | C02 small reviewed-source content and C08 owner loop precede M3 as authoring convenience only. Source digests/authoring locks cannot become attested installs; workflows stay inert and normal install/update keeps verification. No M3 criterion is removed |
| D2–D4 decided inputs | Pinned gh with separate publishers, ≤5-minute refresh and ≤24-hour offline validity; synthetic public data with exact approval for private use; installed parsers plus the approved measured libraries. OA1/OA2/OA4/OA5/OA6 supply external operations and qualification evidence, not new D1–D5 choices |
| D5 routing | C23 freezes 100+ reviewed CORE synthetic/public cases: ≥20 tuning, ≥80 held-out, ≥60 held-out matchable, ten eligible synthetic workflows and a digest-pinned compiled bundle/eligibility fixture. OA10 approved by the owner, 2026-09-28: held-out matchable top-1 ≥ 90 % is the absolute gate, measuring first-selection correctness. This dated amendment to D5 replaces its original top-3 ≥ 90 % bar, retained as history, not current acceptance. Exact closure completeness is 100 %. Report both top-1/top-3, negative cohorts, unnecessary context, distractors and latency. S3 D5 uses the 95 % paired top-1 bootstrap (seed 42, S1's 2,000 resamples); C26 exposes that seam without changing S1 results. Hybrid needs a strictly positive gain interval, else ship the passing baseline and retain the comparison. Real executable workflows remain incompatible until S4 |
| Parallel start, 2026-09-28 08:12 | S2/S3 start while S1 finishes. Integrated T034/T035 and T038 live evidence gate C08/C28; M1 release gates M3 exit, not C00 or independent fixture work |
| Static versus runtime | S3 C22a/C22b check all twelve 03 §2.3 rules. Reviewed means declared stage plus named owner on OA1 protected-branch CODEOWNERS-reviewed content; C03 validates declaration/owner, C15 protected publication. Record/show each compiled member's maturity/owner in lock and preview/explain; local checks do not prove a remote review. S4 raises executable admission to qualified and owns execution, broker, sandbox, acceptance and general role/provider qualification; projectable is not route-eligible |
| Hook scope | Owner v4 amendment: S3 keeps four-client local MCP, real Cedar fixtures and ten-point host mapping evidence only. C20 live Copilot normalization/allow/deny/error-to-deny and 4 h move to S4 with the other trusted host adapters. Missing hooks are unprotected; static fixtures never prove live enforcement |
| Impact ownership | C12 scoped kernel records are authority. S3 C27a owns catalog edge schema, read/write adapters and snapshot bindings over S2 G27's public typed-edge port after G25 qualification; C27 traverses that separate rebuildable projection. No fabricated evidence-span claims, similarity edges or implicit in-memory fallback. Missing S2 evidence blocks impact/M3 |
| Migration allocation | C00 reserves no number. The supervisor rechecks every landed/reserved number on main, S1/S2/S3 and deployment-modes at each landing, then allocates the next free one above them. Never a fixed/gapped block or a rewrite of an applied migration; dated observations belong in the S3 plan, not this rule |

Architecture [03](03-agent-orchestration.md) and
[06 S3](06-roadmap.md#s3-catalog--m3) carry the same boundaries. C00 performs
no host installation, account access, repository creation, release, trust
rotation or private-data operation. Missing live evidence remains missing.

## 22. Owner amendments, 2026-09-28 11:25–12:05

The [S3 amendment coverage](../../specs/003-catalog/spec.md#exact-requirement-coverage-after-v4)
refines existing source rows, not a new source-key inventory. C00's 85 included
keys, six exclusions and planned statuses remain unchanged; no new JSON row or
inventory-test exemption is needed. The supplemental FR/task map must become
part of C28's evidence for these existing rows, never an untracked M3 promise.

| Amendment | Existing source rows and planned responsibility |
| --- | --- |
| Workspace config and polished init | `owner.m001.cli`, `owner.m001.load`, `chat.M019 bootstrap, M023 step 13`: C05a/C05b/C05g/C05j implement strict preferences, owner/home/trust-bounded discovery, free flags > workspace > user > defaults, successful root-only writes and plain/script parity. C05f/C05k add the approved branded renderer after the first owner loop |
| Conversation versus artifacts | `owner.m001.load`, `chat.M036 classes, M039 policy`, `delivery.C08 explicit instructions and skills with provenance`: C05c/C05l/C05d/C05e use the bounded canonical BCP 47 subset, question-language fallback and evaluation isolation. Interface en/fr/es, English fallback, tone only for generated prose. MCP --workspace or user preferences, path-free source; native files contain fixed English rules only. C05j/C06/C07 prove byte invariance |
| Verified startup updates | `chat.M006 release`, `chat.M027 TUF`, `delivery.C17 signed releases, freshness, revocation, transparency`, `owner.m024`: C16c–C16g add off/daily discovery, user-only catalog auto with narrowing ceilings, widening/hook consent, invariant receipts and trusted catalog rollback. Runtime propose-only; MCP never applies or carries release-note instructions; no mid-task activation |
| User-approved workspace path trust | `owner.m001.guardrails`, `owner.m032`, `chat.M036 classes, M039 policy`, `chat.M006 destructive`: C05h/C05i/C05j and transferred C20/S4 use kernel-only canonical trust records, explicit add/list/remove with exact confirm-path for CI, root/HOME refusal and unchanged edited preferences. Shared policy denies outside writes and secret reads/escapes; C20/S4 denies Copilot agent-shell trust commands; all host containment stays S4; internal paths never become tool grants |
| Modular delivery | The same rows above: `WorkspacePreferences`, `WorkspaceTrust`, `UpdateSource`, pure instruction construction and `ClientPreferencesDelivery` are small typed ports. New clients/sources add adapters without editing callers; no adapter can weaken mandatory controls or imply a plugin runtime |

### Named remaining obligations

- **S4 session-preferences launch-adapter obligation** (remaining runtime
  portion of `delivery.C08 explicit instructions and skills with provenance`):
  consume C05e's shared fragment in every actual launch, resume and delegation.
  Test adapter payloads on both providers with each tone and a non-interface
  language; preserve selected tag and English artifact/log rules, reject
  model-supplied preference replacement. S3 tests construction/delivery only.
- **S4 workspace-trust hook obligation** (remaining portion of
  `product.GD2, GD4, GD5` and the existing hook deferral): transferred C20
  Copilot (4 h), Pi, Codex and Claude Code must use qualified trusted event/identity adapters with the same path
  policy. Test actual outside-write, secret-read, symlink-escape and agent-shell
  trust administration, including correct --confirm-path arguments, with allowed
  neighbours and zero executor calls on denial.
  S4 also uses the shared update idle lease around tasks/sessions. No S3
  instruction or preference setting is runtime containment evidence.
- **Notify-only update checks for owner-managed components (Qdrant, router runtime,
  host clients, models)** (remaining owner-managed lifecycle follow-up, not an
  S3 exit): only propose after each has an approved source; never apply/download.
  S3 discovers released Maestro and installed catalogs with pinned closures,
  but applies only catalog updates; Cargo and pinned `gh` stay outside.
- **Runtime auto-apply with the installer** (remaining runtime portion of
  `owner.m024` and release-lifecycle rows): define installation layout, platform
  startup handoff, idle exclusion, retained binaries/state, durable receipts and
  trust-checked rollback. S3 C16e/C16g only proposes verified releases with an
  exact approved install command; no automatic runtime switch or rollback.
- **Roots-based workspace detection, once a client-qualified channel exists**
  (remaining client portion of `product.GD2, GD4, GD5`): roots arrive after MCP
  initialization, with no standard instructions-changed notification. S3 C05e
  uses --workspace or user preferences only, not per-client notification schemes.

These obligations also appear in [06](06-roadmap.md). On 2026-09-28, OA9
approved ratatui + crossterm under ADR-0020; C05f measurements and C05k visual
acceptance before C28 remain required. OA2 approved tests of already-installed
Copilot CLI, Pi, Claude Code and Codex in isolated temporary homes; C01 records
exact installed pins. No installs/upgrades, real owner configuration or enterprise
policy changes; broader scope needs fresh OA2 approval. Plain C08 does not wait
for TUI evidence; the tag grammar needs no parser dependency. None of these
approvals or dispositions claims that tasks or live checks have run.

## 23. Manifest v4, owner approval 2026-09-30

[ADR-0022](../adr/0022-manifest-layout-v4-and-language-neutral-extensions.md),
[S3 D13–D15](../../specs/003-catalog/plan.md#d13-manifest-v4-source-contract)
and the [exact requirement map](../../specs/003-catalog/tasks.md#requirements-coverage)
replace the earlier layout, single-owner and live-hook dispositions. The 85
source keys and six exclusions remain frozen; no architecture row is invented
for a new FR. traceability.json adds exact FR/task, MD01–MD16 and G01–G13 links,
all planned, not delivered. There are 68 FR and 25 SC after the amendment.

Mandatory standards/common, framework core and selected teams/languages compose
by explicit closures, not overwrite precedence. Ten personas and eight language
profiles ship in Phase 1; knowledge-only projection stays thin. Delegated
maintainers cannot self-approve ownership; trusted CI verifies identities and
base-owner exact-head approval. Schemas/indexes/CODEOWNERS are generated from
one checked registry. Standard imports remain read-only until Phase 2/ST1's
single-authority switch. No copied non-Rust gate or automatic private grant.

The minimal amendment is 149 h Phase 1 and 121 h Phase 2, beginning immediately
after M3. G07–G11 guides/guard evidence cost 12 h in Phase 1; G12 context limits
cost 6 h in Phase 2, per the supervisor's correction. C29 audits 395 legacy
inputs before M3 with explicit deferred recovery. C20's 4 h transfers to S4.
Independent package releases, scaffolder, generic evals, extensions and named
gap kits retain their Phase 2 milestones. Any-language extensions run out of
process: MCP actions, separate durable engine events, no launch during S3
check/compile/install. Standards/pins/signing/trust/owned-removal and existing
M3 quality controls are not deferred. S6's restricted private collection mount,
seven later language-gate projects and post-M1 live model/router work remain
separate; no private content was read or published by this amendment.

The owner at 20:45 made manifests the sole home for URL policy/decision/
promotion/expiry/migration JSON. `delivery.§2.2` now maps source-owned rules,
core-derived published schemas and signed-release plus recorded owner/maintainer
review; collections carry only strict JSON declarations and exact references.
C66 rises 3→4 h, giving the 149/121 h totals above; C52a/b/C68 and C41/C43 add
0 h. FR-S3-068/SC-S3-025 carry this new contract without another architecture
key. S6 alone supplies the runtime `PolicySource`/`ResourceSource` catalog
adapter; its task/hours are assigned separately. C66's N07 `IdentityMigration`
type synchronization is pending landing, not delivered evidence or permission
to use the under-review commit. Private source rules use admitted private
packages (C69); C42 stays restricted and core stores no real per-site rules.
