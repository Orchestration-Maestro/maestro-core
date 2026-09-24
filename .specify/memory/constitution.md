# Maestro Constitution

The rules every specification, plan, task, review and release in the Maestro
repositories must satisfy. The architecture lives in
[`docs/architecture/`](../../docs/architecture/README.md); the decisions in
[`docs/adr/`](../../docs/adr/README.md).

## Core Principles

### I. Evidence over assertion (NON-NEGOTIABLE)

A capability exists when a test, a CI gate or an evaluation proves it, and not
before. Reports never turn a failed, skipped or unavailable check into a pass;
missing is not zero; a green run over zero relevant tests proves nothing. Every
quality claim links to the run, report or artifact that supports it.

### II. Vertical slices, no scaffolding

Each slice delivers working, released software end to end. No empty crates, stub
commands, placeholder interfaces or frameworks ahead of need. A seam exists only
where two implementations vary across it. Only the active slice is specified in
detail.

### III. Test-first, deep modules

Changed behaviour starts with a failing test, then the smallest implementation
that passes, then refactoring. Modules expose small interfaces over substantial
behaviour and are tested through those interfaces. Denial and refusal tests must
prove their stimulus reached the real guard.

### IV. Provenance and immutability

Original bytes, captures, canonical documents, chunks, bundles and run artifacts
are immutable and content-addressed. Every derived record points to its exact
sources and to the profile that produced it. The kernel is the only authority;
indexes, graphs and caches are generation-stamped projections that can always be
rebuilt.

### V. The host decides (NON-NEGOTIABLE)

Policies, catalog prose, source documents, retrieved passages, model outputs and
tool outputs are data, never grants. Only the host broker, evaluating reviewed
policy against trusted facts, allows an effect; only host acceptance, checking
contracts against recorded evidence, completes a task. Default deny.

### VI. Rust first, exceptions visible

First-party code is Rust. A non-Rust component is allowed when it is external
(the model router's llama.cpp, a database server, the Copilot runtime) or when a
recorded exception names the missing capability, the alternatives considered,
its inputs and outputs, and its owner. Existing Python producers run until their
native replacement passes parity, source by source.

### VII. Local first, no silent degradation

Local models by default; the managed route is an explicit per-node profile. No
provider fallback, no truncation, no skipped gate, no hidden retry of a
non-idempotent effect. An unavailable dependency is a typed refusal with a
remediation.

### VIII. Measured, not assumed

Models, fusion weights, features and optimizations are chosen by recorded
evaluation on our own suites and data; performance work starts from a
measurement. No model, threshold or number is invented; an initial target is
labelled as such and revisited once measured.

## Quality Gates

- **CI:** every repository calls `Orchestration-Maestro/rust-workflows` pinned by
  commit (currently v1.2.1): formatting, Clippy pedantic with warnings denied,
  tests and doctests, line coverage ≥ 90 %, mutation testing on changed code,
  public API compatibility, unused dependencies, unsafe code denied, licence and
  advisory policy, secret scanning, SARIF, SBOMs and attested releases.
- **Source limits:** cognitive complexity ≤ 15; functions ≤ 100 lines and ≤ 5
  parameters; files ≤ 500 counted lines (reported above 300); Rust, shell and
  Just lines ≤ 100 columns; public and private items documented; no `unwrap`,
  `expect`, `panic!`, `todo!` or `dbg!` in production code.
- **Evaluations:** public synthetic suites gate pull requests; private suites
  run on the reference workstation and their reports accompany any change to
  retrieval, prompts, models or policies. A drop of more than 2 points on a
  gated retrieval metric, or any command-exactness failure, blocks the change.
- **Security:** policy rules ship with an allowed-neighbour test and a denied
  test; no secret, personal path or vendor-private material in public
  repositories.

## Development Workflow

- Pull requests only, squash merges, signed commits, conventional titles; one
  pull request per repository per working session, with a `feat` and a `fix`
  never sharing one.
- `just check` passes locally before any commit is pushed.
- Specs, plans and tasks follow Spec Kit (`specs/NNN-slug/`); the plan names
  exact files, interfaces and tests; tasks are small enough to review alone.
- Documentation tables are generated from their sources where a source exists;
  hand-maintained duplicates are not accepted.
- English for code, identifiers and repository prose.

## Governance

This constitution supersedes conflicting guidance in any other document. An
amendment is a pull request that changes this file, states the reason and the
migration of affected specs, and bumps the version: major for a removed or
redefined principle, minor for an added principle or gate, patch for wording.
Reviews check compliance; a justified exception is recorded in the plan's
complexity-tracking table with an owner and an expiry.

**Version**: 1.0.0 | **Ratified**: 2026-09-23 | **Last Amended**: 2026-09-23
