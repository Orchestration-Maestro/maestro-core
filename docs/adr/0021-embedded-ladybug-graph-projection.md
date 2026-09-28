# Embedded LadybugDB for the graph projection

Status: accepted, 2026-09-28. Owner direction recorded on 2026-09-27 at 21:27;
all remaining S2 recommendations approved on 2026-09-28 at 01:56.
Amends [ADR-0004](0004-neo4j-for-the-graph-projection.md).
This status accepts the direction, not technical qualification or delivery.
G01 reconciles the design; G24 alone finalizes this record against G25's actual
evidence and the M2 release checks.

## Decision

S2 uses **LadybugDB through the Rust `lbug` crate, embedded in process**, as
its graph traversal projection. The owner prefers an embedded graph before a
Neo4j service. The choice is approved; technical adoption still requires the
qualification defined by **G25**, first in the
[S2 task list](../../specs/002-knowledge-graph/tasks.md), but independent of the
SQLite pilot. S1 and that pilot can proceed while qualification runs.

SQLite in WAL mode plus the content-addressed artifact store remain the only
authority, as [ADR-0002](0002-one-authority-many-projections.md) requires.
Entities, claims, aliases, mentions, review records and verified source supports
live there. LadybugDB contains disposable, generation-stamped projections of
those records. Application collection, generation, entity and claim IDs are
keys; engine-internal IDs are never stable identities or evidence.

S2 uses one engine and no new daemon, network port, Docker or JVM. Package the
qualified native runtime without first-use downloads. The rule-only pilot may
use indexed SQLite one-hop neighbors; G11 replaces that temporary traversal
path with Cypher. SQLite keeps claims, export and evidence verification, not a
second recursive graph engine. A configured graph `none` is disabled with
zero calls, including opens/probes. A selected but absent, stale, locked or
rebuilding graph reports R4 `unavailable`; passage retrieval continues and
graph-dependent conclusions are refused when their proof is missing.

All lbug calls stay in knowledge's `graph/projection/` and `graph/cypher.rs`,
behind application-ID operations. G27 owns the public typed-edge API at
`crates/maestro-knowledge/src/graph/projection/port.rs`: slices write/read their
own authoritative edge families without raw Cypher or engine IDs. S3 C27a's
catalog dependency edges never become documentary evidence-span claims.
The supervisor accepts this seam for D07, which later wraps it in `GraphStore`
for backend choice. This does not add a second engine or backend trait to S2.

## Qualification and safety conditions

G25 pins the exact lbug version and the fewest features. It measures the added
crates, duplicate versions, native links, licences, build time and binary-size
delta under [ADR-0020](0020-rust-libraries-with-named-dependency-exceptions.md).
Apply the six-row pass bar in [plan A1](../../specs/002-knowledge-graph/plan.md):
bundled native source with no native-build network, no system OpenSSL, added
clean CI build ≤15 minutes and no unrelated-change liblbug rebuild, platform
evidence, licence compliance, and at most one forced duplicate. That duplicate
needs a named DEP-001 exception, forcing library, removal condition and vet
record. Measure real-cache shard/coverage cost: shards still fit 30 minutes,
local builds 8 GiB with three jobs. Binary delta/warm times are measured, not
given invented ceilings. No blanket exception or additional library approval.

Linux evidence plus a supervisor-approved dated Windows/macOS CI plan permits
implementation, not M2 acceptance. Actual native three-OS builds/tests,
C++20/CMake and supported compilers (GCC at least 13 on Linux), and both local
cross-Clippy recipes remain required. A supported feature/prebuilt recipe may
preserve a gate; native all-feature CI must still cover every shipped path.
A platform plan is never described as a passed test. Other missing evidence
fails/blocks; there is no waiver.

Test types, parameters, rollback, the single parameterized-batch loader,
per-hop filtered bounded paths and native cancellation. Test actual independent processes: a
writer while CLI and MCP readers use pinned generations, same-file locking,
immutable-generation reads, second-writer refusal and kill/reopen recovery.
The intended mode gives the writer unpublished owned files and readers
immutable published files; adopt it only if G25 proves it is supported. No
unsafe wrapper, bypassed lock or new coordinating service is permitted.

Claims/profile membership freeze per generation. Native writes happen outside
SQLite transactions; flush/close/reopen, schema, IDs, counts and digests are
verified before kernel readiness exposes a projection. Every hop is filtered
before selection/limits, and the kernel rechecks every result's claims,
entities, supports, eligibility and current grants before delivery. A graph
match or exact quote alone does not prove a relation is true.

Delete/rebuild equality is an M2 exit: close handles and drain readers, delete
only disposable graph files, rebuild from the kernel, and obtain identical
ordered neighbors, paths, claims, evidence and coverage, excluding only timings
and transport IDs. Retained generations and other collections survive. No
model rerun, Qdrant claim source or graph backup is needed.

## Considered options

- **Neo4j/neo4rs first (ADR-0004):** a separate service, not built in S2. A
  LadybugDB qualification failure needs a supervisor re-plan ruling before
  substitution, including driver qualification and a revised estimate.
  Separately, approved deployment-modes work adds a later user-selected
  external Neo4j adapter behind D07's graph port; that work does not require
  lbug to fail. Neither is a runtime fallback or dependency of S2's lbug path.
- **SQLite edges as the final graph:** sufficient for the thin pilot, but not
  the approved S2 traversal engine. Retaining both would duplicate path,
  authorization and recovery behavior.
- **Multiple graph adapters now:** defer backend selection to D07; S2 owns
  only the typed-edge application-ID port and one lbug implementation.

## Consequences

The graph needs native C++ build and packaging evidence and a supported
multi-process file-ownership model; G25 can block engine adoption, not the pilot.
An approved crate is not a qualified crate. If no supported safe mode passes,
stop for the fallback ruling rather than weaken isolation or platform gates.

Qdrant Server remains unchanged under ADR-0003. Qdrant Edge is evaluated in a
separate track; the longer-term no-external-servers goal does not qualify Edge
or add it to S2. PageRank, Personalized PageRank, Leiden, node similarity,
global/DRIFT search, fuzzy/vector linking and community summaries remain
deferred until measured gain justifies a later plan. The
[S2 specification](../../specs/002-knowledge-graph/spec.md) and
[plan](../../specs/002-knowledge-graph/plan.md) define the complete scope and
fixed quality gates.
