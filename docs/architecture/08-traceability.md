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
| `ingest` | Full ingestion design, ingestion source audit, one-page Spider spike and the source-policy proposal (2026-09-22) | Native acquisition lifecycle, sessions, policy, 15 sources |
| `delivery` | Unified delivery plan (2026-09-22) | Tasks U01–U18 with folded F0–F8, Q1–Q4, D1–D5; dispositions C01–C30; corrections R01–R11 |
| `product` | Provider analysis: master product specification, capability and Graphify decisions made with the owner, native Rust product direction, visual workbench, native ingestion specification | C01–C16, M01–M11, U01–U15, J01–J08, A01–A20, N01–N10, D01–D11, CD1–CD8, GD1–GD5, V01–V12 |

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
| chat.M006 compiler | Structure, references, duplicate IDs, cycles, permission analysis, contract compatibility, normalization, native generation; never last-file-wins | Kept | [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install), [§2.3](03-agent-orchestration.md#23-compile-time-validation), [§1.6](03-agent-orchestration.md#16-configuration-and-overrides) |
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
| chat.M031 hybrid | Dense + BM25 + RRF; eligibility filters in every branch; optional rerank | Kept | [03 §1.4](03-agent-orchestration.md#14-routing-an-intent-to-a-workflow) |
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
| product.GD2, GD4, GD5 | Four initial clients; hook administration; local-only access | Kept | [03 §1.5](03-agent-orchestration.md#15-native-projection-convenience-mode), [04 §1](04-intelligence-backend.md#1-scope-and-stance) |

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
| ingest scope | Official docs, community, KB, GitHub, attachments, distribution catalogue; installers asset-only; private archives a separate ingress; 15 sources and 40 entry URLs | Kept | [01 §1](01-knowledge-pipeline.md#1-collections-sources-and-scopes), [§2.2](01-knowledge-pipeline.md#22-s6--native-acquisition) |
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
| product.GD1–GD5 | Provider configuration, four initial clients, non-blocking graph advice, hook administration, local access | Kept | §6, §9, [04 §7](04-intelligence-backend.md#7-context-management-the-undecided-489-surfaces) |
| product.NP | Native Rust desktop; Rust wherever feasible; justified Python exceptions; no framework reuse; one backend; three platforms | Kept | ADR-0016, [04 §1](04-intelligence-backend.md#1-scope-and-stance), [§8](04-intelligence-backend.md#8-phase-i4--workbench) |

## 14. Foundation and engineering

| ID | Requirement | Status | Where |
| --- | --- | --- | --- |
| core ownership | `maestro-core` is the complete production home; no separate runtime repository | Kept | README §7 |
| core no stubs | No empty command handlers, interface crates or stub supervisor | Kept | Constitution principle II, README §8 |
| core dependency rule | Adapters → application → processing → I/O; no transport types in processing; no cycles | Kept | README §8 |
| core abstractions | New crates, traits and registries only for a real variation | Kept | Constitution principle III |
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
| A11 | Empty agent metadata with a separate descriptor (chat.M019) | Frontmatter `metadata:` if Copilot tolerates it, else a sidecar | Decided by an S3 spike |
| A12 | Ordered workflow steps (chat) | Workflow graphs with bounded loops and joins (ADR-0006) | Expresses reviews, repairs and fan-out |
| A13 | Rig as the agent abstraction (rag.N016) | The in-house engine and sessions | Semantics are host-specific |
| A14 | Tantivy for lexical search (rag.N011) | Qdrant server-side BM25; Tantivy only if tests show gaps | One engine |
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
| Publisher identities, trust roots, key rotation procedure | S3 | Manifests release workflow identity; documented rotation |
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
