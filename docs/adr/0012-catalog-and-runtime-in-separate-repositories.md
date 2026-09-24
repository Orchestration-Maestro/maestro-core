# Catalog content and runtime live in separate repositories

Status: accepted, 2026-09-23.

`maestro-manifests` holds catalog content (agents, skills, workflows, contracts,
policies) and releases it as attested bundles; `maestro-core` holds the runtime
that verifies, installs and enforces them. Contributors change content without
touching runtime code, capability owners approve their sections, and a laptop
runs a released bundle without cloning either repository. The compiler and
validators exist once, in `maestro-core`; manifests CI runs the released binary.
