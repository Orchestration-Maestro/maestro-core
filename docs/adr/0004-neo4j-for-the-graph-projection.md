# Neo4j Community for the graph projection

Status: accepted, 2026-09-23; amended for S2 by
[ADR-0021](0021-embedded-ladybug-graph-projection.md), 2026-09-28. S2 chooses
embedded LadybugDB first, subject to G25 qualification, with SQLite still the
authority. A qualification failure needs a supervisor re-plan ruling before
substituting Neo4j in S2. Separately, approved deployment modes add a later
user-selected external Neo4j adapter behind the graph port. Neither is a
runtime fallback. G24 finalizes the qualification disposition against G25;
this amendment is not engine adoption or M2 evidence. The approved
[manifest v4 handoff](../../specs/002-knowledge-graph/plan.md#manifest-v4-settings-and-lock-handoff)
uses core backend configuration and the existing settings registry, not engine
resources; it neither adds a Neo4j adapter nor qualifies a pin.

## Historical decision

The following options, versions and fallbacks record the original decision,
not S2 implementation instructions. S2 has no SQLite or petgraph traversal;
its pilot reads LadybugDB (ADR-0021, amended 2026-09-28), and G11 extends it.

The knowledge graph's facts live in the kernel; Neo4j 2026.x Community Edition,
a separate local service reached over Bolt through neo4rs, is the projection used
for traversal, paths and graph algorithms. It was chosen for its mature Cypher
and algorithm ecosystem and to pair with Qdrant as the team decided. Community
Edition has one user database, so projections carry a generation property that
every query binds.

### Considered options

- LadybugDB (maintained fork of the archived Kùzu): embedded, Cypher, no JVM;
  evaluated as an adapter in S2 and preferred for laptops if it passes the graph
  suite.
- SurrealDB, FalkorDB: rejected for now (maturity of graph algorithms; licence
  and Redis dependency respectively).
- SQLite edges + petgraph: kept as the fallback for small graphs and algorithms.

### Consequences

A JVM service on developer machines, and a driver (neo4rs 0.9) whose
compatibility with 2026.x must be qualified; Neo4j 5.26 LTS is the fallback.
