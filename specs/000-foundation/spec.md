# Feature Specification: Foundation

**Feature Branch**: `000-foundation`

**Created**: 2026-09-23

**Status**: Draft

**Input**: "Start fresh in our organization: keep only the canonicalization crate,
bring it to the organization's gates, and publish maestro-core and
maestro-model-router (public) and the Control-M collection (private)."
`maestro-manifests` is created in S3 with its first file (owner, 2026-09-24:
the catalog is written from zero).

**Plan**: [plan.md](plan.md) · **Tasks**: [tasks.md](tasks.md)

Architecture: [06 Roadmap, S0](../../docs/architecture/06-roadmap.md#s0-foundation),
[ADR-0001](../../docs/adr/0001-fresh-start-with-canonicalization-only.md),
[ADR-0009](../../docs/adr/0009-vendor-specific-material-stays-private.md).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A clean repository under the organization's gates (Priority: P1)

As the maintainer, I work in a `maestro-core` repository that contains only the
canonicalization crate plus the new architecture, and that passes the
organization's CI, so that new work starts on a verified base.

**Why this priority**: every later slice depends on a repository whose gates are
real. Measured on 2026-09-24: the crate does not inherit the workspace lints, and
with them it reports 244 Clippy findings and its integration tests do not
compile; four files exceed 500 counted lines; eleven functions exceed 100 lines
or complexity 15; the draft CI still calls Sonar.

**Independent Test**: `just check` passes locally and the `rust-workflows` CI
passes on the pull request that introduces the repository content.

**Acceptance Scenarios**:

1. **Given** the reset repository, **When** `just check` runs, **Then** formatting,
   Clippy (pedantic, warnings denied, workspace lints inherited), tests, doctests,
   coverage ≥ 90 %, dependency policy and secret scan all pass.
2. **Given** the canonicalization test suite from before the reset, **When** it runs
   after the refactoring, **Then** every test passes with unchanged expected
   outputs and fixture bytes.
3. **Given** the source limits, **When** the repository check runs, **Then** no file
   exceeds 500 counted lines, no function exceeds 100 lines, 5 parameters or
   complexity 15.

---

### User Story 2 - Nothing private in public repositories (Priority: P1)

As the maintainer, I publish repositories that contain no personal path, secret,
vendor-infrastructure reference or vendor-private material.

**Why this priority**: the repositories are public; the native tokenizer contract
currently embeds absolute home paths and the crate mentions a private artifact
registry.

**Independent Test**: gitleaks and a path check over the working tree and history
report nothing; the native tokenizer resolves its artifacts from a local binding
file outside the repository.

**Acceptance Scenarios**:

1. **Given** the committed tokenizer contract, **When** it is inspected, **Then** it
   holds artifact fingerprints and logical roles but no absolute path.
2. **Given** a local binding file named by `MAESTRO_NATIVE_BINDING`, **When** the
   native tests run explicitly, **Then** they resolve the artifacts from it and the
   counter loads its libraries from the bound directory; **When** the binding is
   missing, **Then** they refuse with an error naming the variable and the schema.
3. **Given** the repository, **When** searched for private registry names and home
   paths, **Then** no match exists.

---

### User Story 3 - The repositories S1 needs exist (Priority: P2)

As the maintainer, I find `maestro-core` and `maestro-model-router` (public; the
router was imported on 2026-09-24, pull request #1) under
`Orchestration-Maestro` with the organization's rulesets and CI, and
`ctm-collection` as a private repository outside it, because the organization's
rulesets allow only public repositories.

**Why this priority**: S1 lands in these repositories; the model router is a
runtime dependency of S1 and the collection holds its private inputs.

**Independent Test**: each repository's default branch has a merged, signed,
squash-merged pull request and a green required status.

**Acceptance Scenarios**:

1. **Given** `maestro-model-router`, imported on 2026-09-24, **When** it is checked,
   **Then** it is public, has `stack=rust`, pins rust-workflows v1.2.1 and its
   `main` is green.
2. **Given** `ctm-collection`, **When** created, **Then** it is private, holds the
   vendor-specific sources that leave the public tree, and no corpus content; the
   exporter lands in S1.

---

### User Story 4 - The process is in place (Priority: P3)

As a contributor, I find the constitution, the architecture, the ADRs, the
glossary and Spec Kit scaffolding in `maestro-core`, so that every slice follows
the same process.

**Independent Test**: `specify check` succeeds; the constitution renders; every
link in `docs/` resolves.

### Edge Cases

- The archive snapshot of the old repository fails or is incomplete → the reset
  does not start.
- Spec Kit's `init` would overwrite the existing constitution → scaffolding is
  generated in a temporary directory and only missing files are copied.
- A carried-over test depends on a machine path → it becomes an explicit native
  test with a binding, never a silently skipped one.
- The counter's executable names an absolute library path of its own → the
  invocation sets `LD_LIBRARY_PATH` to the bound directory, so the libraries
  that were verified are the ones that load.
- The import pull request's diff is the whole crate, 810 mutants, more than the
  CI job's 45 minutes → a full local run reaches zero survivors first; only that
  pull request runs CI without mutation testing, and the next one restores it.
- The organization refuses private repositories → the collection is created on
  the maintainer's account (default pending the owner's choice,
  [08 §17](../../docs/architecture/08-traceability.md#17-open-decisions)); the
  organization's settings stay as they are.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-S0-001**: The old repository content MUST be archived as a verified
  snapshot outside the organization directory before any removal, after the
  maintainer confirms.
- **FR-S0-002**: The repository MUST contain only the canonicalization crate as
  product code, renamed `crates/maestro-canonicalization` (the workspace's
  `maestro-<area>` names; identities unchanged), with its tests, fixtures and
  documentation, plus `crates/maestro-conventions`, whose tests hold the
  repository to its conventions.
- **FR-S0-003**: The workspace MUST define the organization's lints and every
  crate MUST inherit them.
- **FR-S0-004**: The canonicalization crate MUST keep its public API, identity
  recipes, schema and parser versions and fixture bytes.
- **FR-S0-005**: Files, functions and parameters MUST meet the source limits of the
  constitution.
- **FR-S0-006**: The native tokenizer MUST resolve artifact paths from a binding file
  outside the repository (`maestro-native-binding/1`, named by
  `MAESTRO_NATIVE_BINDING`, at most 1 MiB, relative paths resolved against its
  directory, unknown keys refused); the committed profile MUST hold only
  fingerprints, file names and roles; the counter MUST load its libraries from the
  bound directory.
- **FR-S0-007**: CI MUST call `rust-workflows` pinned by commit (v1.2.1) with coverage
  90, pedantic Clippy, unsafe denied, mutation testing, API compatibility, unused
  dependencies and SARIF enabled, and no Sonar input or secret.
- **FR-S0-008**: The repository MUST carry Dependabot, Scorecard, CODEOWNERS, the
  toolbelt (`mise.toml`, `mise.lock`), a justfile with `setup`, `check` and `docs`,
  and prek hooks for commits and messages; contributing, security and conduct
  come from the organization's defaults.
- **FR-S0-009**: `maestro-core` and `maestro-model-router` MUST exist in the organization
  with its rulesets; `ctm-collection` MUST be private.
- **FR-S0-010**: No public repository MAY contain secrets, personal paths or vendor-private
  material; a check MUST enforce it.
- **FR-S0-011**: The repository policies (counted lines per file, personal paths and
  private registries, lint inheritance, Markdown links and anchors) MUST be tests
  that CI runs.
- **FR-S0-012**: Before the import pull request, a full local mutation run MUST
  report zero surviving mutants; that pull request alone MAY run CI without
  mutation testing, recorded with its expiry in the plan.

### Key Entities

- **Archive snapshot**: compressed copy of the pre-reset repository with a digest
  manifest.
- **Native binding**: local file mapping logical artifact roles (model, counter,
  libraries) to paths.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-S0-001**: `maestro-core` CI is green on `main` with line coverage ≥ 90 %.
- **SC-S0-002**: 0 Clippy findings under the workspace lints (from 244).
- **SC-S0-003**: 0 files over 500 counted lines (from 4 today).
- **SC-S0-004**: 0 personal paths, secrets or private-registry references in the
  history of any public repository.
- **SC-S0-005**: 3 repositories exist; the 2 public ones have a green required
  status on `main`.
- **SC-S0-006**: 0 surviving mutants in a full local run over the crate (810 mutants).

## Assumptions

- The organization's rulesets, the release App and the `rust-workflows` v1.2.1
  release are available (they are, as of 2026-09-23).
- Historical coverage of the crate (reported 92 %) holds after refactoring; if not,
  tests are added rather than the floor lowered.
- The model router's uncommitted files are the maintainer's own work and may be
  published.
- The qualified tokenizer artifacts are present on the reference workstation
  (checked 2026-09-24: model, counter, 26 library files, sources); the four native
  tests pass there today (223 s).
