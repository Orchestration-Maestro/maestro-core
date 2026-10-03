# Catalog content and runtime live in separate repositories

Status: accepted, 2026-09-23.

`maestro-manifests` holds catalog content (agents, skills, workflows, contracts,
policies) and releases it as attested bundles; `maestro-core` holds the runtime
that verifies, installs and enforces them. Contributors change content without
touching runtime code, area owners or their delegated maintainers review
content, and a laptop
runs a released bundle without cloning either repository. The compiler and
validators exist once, in `maestro-core`; manifests CI runs the released binary.

Amended 2026-09-30: [ADR-0022](0022-manifest-layout-v4-and-language-neutral-extensions.md)
defines mandatory global standards, shared languages, framework and team areas.
Owners alone approve descriptors/delegation; generated ownership routes review
and trusted CI verifies it. Signed catalog releases and exact compatibility/pins
remain Phase 1 requirements; independent package publishers arrive in Phase 2.
Neither content ownership nor a generated index grants installation authority.
