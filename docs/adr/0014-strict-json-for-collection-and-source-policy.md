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

## Amendment, 2026-09-30 20:45: rules live in manifests

The knowledge-source kind owns URL rules beside
`knowledge/sources/<name>/source.toml`: strict JSON `maestro-source-policy/1`,
its denial/promotion decisions and expiry, and
`maestro-url-identity-migration/1` records. Collections retain strict
`collection.json` and exact source-rule references, not copied URL rules.
Maestro-core owns the types, parsers and engine, with zero real per-site rules.

Reviewed rule admission is the signed, digest-pinned catalog release plus the
owner and maintainer approvals recorded under protected ownership rules.
Policy fields cannot assert their own approval. Current trust, expiry,
revocation, collection ACLs and local acquisition/processing entitlement still
apply. S6 supplies the catalog-backed adapter behind its existing `PolicySource`
and `ResourceSource` ports; S3 check/compile/install performs no fetching.

Publish JSON Schemas generated from the core's `schemars` types in manifest
`schemas/`, with authored valid/invalid fixtures and four-checkpoint drift/
admission checks. Do not duplicate the validators or promise a generated schema
is runtime qualification. The migration type is an N07 synchronization
prerequisite after its review fixes land, not a delivered S3 implementation.
Private inventories, including Control-M URLs, live only in admitted private
manifest packages, never the public repository. C42's restricted collection
mount does not widen. [S3 D13](../../specs/003-catalog/plan.md#manifest-owned-source-rules)
defines the static contract; the S6 runtime adapter is separately tasked.
