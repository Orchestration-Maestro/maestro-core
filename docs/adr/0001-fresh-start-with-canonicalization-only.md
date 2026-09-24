# Fresh start: only the canonicalization crate is carried over

Status: accepted, 2026-09-23; amended 2026-09-24.

Earlier iterations produced a large body of plans and a small amount of product
code. We restart `maestro-core` with a fresh history and carry over only
`document-canonicalization`, the one component that is built, tested and
fixture-backed. The crate keeps its behaviour and identities; S0 only brings it
to the organization's lint and file-size gates.

**Amendment (2026-09-24):** fresh start applies to **code and history, not to
requirements**. Every requirement, decision and user choice recorded in the
organization's earlier material (the two design conversations, the fresh-core
and ingestion designs, the unified delivery plan and the provider analysis) is
traced in [08](../architecture/08-traceability.md) as kept, adapted, deferred or
dropped with a stated reason. Nothing is discarded silently. Repositories outside
the organization are not sources.

**Amendment (2026-09-24, catalog):** the fresh start covers the catalog's
content too. `maestro-manifests` is written from zero, from the requirements in
08; no file of an earlier catalog is imported, copied or opened while S3 is
written, so the new catalog shows what the requirements alone produce. After M3,
one comparison pass reads the earlier catalog, and each item worth recovering
enters by its own pull request
([03 §1.8](../architecture/03-agent-orchestration.md#18-writing-the-first-catalog)).

**Amendment (2026-09-24, name):** the crate is renamed `maestro-canonicalization`
in S0, following the workspace's `maestro-<area>` names. Its identities do not
change: they are built from fixed version strings (`PARSER_VERSION`,
`CHUNKER_VERSION`, `PREPARATION_PROFILE`), never from the crate's name.
