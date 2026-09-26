# Qdrant server for vector and lexical projections

Status: accepted, 2026-09-23.

Dense and BM25 projections live in a local Qdrant 1.19 server, run as a systemd
user unit and accessed through qdrant-client 1.19. It provides hybrid queries,
server-side BM25 without a model to serve, payload filtering and aliases that
make generation switches atomic. Qdrant Edge (embedded, no daemon) is evaluated
before the laptop rollout because it would remove a service from every developer
machine; it is not adopted yet because it is young (0.8) and lacks server-side
BM25.

## Considered options

LanceDB (embedded) and tantivy + an in-process vector index were viable; Qdrant
was preferred for its mature hybrid query API and the team's explicit choice.

## Note, 2026-09-25

R7, in the S1 [research notes][r7], measured that server-side BM25 cannot hold
the French and English analyzer policy. Maestro computes the sparse vectors
with its `bm25-en-fr/1` analyzer; Qdrant keeps the sparse index, IDF weighting
and fusion. The decision stands, and Edge's missing server-side BM25 no longer
counts against it.

[r7]: ../../specs/001-knowledge-kernel/research.md#r7-server-side-bm25
