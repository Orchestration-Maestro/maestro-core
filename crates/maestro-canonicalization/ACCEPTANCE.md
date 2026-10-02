# Phase A acceptance — canonicalization (historical, 2026-09-26)

## Decision

**CANONICALIZATION: PASS — implementation acceptance for the contract below.**

- **Private production results:** sample outcomes, corpus coverage and the
  acceptance receipt are retained in the private collection repository; no
  production counts or source inventory are published here.
- **DEDUPLICATION and CHUNKING: separate library evidence in [DEDUPLICATION.md](DEDUPLICATION.md) and [CHUNKING.md](CHUNKING.md).** This crate remains library-only; import, CLI and indexing live in `maestro-knowledge` and `maestro`. Task #19's native tokenizer qualification is historical; see [TOKENIZER.md](TOKENIZER.md) for the S1 parity contract. Neither result establishes whole S1 acceptance.
- **Separate baseline:** development tier passed. Full repository security, release/mutation/payload and hosted CI acceptance remain unestablished; see `../docs/RUST-BASELINE.md`. The user-retained token fixture was not edited.

This report covers the nine Phase A requirements supplied by the user. It does not establish PDF/OCR accuracy, source authenticity, authorization, retrieval quality or production-scale readiness.

## Supported contract

Completed UTF-8 Markdown is processed locally with pinned `pulldown-cmark =0.13.4`, recorded extension options, schema **1.2.0**, and parser profile **canonicalization/0.3.0+pulldown-cmark/0.13.4+source-accounting**. There is no document networking, inference, model download or source rewriting. This 2026-09-26 record was checked at the then-declared MSRV 1.85. The workspace now inherits MSRV 1.98 (2026-09-27); see the root [current verification instructions](../../README.md#-develop).

The current reader accepts snapshots from this schema/profile. Older artifacts are retained, never rewritten or silently migrated; compatibility with older readers/profiles is not claimed. Historical **source updates within this supported contract** remain separately retrievable. Inputs exceeding 128 parser nesting nodes receive an explicit refusal. Empty/body-free or conflicting/malformed metadata inputs have explicit blocked outcomes, rather than fabricated content or provenance.

Storage is application-immutable on a local Unix filesystem supporting hard links and directory synchronization, tested on Linux. The new pinned **rustix =1.1.5** dependency was already cached and was resolved offline. Safe directory-relative APIs prevent symlink/ancestor replacement from redirecting snapshot access; no unsafe Rust was added. Filesystem owners can still alter/delete artifacts, so replay and hashes do not replace trusted references or OS access controls.

## Criterion / evidence matrix

The test and coverage figures below are historical Phase A evidence, not current
workspace-gate results.

### Source integrity and identity

| Criterion | Executed evidence | Actual result | Limitation |
| --- | --- | --- | --- |
| **1. Original content is immutable** — exact original/copy bytes and hashes; separate derived text; invalid/unreadable inputs refused; original revisions retrievable | `cli_contract`: `cli_preserves_original_bytes_and_reuses_identical_artifacts`, `cli_emits_failed_validation_with_nonzero_exit_status`, `cli_rejects_unreadable_and_invalid_utf8_without_repair`, `revisions_and_partial_publications_remain_recoverable`, `verified_loader_refuses_tampered_and_incomplete_snapshots`; input byte/hash assertions | **PASS.** Accepted and failed originals compared before/after; changed revisions preserve old bytes; loader verifies fixed references and replay. | Unreadable-input tests use absent paths/directories; not a cross-platform ACL matrix. Original source ownership/ACLs remain external. |
| **2. Identity and revisioning** — logical source identity differs from equality; stable reruns; immutable updates; recorded configuration; operational runs outside identity | `document_contract`: `identical_text_from_different_sources_keeps_distinct_identity`, `repeated_processing_produces_identical_serialized_documents`; `cli_contract`: `operational_runs_do_not_change_source_identities`, revision recovery; `phase_a_acceptance`: `source_metadata_named_like_run_metadata_still_versions_provenance` | **PASS.** Same-content sources retain distinct IDs. Operational metadata changes artifact hashes, not document/revision/block IDs. Actual source metadata still changes provenance revisions. | Fallback local identity must remain stable or be explicitly overridden when paths move. No implicit source-ID migration. |

### Structure, provenance and meaning

| Criterion | Executed evidence | Actual result | Limitation |
| --- | --- | --- | --- |
| **3. Structure** — headings/levels/paths, repeated/skipped levels, heading-free prose, nested lists/quotes, code, tables, links/images, footnotes, explicit unsupported content | All `document_contract` structure tests; `phase_a_acceptance` heading-free, surplus-cell, duplicate-definition, heading-attribute and HTML-only regressions; `validation_boundary::recognized_container_markers_are_not_false_content_loss` | **PASS.** Resolved heading titles/IDs remain intact while omitted attribute syntax is retained raw. Surplus cells do not acquire invented headers. Nonempty HTML-only documents receive warnings, not false empty-document failures. | Raw/HTML retention is not semantic interpretation. Heading suffix fallback intentionally retains the complete unrepresented suffix, including presentation syntax. |
| **4. Provenance** — immutable revision references, half-open original UTF-8 spans, recoverable substrings, separate contributions, hierarchy-aware overlaps and supplied mappings | `unicode_spans_index_the_unchanged_markdown`, `supplied_extractor_structure_and_locations_are_retained`, literal source assertions in `phase_a_acceptance`, all 480 generated cases | **PASS.** Bounds/boundaries, parent containment and revision links checked. Disjoint inline contributions remain separate; broad container ranges may legitimately overlap children. Missing original pages/coordinates remain unavailable. | Spans index Markdown syntax, never normalized text. Supplied extractor coordinates are retained, not independently verified against a PDF. |
| **5. No silent loss or meaning change** — exhaustive byte accounting, markup distinct from content, explicit fallback findings, preserved negation/numbers/units/versions/identifiers/headers and independent source quotes | `every_original_byte_has_explicit_nonoverlapping_accounting`, `meaning_sensitive_text_and_source_quotes_are_separate`, `tables_preserve_headers_cells_alignment_and_values`, duplicate/surplus/heading regressions, `nul_replacement_is_visible_and_original_quotes_remain_exact`; generated marker-role checks and fixture byte accounting | **PASS.** Sampled source bytes partition completely; no unaccounted ranges. Meaning-sensitive literals are independently asserted, not compared only to the parser itself. NUL/replacement handling is visible through findings; original bytes remain exact. | A complete ledger does not prove earlier extraction accuracy or the semantics of unsupported syntax. Generated cases are bounded, not a proof over every possible Markdown input. |

### Trust and deterministic storage

| Criterion | Executed evidence | Actual result | Limitation |
| --- | --- | --- | --- |
| **6. Metadata, permissions and assets** — available provenance survives; missing values explicit; no public default/inferred permissions; missing/unsafe assets reported; instructions/links remain data | Policy/extractor/asset tests in `document_contract`; duplicate YAML/JSON tests; `cli_keeps_missing_and_outside_assets_visible`, `symlinks_and_traversal_cannot_escape_asset_or_snapshot_roots`, `embedded_instructions_are_data_not_permissions_execution_or_network_requests`; store ancestor-replacement/FIFO test | **PASS.** Local fake external asset remains untouched; unsafe destinations warn. Embedded shell text creates no file, local HTTP listener receives no connection, and policy remains null. Duplicate policy keys are rejected. | Asset files are observed, not read/copied. Availability can change after observation and never grants access. Consumers must enforce authorization and must not execute raw HTML. |
| **7. Versioned deterministic output** — explicit schema/profile, round-trips, stable ordering/IDs/findings, incomplete/corrupt snapshots rejected, failures cannot overwrite valid output | Repeated/round-trip tests; checked-in example comparison; source replay mutation tests; operational runs; partial publication recovery; eight concurrent identical writers; verified loader corruption tests | **PASS.** All accepted snapshot references resolve. JSON is the final synchronized publication; stale staging files are not artifacts. Existing unequal files are refused, not repaired in place. | A failed-but-consistent snapshot is intentionally loadable for inspection; callers must check `validation_status`. No claim of signed authenticity or crash-injection/power-loss certification. |

### Execution and completion

| Criterion | Executed evidence | Actual result | Limitation |
| --- | --- | --- | --- |
| **8. Tests and execution evidence** — requested fixtures, Unicode/CRLF, invalid inputs, updates, duplicates, round-trips, recovery, generated invariants and repository checks | Commands below; **43 tests + one doctest**; generated Cartesian product: 12 fragments × 10 wrappers × two line endings × two extension profiles = **480 cases** | **PASS.** All expected outcomes passed. Assertions identify the fragment/wrapper/profile case. Golden line coverage **1,531 / 1,775 = 86.25%**, above the unchanged 80% consumer floor. | No property-testing dependency or unbounded fuzzer was added. Linux execution only. |
| **9. Completion rule** — mandatory invariants and fixtures pass, critical known defects repaired, limitations explicit, reproducible evidence distinguishes implementation/document/corpus acceptance | This matrix, fresh command logs, current example, corpus manifest and red→green regressions | **PASS for Phase A.** No known unresolved critical defect in the supported canonicalization contract. Phase B was not started while Phase A was incomplete. | Independent review covered the parser/accounting seam, not a complete fresh storage security audit. Full repository baseline and hosted acceptance remain separate blockers. |

## Executed commands

The commands and timings below are the historical Phase A record from the
standalone crate checkout. Current package checks run from the workspace root;
see the [current verification instructions](../../README.md#-develop). Historical
logs and machine-readable outcomes were kept under ignored `target/acceptance/`.

| Exact command | Exit / elapsed | Artifact |
| --- | --- | --- |
| `cargo fmt --all --check` | 0 / 0.129 s | `fmt.log` |
| `cargo check --all-targets --locked --offline` | 0 / 0.240 s | `check.log` |
| `cargo clippy --all-targets --locked --offline -- -D warnings` | 0 / 1.708 s | `clippy.log` |
| `cargo test --locked --offline` | 0 / 1.453 s; 43 tests + one doctest | `tests.log` |
| `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --locked --offline` | 0 / 0.700 s | `rustdoc.log` |

The historical root `just check` run exited **0 in 20.483 s** (21 s printed). It ran hooks, hook-regression checks and the development tier: quality, coverage, MSRV, feature eligibility, unused dependencies, licenses and advisory audit. The crate declared no feature combinations, so that gate explicitly reported skipped; hosted-workflow hooks also skipped because workflow files were absent. The final rerun included the excessive-nesting refusal and documentation updates. Logs were local to the historical checkout.

A strict Clippy run first caught a needless test reference; it was fixed and rerun. The spelling hook then flagged the real POSIX `WRONLY` identifier; an exact identifier exception was added, not a broad exclusion. No security scanner rule was suppressed. Live LSP probes were inconclusive; fresh compiler/Clippy executions, not silent diagnostics, are the compilation evidence.

## Repairs and red→green evidence

| Repair | Before | After |
| --- | --- | --- |
| Operational envelope | `operational-red.log`: sidecar rejected the new field | CLI identity/provenance assertions pass; operations remain outside identities |
| Verified snapshot loading | `snapshot-red.log`: three loader/recovery tests failed against the initial unavailable loader | Immutable loader, corruption/path/refusal tests and recovery pass |
| Heading omissions / HTML-only rejection | `dialect-red.log`: both independent-review counterexamples failed | Exact raw retention without changed heading titles; warning-only HTML passes |
| NUL visibility | `nul-red.log`: no finding for source NUL | Explicit source-span finding, original quote unchanged |

Independent read-only review: workflow `df5a1b1c-9e5c-4936-b250-98b4535c96ce`, artifact `review/phase-a-source-audit.md`. Its initial verdict was **BLOCK**, with the two source-backed defects above. The parent reproduced both failures, repaired them and ran the passing regressions/full suite; this is not presented as an independent post-fix PASS. Pinned parser and rustix APIs were checked against locally cached upstream source.

## Private production evidence

The current product collection scope, counts and acceptance receipts are held in
the private collection repository. Public CI uses synthetic fixtures only; this
public repository publishes no production corpus inventory, contents, counts
or reports.

## Preservation and next boundary

The example JSON was regenerated through the real CLI. Its document/revision/content identities and supplied/merged provenance stayed equal; only derived contract output changed. Original `examples/input.md` and `examples/expected/original.md` remain byte-identical, SHA-256 `532551db335beb56018d9fcee91f87492a5d0f27b94bf78f0486e0c378217272`.

No project files were staged or committed; HEAD remained `2ffe556a782ab3a211767eb69f7d085829387fbf` on `work/canonical-acceptance-chunks`. Unrelated work, acquisition settings and captured fixtures were preserved.

Phase A permitted the separate tokenizer qualification recorded in [TOKENIZER.md](TOKENIZER.md). Scoped deduplication and chunking remain library responsibilities documented in [DEDUPLICATION.md](DEDUPLICATION.md) and [CHUNKING.md](CHUNKING.md). Import, CLI, indexing, retrieval and inference are owned by the higher-level `maestro-knowledge` and `maestro` crates; this crate does not infer permissions or perform those operations.
