# No model is preselected

Status: accepted, 2026-09-23.

Every model role (embedder, reranker, answerer, extractor, judge, agent roles)
is filled by the winner of a recorded bake-off on our own evaluation suites,
under hard constraints on latency, memory and licence. Models already served by
the router are candidates like any other. Winners are recorded as model cards
(file digest, template, server build, measured limits) and re-evaluated when a
candidate appears or a build changes.
