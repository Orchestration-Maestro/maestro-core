# One authority, many projections

Status: accepted, 2026-09-23.

The kernel (SQLite in WAL mode plus a content-addressed artifact store) is the
only source of truth for scopes, the journal, documents, chunks, facts, catalog
installs and runs. Qdrant collections, Neo4j graphs and caches are
generation-stamped projections that can be deleted and rebuilt from the kernel at
any time. This satisfies the "one shared database" requirement of the
intelligence backend without making a vector or graph engine an authority, and
it keeps a laptop install to one file plus a directory to back up.

## Considered options

- Postgres (+ pgvector): stronger concurrency, but a server to operate on every
  laptop and no advantage at our write rates.
- SurrealDB 3 as a single multi-model store: attractive, but it would make the
  search engine the authority and couple every capability to one young engine.

## Consequences

SQLite allows one writer at a time: transactions stay short and never wrap
network or model calls. Every projection needs a rebuild path and a test that
proves it.
