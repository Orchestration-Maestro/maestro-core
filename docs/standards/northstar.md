# Northstar for `maestro-core`

> Automate the guardrails to deliver faster, with higher quality, and more
> securely.

`maestro-core` steers by the organization's
[Northstar](https://github.com/Orchestration-Maestro/.github/blob/864d85597a833864cd8506c3925830503b3c2163/golden-rules/northstar.md):
one KPI per pillar, each with its measurement. Unmeasured is written `not
measured`, never estimated; a value read by hand carries the date it was read.

## The point

Maestro's runtime turns an organization's documents into source-backed evidence
that an agent can cite. Today it holds the first step: Markdown in; canonical
documents with byte-exact provenance, exact duplicates grouped and
token-budgeted chunks out, so that retrieval can later answer with what a
document actually says.

## KPIs

| Pillar | KPI | Current | Target | Measured by |
| --- | --- | --- | --- | --- |
| Speed | Duration of the required Rust CI on a pull request | not measured | set from the first baseline | The run time of `rust / Required Rust CI` |
| Quality | Surviving mutants | 0 | 0 | Mutation testing on every pull request's diff in CI; `just mutants` for the whole workspace |
| Maintainability | Clippy pedantic warnings | 0 | 0 | `just check` and CI deny every warning |
| Security | Open CodeQL alerts | 0 (read 2026-09-24) | 0 | Code scanning, CodeQL default setup |
