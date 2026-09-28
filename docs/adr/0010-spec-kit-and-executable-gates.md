# Spec Kit for delivery; evaluations and tests are the gates

Status: accepted, 2026-09-23; amended by
[ADR-0017](0017-spec-kit-installed-once-for-the-organization.md), 2026-09-24.

Each active slice gets a Spec Kit spec, plan and tasks under `specs/NNN-*/`,
governed by the constitution in `.specify/memory/constitution.md`. A slice is
accepted by passing tests, CI gates and evaluation thresholds, not by prose
describing qualification. Only the active slice is specified in detail, which
keeps plans from outgrowing delivery.

## Amendment, 2026-09-28

The owner's 08:12 decision, "Finish S1 in parallel and start S2 and S3", is a
scoped exception: S2 and S3 are active while S1 finishes. Their unfinished S1
prerequisites gate the affected live checks and exits, not fixture work.
Every other slice still starts from real prerequisite evidence and is specified
in detail only when active. Tests, CI and evaluation evidence remain the
acceptance gates for every slice.
