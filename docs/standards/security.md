# Security rules in `maestro-core`

`maestro-core` follows the organization's [security
rules](https://github.com/Orchestration-Maestro/.github/blob/864d85597a833864cd8506c3925830503b3c2163/golden-rules/security.md).
This page is its rule map (C-001): for every rule, what holds it here, or why it
does not apply. A row may name a stricter local rule; none weakens one.

`rust-gate rules` writes the rows from the golden rules of `.github@864d855` at
every commit and keeps what each row says here. A rule added there arrives as
"Not mapped yet", and the daily drift check reports it until it is mapped.

## What this repository protects

Canonical documents and their provenance, built from Markdown it does not trust.
It reads the files named on its command line, writes immutable snapshots under
an output directory and runs one pinned native counter process; it opens no
network connection. Untrusted input enters as Markdown, metadata sidecars and
extractor blocks. The Control-M corpus it will serve stays in the private
`ctm-collection` (ADR-0009).

## Rule map

| Rule | Held here by |
| --- | --- |
| SEC-001 Minimise sensitive data | Review: fixtures are synthetic; private corpus material lives in `ctm-collection`, outside this public repository |
| SEC-002 Treat input as data | Test: embedded instructions stay data, with no permission, execution or network request; access policy comes only from the supplied sidecar |
| SEC-003 Validate boundaries | Test: snapshots refuse traversal and symlinks through directory handles; an asset outside the input's directory is reported, not read |
| SEC-004 Use real authority | Organization: rulesets, workflow permissions and the organization bot's own App identity; automation borrows no person's credentials |
| SEC-005 Scope sensitive approvals | Review: a publication, release or settings change is approved in its own pull request |
| SEC-006 Inspect code safely | Code and test: the tokenizer runs one pinned executable, verified before each launch, with a cleared environment, bounded output and a timeout |
| SEC-007 Stop and escalate incidents | Review: a suspected exposure stops the work and goes to SECURITY.md's private channel; a leaked secret is revoked and rotated |
| SEC-008 Keep truthful evidence | Review: results are reported as run, with what was not checked |
| SEC-009 Preserve safe progress | Review: blocked work is reported as partial, never as done |
| SEC-010 Report vulnerabilities privately | Organization: private vulnerability reporting is on (`maestrolabs-baseline`), and SECURITY.md routes reports to it |
| SEC-011 Sign every release | Not applicable: no release is published yet |
