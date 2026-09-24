# Neo4j Community for the graph projection

Status: accepted, 2026-09-23.

The knowledge graph's facts live in the kernel; Neo4j 2026.x Community Edition,
a separate local service reached over Bolt through neo4rs, is the projection used
for traversal, paths and graph algorithms. It was chosen for its mature Cypher
and algorithm ecosystem and to pair with Qdrant as the team decided. Community
Edition has one user database, so projections carry a generation property that
every query binds.

## Considered options

- LadybugDB (maintained fork of the archived Kùzu): embedded, Cypher, no JVM;
  evaluated as an adapter in S2 and preferred for laptops if it passes the graph
  suite.
- SurrealDB, FalkorDB: rejected for now (maturity of graph algorithms; licence
  and Redis dependency respectively).
- SQLite edges + petgraph: kept as the fallback for small graphs and algorithms.

## Consequences

A JVM service on developer machines, and a driver (neo4rs 0.9) whose
compatibility with 2026.x must be qualified; Neo4j 5.26 LTS is the fallback.
