# Embedded LadybugDB for the graph projection

Status: accepted, 2026-09-28; amended 2026-09-28 (owner ruling 11:25/11:30).
Owner direction recorded on 2026-09-27 at 21:27;
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
[S2 task list](../../specs/002-knowledge-graph/tasks.md). Public claim/rule work
can proceed alongside qualification; pilot graph reads wait for it under the
2026-09-28 amendment below.

SQLite in WAL mode plus the content-addressed artifact store remain the only
authority, as [ADR-0002](0002-one-authority-many-projections.md) requires.
Entities, claims, aliases, mentions, review records and verified source supports
live there. LadybugDB contains disposable, generation-stamped projections of
those records. Application collection, generation, entity and claim IDs are
keys; engine-internal IDs are never stable identities or evidence.

S2 uses one engine and no new daemon, network port, Docker or JVM. Package the
qualified native runtime without first-use downloads. Under the 2026-09-28
amendment below, the rule-only pilot reads LadybugDB through G27/G28; G11
extends that engine with bounded paths. SQLite keeps claim authority/export
and evidence verification, never graph queries. A configured graph `none` is
disabled with zero calls, including opens/probes. A selected but absent, stale, locked or
rebuilding graph reports R4 `unavailable`; passage retrieval continues and
graph-dependent conclusions are refused when their proof is missing.

All lbug calls stay in knowledge's `graph/projection/` and `graph/cypher.rs`,
behind application-ID operations. G27 owns the public typed-edge API at
`crates/maestro-knowledge/src/graph/projection/port.rs`: slices write/read their
own authoritative edge families without raw Cypher or engine IDs. Its scoped
`entity_facts` read also returns literal-valued claim records on their subjects,
keyed by claim application ID with predicate, literal type/lexeme and generation;
these create no literal node or edge and never count toward path length. S3 C27a's
catalog dependency edges never become documentary evidence-span claims.
The supervisor accepts this seam for D07, which later wraps it in `GraphStore`
for backend choice. This does not add a second engine or backend trait to S2.

## Amendment (2026-09-28, owner ruling 11:25/11:30)

LadybugDB stays the graph engine from the pilot onward. SQLite holds only claim
authority and evidence checks, not neighbor/path queries. Pilot neighbors and
all paths read LadybugDB behind G27's projection port, using G28's single loader;
there is no temporary SQLite graph engine or postponed native graph delivery.
G04 follows G25/G28/G03, and G11 extends its reads with bounded Cypher paths.
G10 follows G02; G05 is a later private pilot-review checkpoint that gates
private acceptance, not public graph construction. This amends the earlier
SQLite-pilot sequencing without changing the embedded engine choice.

## Manifest v4 settings and lock handoff

The [S2 handoff](../../specs/002-knowledge-graph/plan.md#manifest-v4-settings-and-lock-handoff)
records S3 `30b702b` D13/D14 and C44/C46/C47a/C48, without finalizing native
qualification. `core/backends/graphdb/config.toml` declares
`type = "ladybug"`; the registered adapter maps it to `graph.engine`.
Retired engine resources are refused. Old `lbug` setting values require explicit
migration; they are not an alternative v4 contract or an invented alias.

C46 feeds backend defaults and `settings/defaults.toml` into one lowest S1
registry slot, one producer per key. Keep D14's three product-free settings
(`graphdb.buffer_pool_size`, `graphdb.max_db_size`, `graphdb.max_num_threads`)
and exact bounds/defaults from the handoff; validate inactive tables and masked
invalid values too. Rooted handles, read-only readers, writable writers and
checkpoint-on-close true remain locked; no manifest path or invented setter.
`none` makes zero calls; uncompiled selection refuses before native calls while
repair remains usable. Selected unavailable graphs never trigger fallback.

C47a hands frozen admitted defaults and the complete non-resource lock to the
existing S2 seam; bundles preserve checked configs, and changed config/lock
cannot replay. E07a/E08b/G28 still own actual consumer wiring. C46 owns the
unspecified migration/encoded-value details, C47a the concrete wire shape;
G01 invents neither. C48 requires actual G25/E07a qualified fork/lock/build/
feature metadata and approved owners (**OA1**, S3 plan:1932), source-only builds
and a disabled native extension installer. No new pin or approval is supplied.
G22's release/drill delta stays separate; G24 still finalizes this ADR.

## Qualification and safety conditions

G25 pins the exact lbug version and the fewest features. It measures the added
crates, duplicate versions, native links, licences, build time and binary-size
delta under [ADR-0020](0020-rust-libraries-with-named-dependency-exceptions.md).
Apply the six-row pass bar in [plan A1](../../specs/002-knowledge-graph/plan.md):
bundled native source with no native-build network, no system OpenSSL, added
clean CI build ≤25 minutes (raised from 15 by the owner on 2026-10-01) and no
unrelated-change liblbug rebuild, platform
evidence, licence compliance, and at most one forced duplicate. That duplicate
needs a named DEP-001 exception, forcing library, removal condition and vet
record. Measure real-cache shard/coverage cost: shards still fit 30 minutes,
local builds 8 GiB with three jobs. Binary delta/warm times are measured, not
given invented ceilings. No blanket exception or additional library approval.

Linux evidence, working gate-preserving local cross-Clippy recipes for both
targets and native Windows/macOS evidence or a supervisor-approved dated CI
plan permit implementation, not M2 acceptance. Actual native three-OS builds/
tests, C++20/CMake and supported compilers (GCC at least 13 on Linux) remain
required. A supported feature/prebuilt recipe may
preserve a gate; native all-feature CI must still cover every shipped path.
A platform plan is never described as a passed test. Other missing evidence
fails/blocks; no gate is weakened.

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

## The carried lbug fork

The pins and measurements below are historical G25 observations, not C48's
qualified declarations. D8 requires a freshly qualified combined rooted/cache/
source-default fork through E07a; default builds remain featureless and G22's
M2 release enables `engine`. This supersedes the earlier default-dependency
plan below, not the recorded measurements. G01 does not select a replacement pin.

G25 found that `lbug` 0.20.4 links system OpenSSL for its extension installer
alone, with no switch to drop it (bar item 2). Upstream's optional-OpenSSL pull
requests, LadybugDB/ladybug#777 and #796, closed unmerged. The owner chose on
2026-09-28 to keep LadybugDB and carry a small patch.

The patch lives in the organization's repository
[`Orchestration-Maestro/lbug`](https://github.com/Orchestration-Maestro/lbug):
the crates.io 0.20.4 crate imported unmodified, then the patch commits.
The workspace depends on the fork directly, as a git dependency at commit
`f91b5bb` with `version = "=0.20.4"` and default features off (`Cargo.lock`
pins the full hash). It was a `[patch.crates-io]` until 2026-10-03:
cargo-semver-checks builds each crate in a placeholder project that ignores
`[patch]`, so its API check built crates.io lbug instead of the fork. Its DEP-001 exception in
`maestro-quality.toml` allows that one git source, and `supply-chain`
records its vet exemption. The current pin includes E01/E02 rooted
filesystem operations, E03 external native-cache reuse, E03b source-only
builds, E01e strict rooted WAL replay, E07b's hash-index rollback fix and
PR #19's native cache key and built-in CMake Debug preset, plus PR #20's
source-checked bootstrap cfg reuse. Bootstrap-only cfgs added by
cargo-semver-checks are ignored only for source trees that never mention
`CARGO_CFG_`; the bundled tree qualifies, allowing API checks to reuse the
shared engine build. Earlier three-OS qualification and the `02d90e7` rollback
repin evidence remain recorded in
[S2 research](../../specs/002-knowledge-graph/research.md#e07b-repin-02d90e7);
the [native cache repin](../../specs/002-knowledge-graph/research.md#native-cache-repin-f91b5bb)
records the move to `f91b5bb`.

- **OpenSSL-free** (`575d94f`): a default Cargo feature,
  `extension_installer`, keeps upstream's behaviour. Without it, the CMake
  option `LBUG_EXTENSION_INSTALLER=OFF` skips `find_package(OpenSSL 3)`,
  `INSTALL` fails with a clear error instead of downloading over HTTPS or
  plain HTTP, and `build.rs` links neither `ssl` nor `crypto`. A static link
  also stops building the unused shared library.
- **CMake reuse** (`03c5460`, `4301d51`): with `LBUG_REUSE_CMAKE_BUILD`
  (set in `.cargo/config.toml`), the C++ build lives in one directory per
  target directory, profile and compiler settings, and a finished build is
  reused within that target directory whatever features, `RUSTFLAGS` or
  package selection Cargo builds the crate for: `-p` against `--workspace`,
  a feature check, or a CI cache restore with fresh source timestamps. A
  build into another target directory pays its own C++ build: coverage
  (`llvm-cov-target`), the gate's second release build for hardening
  (`rust-target-verify`), and a mutation run with its own target. The
  archive links `-bundle`, so the 2.6 GB debug archive is no longer copied
  into every rlib (7 GB of rustc memory), and a finished build drops its
  object files.

Historically, `.cargo/config.toml` pointed `CMAKE_TOOLCHAIN_FILE` at
`.cargo/lbug-debug-flags.cmake` to build the engine's CMake "Debug" type
without debug information (`-O0`; MSVC `/Ob0 /Od /RTC1` with `cl` named).
cmake-rs picks "Debug" for any Rust opt-level 0, whatever the profile's
`debug` says, so a profile override cannot drop `-g`. The external toolchain
file bypassed the fork's verified native cache and disabled cmake-rs's
cross-compile setup and compiler naming in every build type. The reused
CMake build did not see edits to that file without clearing its target cache.

At `8bb2f70`, the same Debug-only preset lives in the fork's source-hashed
`build.rs`; Release flags remain unchanged. The consumer toolchain assignment
and `.cargo/lbug-debug-flags.cmake` are retired. `LBUG_BUILD_FROM_SOURCE` and
`LBUG_REUSE_CMAKE_BUILD` remain set for source-only builds and target-local
reuse when no external cache is supplied. `maestro-quality.toml` opts into
`LBUG_NATIVE_CACHE_DIR` on Linux, keyed by `Cargo.lock` and publishing only
completed `entry-*` directories for the gate's verified cache transport.

E07a keeps native activation opt-in: `maestro-knowledge/engine` owns the
rooted adapter and `maestro/engine` forwards it. The remaining spike
installer/linkage probes also stay behind an off-by-default feature.
Default workspace builds have no lbug dependency. Required rust-central CI
at rust-workflows v4.8.0 consumes the exact native coverage/mutation policy;
`lbug-qualification.yml` adds SHA-bound three-OS native and default proof.
Root `clippy.toml` preserves the organization settings and forbids the old
path-only constructor. Windows rooted writers still refuse; E07a's reader
fixture alone uses one explicitly lint-expected legacy creation.

Historical unrooted spike operation tests and the `open_reopen` example
are retired in favor of product-rooted smoke; their measurements remain in
S2 research. This first boundary is not complete projection activation:
E07b/E08 own remaining operations, and frozen `graphdb.*` settings arrive
through the S1 resolver after S2 and S3 share a branch. G25's paired
cold/cache/coverage/mutation cost obligations still gate M2; no default
engine adoption or timing waiver is granted.

The fork's README says how to move to a new upstream version: import the new
crate, cherry-pick the patch commits, update the pin. When an upstream release
makes OpenSSL optional, drop the fork, its DEP-001 exception and its vet
exemption.

## Considered options

- **Neo4j/neo4rs first (ADR-0004):** a separate service, not built in S2. A
  LadybugDB qualification failure needs a supervisor re-plan ruling before
  substitution, including driver qualification and a revised estimate.
  Separately, approved deployment-modes work adds a later user-selected
  external Neo4j adapter behind D07's graph port; that work does not require
  lbug to fail. Neither is a runtime fallback or dependency of S2's lbug path.
- **SQLite graph queries:** rejected for the pilot as well as the final graph
  by the 2026-09-28 amendment above. A second traversal engine would duplicate
  path, authorization and recovery behavior.
- **Multiple graph adapters now:** defer backend selection to D07; S2 owns
  only the typed-edge application-ID port and one lbug implementation.

## Consequences

The graph needs native C++ build and packaging evidence and a supported
multi-process file-ownership model; G25 can block engine adoption, including
pilot graph reads under the 2026-09-28 amendment.
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
