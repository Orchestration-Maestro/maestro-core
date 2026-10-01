# Manifest v4 separates global, framework and team content

Status: accepted, 2026-09-30 (owner approval; implementation remains planned).

## Decision

Use one pending `maestro-source/2` cutover, not an intermediate layout. Global
content comprises mandatory `standards/<domain>`, common resources and selected
`languages/<language>`; framework declarations live in `core/`; team packages
live in `capabilities/<group>/<name>`. Each area has one `package.toml`, with
nonempty owners and optional delegated maintainers. Groups confer no authority.
Common has prompt `contracts/`, but no agents or root instructions/policies.
Universal normative instructions and policies live in standards.

Use functional paths, filenames and IDs; products are values. One checker-owned
exception table permits only the three approved host directory names and exact
registered host/tool-required filenames/placements. A package cannot exempt
itself. Resources use `kind:namespace/local-name`; package, language, standard
and preset roots have their own qualified IDs, without language/package aliases.
Local names are unique per kind and namespace; area namespaces are globally
unique. Old capability paths/IDs and mixed layouts refuse with migration advice.

Common, core and every admitted standard are mandatory once in each installed
closure. Core holds the canonical Maestro persona and framework roles/workflows;
availability does not inject every persona into a knowledge-only native context.
Dependencies are explicit, never folder-order overrides: common/standards cannot
require core or optional teams; core cannot require teams; languages cannot
require teams. Team restrictions can add or narrow, never weaken standards.
Central, scoped, expiring exceptions require standard and root-owner approval;
non-negotiable rules have none. Ownership grants no runtime or publisher authority.

The editable standards authority remains singular. Phase 1 imports pinned
canonical organization/gate sources read-only with drift checks. Phase 2/ST1
freezes edits across a coordinated switch, makes manifest standards editable,
then renders verified-pinned organization and gate mirrors. No independent
manifest-standard edit precedes that switch.

Three core backend bases (`graphdb`, `vectordb`, `mcp`) and common defaults feed
one lowest defaults slot in the existing S1 registry. Package configurations
make only namespaced additions or registered narrowings; they cannot replace
types/endpoints or edit signed core. The effective MCP view is derived, not a
second server registry. Kernel model cards stay kernel-owned, declared under
`llm/models/<role>/`; install never registers or selects a model.

Extensions may be implemented in any language and are catalog declarations,
not in-process libraries. Maestro's own code remains Rust. Extend
[ADR-0013](0013-extensions-through-events-and-operations-out-of-process.md):
every action is an MCP tool call, while versioned engine events retain their
separate durable cursor/acknowledgment contract. Hook-event protocols belong to
the engine/checker, selected by point; they are not catalog contract resources
or dependency edges. Local runtime authority supplies executable/interpreter
bindings, scoped grants and secrets. No automatic runtime installation follows.

The owner's 20:45 amendment makes knowledge sources the sole home for URL
policy/decision/promotion/expiry/identity-migration JSON beside `source.toml`.
[ADR-0014](0014-strict-json-for-collection-and-source-policy.md) records the
formats, generated schemas and signed-release plus ownership-review admission.
Collections reference those rules; private URL inventories remain in admitted
private packages. Core has no real per-site rules. S6 alone supplies the runtime
catalog adapter through its existing policy/resource ports.

S3 validates and installs exact verified artifacts without launching content.
S4 owns sandboxing, launch, supervision, restart and durable delivery. C20's
live Copilot proof moves to S4 with its existing 4 h, not a saving. Ten-point
host mappings in S3 report mapped/unsupported/unqualified rather than claiming
live protection.

## Alternatives and consequences

- A type-first tree or another intermediate owner-only layout would repeat
  migration and split ownership. Reject both; register final placements once.
- Per-resource owner lists and handwritten schemas/indexes drift. Generate
  ownership/navigation/schemas from checked area and descriptor records;
  trusted base-code CI verifies real identities and base-owner exact-head review.
- Directory precedence or replaceable standards would turn content selection
  into authority. Reject collisions and widening instead; preserve explicit
  compatible pins, local trust, signed admission and current revocation checks.
- Rust-only extensions unnecessarily couple contributors to the runtime stack;
  in-process foreign code endangers authority. Language-neutral supervised
  processes preserve the existing replaceable adapter boundary.

This approval adopts no new library, grants no private-data access and proves
no runtime qualification. Minimal Phase 1 retains M3 safety; Phase 2 starts
immediately after M3. Full non-Rust gates, real renderer implementations, live
connectors and collection ingestion remain separate projects.

## Implementation contract

[S3 specification](../../specs/003-catalog/spec.md),
[plan D13–D15](../../specs/003-catalog/plan.md#d13-manifest-v4-source-contract)
and [task accounting](../../specs/003-catalog/tasks.md#critical-paths-and-effort)
carry exact fields, checkpoints, phase boundaries and refusal tests. Approved
source records: `manifest-design-final.md` §§2–8 and
`manifest-gap-analysis.md` §§3, 6–7, dated 2026-09-30. The supervisor corrected
G12 context accounting to Phase 2; Phase 1 gaps are G01 and G07–G11. ADR-0012's
separate content/runtime repositories and ADR-0015's signed freshness admission
remain unchanged.
