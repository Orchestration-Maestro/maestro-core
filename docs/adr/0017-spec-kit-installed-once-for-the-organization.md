# Spec Kit is installed once for the organization, not committed

Status: accepted (owner decision, 2026-09-24). Amends
[ADR-0010](0010-spec-kit-and-executable-gates.md).

Spec Kit's templates, scripts, workflows and agent skills are tooling every
repository of the organization shares, so no repository commits them. They are
installed once, in a `.specify/` and an `.agents/skills/` beside the
organization's checkouts; each repository holds a `.specify` link to that
install, which git ignores, so Spec Kit still takes the repository as its
project root and writes `specs/NNN-*/` here. The constitution governs every
repository and lives in the organization's `.github` repository as
[`CONSTITUTION.md`](https://github.com/Orchestration-Maestro/.github/blob/main/CONSTITUTION.md), which the install's `memory/constitution.md` links
to. Specs, plans and tasks stay in the repository they deliver.

## Considered options

- Committed per repository, as S0 first did: every repository carries a copy of
  the same third-party scripts, which drift apart and fail the organization's
  line-length rule they were never written for.
- A single `.specify/` beside the checkouts and no link: Spec Kit takes the first
  directory holding `.specify/` as the project root, so specs would land outside
  every repository.
