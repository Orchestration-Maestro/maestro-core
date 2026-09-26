# Maestro

An organizational agent platform: a governed catalog of agents and workflows, a
runtime that executes them under host-enforced policy, and a knowledge kernel
that serves source-backed evidence.

## Kernel

**Kernel**:
The single authoritative store: SQLite plus a content-addressed artifact store.
_Avoid_: database, backend (when meaning the authority)

**Scope**:
A node of the access tree (workspace, collection, source, project, agent, run,
session) that every read is filtered by.
_Avoid_: tenant, namespace

**Journal**:
The append-only log of events (runs, jobs, captures, decisions) kept by the
kernel.
_Avoid_: log (for diagnostics), audit trail

**Artifact**:
Immutable bytes stored and addressed by their SHA-256 digest.
_Avoid_: blob, file

**Projection**:
A derived, rebuildable view of kernel data in another engine (Qdrant, Neo4j, a
cache), stamped with a generation.
_Avoid_: index (for the whole concept), replica

**Generation**:
One complete, verified build of a projection for a collection, bound to exact
profiles; readers are pinned to one.
_Avoid_: version, snapshot

## Knowledge

**Collection**:
A logical body of knowledge (e.g. `ctm`, `catalog`); not a permission and not a
Qdrant collection.
_Avoid_: corpus (for the logical unit), dataset

**Source**:
A declared origin of documents within a collection, with its kind and profiles.

**Capture**:
The exact bytes and envelope of one fetch, before any interpretation.
_Avoid_: download, snapshot

**Revision**:
One exact version of a document's bytes and metadata.

**Canonical document**:
The typed, replayable structure of a revision's Markdown, with original spans.

**Chunk**:
A token-budgeted, source-mapped passage prepared for representation.
_Avoid_: embedding, snippet

**Evidence bundle**:
The cited, verbatim passages a search returns, with provenance and signals.
_Avoid_: context, results

**Entity**:
A thing the knowledge graph names (a component, command, parameter, error
code…), resolved across its aliases.

**Relation**:
A typed link between entities that carries evidence spans and a validity range.
_Avoid_: edge (outside graph-engine code), fact

**Claim**:
An assertion extracted from a source with its evidence, conditions, validity and
review state, not yet or not necessarily admitted as knowledge.

**Hypothesis**:
A proposed, unestablished link or explanation, marked as such with its basis
and uncertainty; never presented as a fact.

**Quality outcome**:
The explicit disposition a document receives before indexing (`accepted`,
`accepted_with_warnings`, `needs_reextraction`, `quarantined`, `excluded`).

**Support group**:
The passages that together prove a multi-step conclusion; kept whole in an
evidence bundle or not used.

**Restore bundle**:
The single context payload (L0 identity, L1 working context, L2 recall) delivered
once per context generation at startup, resume or after compaction.
_Avoid_: summary, memory dump

**Checkpoint barrier**:
The durable save of an event cut that must succeed before context is compacted
or replaced.

**Continuity**:
The responsibility for authorized events, decisions, artifacts and unfinished
work surviving runs, delegation and context changes.

**Model card**:
The recorded identity and measured limits of a model filling a role (file
digest, template, server build, results).

**Bake-off**:
The recorded evaluation that selects the model for a role.

## Catalog and orchestration

**Catalog**:
The reviewed agents, skills, instructions, prompts, workflow graphs, contracts
and policies in `maestro-manifests`.
_Avoid_: manifest (for the whole catalog), registry

**Bundle**:
An immutable, attested release of the catalog, usable only while its freshness
record is valid and it is not revoked.

**Maturity**:
The evidence stage of a catalog resource: placeholder, authored, reviewed,
qualified, retired. Only qualified resources run.
_Avoid_: status (for this concept)

**Override class**:
The single class of a configurable setting: free, bounded, additive or locked.

**Discovery card**:
The searchable description of a workflow, agent or skill (summary, intents,
use-when, avoid-when, examples) indexed for routing.

**Project lock**:
The per-project record pinning bundle, runtime, SDK, model and sandbox profiles.

**Comparison pass**:
The one review, after the catalog written from zero is released, that reads an
earlier catalog against it; each item recovered enters by its own pull request.
_Avoid_: import, migration (for catalog content)

**Agent**:
A role definition (persona, responsibilities, tools, contracts) in the catalog.
_Avoid_: bot, assistant

**Skill**:
A reusable procedure an agent can load.

**Workflow graph**:
A reviewed graph of nodes and conditional edges that is the unit of execution.
_Avoid_: pipeline, chain

**Node**:
One step of a workflow graph: agent, step, gate, router, map, join or subgraph.

**Run**:
One execution of a workflow graph, recorded in the journal.
_Avoid_: job (reserved for kernel jobs), session

**Session**:
One model conversation executing an agent node.

**Handoff contract**:
The JSON Schema and semantic validators a node's result must satisfy.
_Avoid_: output format

**Acceptance**:
The host's verification that a submitted result satisfies its contract against
recorded evidence.

**Broker**:
The host component that decides, with Cedar policies, whether an operation may
happen.
_Avoid_: guard, firewall

**Provider profile**:
The provider and model a node may use (`copilot` or `llamacpp`), from the
bake-off results.

**Orchestrator**:
The Maestro agent that routes intents to workflow graphs and never executes work
itself.
_Avoid_: supervisor (reserved for dynamic planning), master agent

**Host**:
The program an agent or developer uses (Pi, Copilot CLI, VS Code), or the
`maestro` process when it enforces policy.

**Handoff**:
A node's accepted output passed to the next node, with the host-built envelope
(sender, recipient, scope, snapshot, digests, validated evidence).

**Qualification registry**:
The record of which model profile is qualified for which role, workflow and
hardware.

## Extensibility

**Command**:
A typed, versioned request to an application operation, with an authenticated
principal and an idempotency key; every entry point produces commands.
_Avoid_: call, action (for this concept)

**Entry point**:
A way a request reaches Maestro: CLI, MCP, local HTTP API, schedule, source
watcher, inbound webhook or extension.

**Event**:
A typed, versioned fact from the journal, in a CloudEvents envelope.
_Avoid_: message, notification

**Exit point**:
A consumer of the public event stream: a built-in projection, an extension, an
outbound webhook or a broker bridge.

**Subscription**:
A consumer's durable cursor over selected event types and scopes, with
acknowledgements and a dead-letter list.

**Extension**:
A declared, reviewed and separately activated integration that runs as a
sandboxed process with its own least-privilege principal.
_Avoid_: plugin (implies in-process code)

## Analysis and provenance

**Target**:
One system Maestro is authorized to analyse, pinned to an exact revision.
_Avoid_: competitor, subject

**Analyzer**:
An extension that leases an analysis job and submits findings about a target,
with their evidence, coverage, limits and method; the instrument behind it
(licence scanner, graph query runner, decompiler, traffic recorder) is
replaceable and never ships.

**Finding**:
One analyzer observation about a target revision: a claim like any other, with
the query or configuration that produced it.

**Surface**:
An externally observable entry point of a target: a command, route, tool,
library entry or file format. What an inventory counts.

**Behaviour contract**:
An executable description of what an authorized system does from the outside,
which becomes both the specification a reimplementation is written from and the
suite it is measured against.
_Avoid_: test plan, acceptance criteria (which are narrower)

**Clean-room boundary**:
The separation between the `analysis/<target>` scope and implementation work,
crossed only by an approved specification that quotes nothing.

**Provenance register**:
The record of every component Maestro adopts, ports or studies: origin, pinned
revision, licence, usage class, obligations and approver.
_Avoid_: inventory, SBOM (which are generated from it)

**Usage class**:
How a component entered our tree: `reused`, `modified`, `ported`,
`specification-only` or `independent`. The first three carry the original
licence's obligations.
