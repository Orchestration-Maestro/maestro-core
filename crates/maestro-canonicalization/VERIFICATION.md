# Initial verification — 2026-09-21 (historical)

Current Phase A acceptance is recorded in [ACCEPTANCE.md](ACCEPTANCE.md): 43 tests plus a doctest, 86.25% line coverage, verified snapshots, and a measured 56-document sample. The figures and earlier blocking-gap behavior below are retained as historical evidence, not current acceptance claims.

Scope: the new standalone `canonicalization/` crate. Acquisition code and existing corpus bytes were not changed by this implementation. No files were staged or committed.

## Executed checks

| Check | Observed result |
| --- | --- |
| `just check` | Exit 0 using the local golden `rust-gate`; quality, coverage, MSRV, features, unused dependencies, licence/source/yank policy and audit. |
| Golden quality | Strict format/Clippy/rustdoc passed; nextest: **25 passed, 0 skipped**; **1 doctest passed**. |
| Golden coverage | **1,223 / 1,501 lines = 81.48%**; unchanged consumer threshold: 80%. |
| Golden dependency checks | No unused dependencies; advisories, bans, licences and sources passed. RustSec: 31 dependencies, zero vulnerabilities or informational warnings in the generated report. |
| MSRV | Crate compiled under the declared Rust 1.85 toolchain; primary pinned toolchain is 1.98.1. |

The golden feature-combination step was invoked; this crate declares no optional features, so there were no extra feature combinations. Active LSP probes did not provide clean confirmations; compiler/Clippy execution supplies the type-check evidence. The sole AST style suggestion was a let-chain incompatible with MSRV 1.85, so it was not applied.

Additional local evidence:

- Two separate release target directories produced identical 1,928,608-byte executables after final formatting. SHA-256: `f67209d2e5f4d0496a1a66e3698f40ca0df62248aac0ddc409a39ced509f3ccd`. This is a same-machine local check, **not** the golden hardened release/attestation job.
- A standalone Gitleaks scan of the module's non-generated files found no leaks. This is **not** the golden repository/history secret-scan workflow.
- Checked-in example JSON and its Markdown reference match actual CLI output byte-for-byte, and the deserialized example passes replay validation.
- New-file whitespace checks found no findings. Source and checked-in example output remain uncommitted.

## Real-corpus smoke test

Three deterministic samples per group (first, middle, last sorted filename):

| Group | Warning-only usable output | Failed output |
| --- | ---: | ---: |
| `bmc-web/core` | 3 | 0 |
| `bmc-web/community` | 2 | 1 |
| `bmc-web/support-knowledge` | 3 | 0 |
| `support-kb/support-kb` | 3 | 0 |
| `bmc-web/automation-api-ga` | 3 | 0 |

**All 15** inputs remained byte-identical; all 15 stored Markdown references matched their inputs; repeated CLI runs returned identical summaries and JSON bytes. The one failed document contains invalid original YAML frontmatter: it remains unchanged, its inspectable output is marked `failed`, and the CLI returns 2. No silent repair was attempted. These samples do not establish corpus-wide canonical validity.

Machine-readable local evidence is under ignored `target/`: `golden-development.log`, `golden-reports/`, `corpus-smoke-report.json`, `repro-report.json` and `gitleaks-module.json`. Corpus-derived snapshots under `target/corpus-smoke/` are not checked in.

## Independent review disposition

The independent review initially blocked approval on four reproduced findings. Each regression was observed failing before its correction, then passing:

| Finding | Correction and regression coverage |
| --- | --- |
| Altered JSON passed validation/storage | Retain pre-merge input metadata; deterministic replay compares the entire derived document. Seven mutations cover text, policy, source reference, revision, links, spans and findings; storage refuses each. |
| Duplicate YAML keys were overwritten | Use the strict YAML value mapping visitor before metadata conversion; reject duplicate root and nested policy keys. JSON sidecars now use the same strict visitor through the JSON deserializer. |
| Nested duplicate reference definitions disappeared | Validate container gaps as well as root spans; omitted definitions produce blocking `unparsed_content`. Recognized alert/definition-list/footnote markers are tested against false positives. |
| Compatible extraction details falsely conflicted | Merge explicit extraction first, then converter fallback; preserve extra details and continue rejecting contradictory converter values. |

Corrections were tested by the parent implementer; **the independent reviewer has not re-reviewed them and no updated approval is claimed**.

## Remaining golden-workflow scope

Mutation testing and the hosted golden secret-scan, hardened release, SBOM, CI/Sonar and publication gates have **not** been established by this work. `just check` is explicitly the local development tier, not a release approval. No thresholds or policies were weakened to obtain its result.

Hosted reusable workflow integration is pending a published, reviewed golden-workflow revision and the associated consumer provisioning. No fabricated SHA or floating workflow reference was added. Use `README.md` for local commands and the exact integration boundary.
