# Strict JSON for collection and source-policy declarations

Status: accepted, 2026-09-24. Retains the 2026-09-22 ingestion decision.

Collection declarations and executable source policies are JSON documents with
a strict, versioned schema (unknown or duplicate keys, dangling references,
invalid URLs, contradictory rules and non-finite budgets are rejected), because
the existing source-policy proposal and its tooling are JSON and a policy
language needs no templating or expressions. The review-only proposal version
(`maestro-ingestion-policy-proposal/1`, `draft_not_executable`) is refused by the
runtime parser; an executable policy has its own version and digest. Catalog
sidecars authored by people (model profiles, MCP servers, extensions) stay TOML,
and Copilot-native resources keep their own formats (ADR-0005).

## Consequences

`collection.json` replaces the earlier `collection.toml` sketch. Committed
declarations hold logical names only; machine paths, sessions and credentials
resolve from runtime bindings.
