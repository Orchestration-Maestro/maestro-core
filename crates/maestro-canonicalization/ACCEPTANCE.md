# Phase A acceptance — canonicalization

## Decision

**CANONICALIZATION: PASS — implementation acceptance for the contract below.**

- **Document acceptance:** 53 sampled documents accepted with warnings; three blocked by metadata conflicts. A stored failed document is evidence, not accepted input to Phase B.
- **Corpus coverage:** 56 / 23,488 Markdown files processed twice (0.2384%); 23,432 not sampled. This is not full-corpus acceptance.
- **DEDUPLICATION and CHUNKING: separate library evidence in [DEDUPLICATION.md](DEDUPLICATION.md) and [CHUNKING.md](CHUNKING.md). Phase B CLI/manifests remain pending.** Task #19 qualified the pinned native GGUF counter and rejected the cached substitute; see [TOKENIZER.md](TOKENIZER.md). Neither result establishes whole Phase B compliance.
- **Separate baseline:** development tier passed. Full repository security, release/mutation/payload and hosted CI acceptance remain unestablished; see `../docs/RUST-BASELINE.md`. The user-retained token fixture was not edited.

This report covers the nine Phase A requirements supplied by the user. It does not establish PDF/OCR accuracy, source authenticity, authorization, retrieval quality or production-scale readiness.

## Supported contract

Completed UTF-8 Markdown is processed locally with pinned `pulldown-cmark =0.13.4`, recorded extension options, schema **1.2.0**, and parser profile **canonicalization/0.3.0+pulldown-cmark/0.13.4+source-accounting**. There is no document networking, inference, model download or source rewriting. Rust is pinned to 1.98.1; the declared MSRV is 1.85 and was checked by the golden gate.

The current reader accepts snapshots from this schema/profile. Older artifacts are retained, never rewritten or silently migrated; compatibility with older readers/profiles is not claimed. Historical **source updates within this supported contract** remain separately retrievable. Inputs exceeding 128 parser nesting nodes receive an explicit refusal. Empty/body-free or conflicting/malformed metadata inputs have explicit blocked outcomes, rather than fabricated content or provenance.

Storage is application-immutable on a local Unix filesystem supporting hard links and directory synchronization, tested on Linux. The new pinned **rustix =1.1.5** dependency was already cached and was resolved offline. Safe directory-relative APIs prevent symlink/ancestor replacement from redirecting snapshot access; no unsafe Rust was added. Filesystem owners can still alter/delete artifacts, so replay and hashes do not replace trusted references or OS access controls.

## Criterion / evidence matrix

### Source integrity and identity

| Criterion | Executed evidence | Actual result | Limitation |
| --- | --- | --- | --- |
| **1. Original content is immutable** — exact original/copy bytes and hashes; separate derived text; invalid/unreadable inputs refused; original revisions retrievable | `cli_contract`: `cli_preserves_original_bytes_and_reuses_identical_artifacts`, `cli_emits_failed_validation_with_nonzero_exit_status`, `cli_rejects_unreadable_and_invalid_utf8_without_repair`, `revisions_and_partial_publications_remain_recoverable`, `verified_loader_refuses_tampered_and_incomplete_snapshots`; corpus byte/hash assertions | **PASS.** Accepted and failed originals compared before/after; changed revisions preserve old bytes; loader verifies fixed references and replay. | Unreadable-input tests use absent paths/directories; not a cross-platform ACL matrix. Original source ownership/ACLs remain external. |
| **2. Identity and revisioning** — logical source identity differs from equality; stable reruns; immutable updates; recorded configuration; operational runs outside identity | `document_contract`: `identical_text_from_different_sources_keeps_distinct_identity`, `repeated_processing_produces_identical_serialized_documents`; `cli_contract`: `operational_runs_do_not_change_source_identities`, revision recovery; `acceptance`: `source_metadata_named_like_run_metadata_still_versions_provenance` | **PASS.** Same-content sources retain distinct IDs. Operational metadata changes artifact hashes, not document/revision/block IDs. Actual source metadata still changes provenance revisions. | Fallback local identity must remain stable or be explicitly overridden when paths move. No implicit source-ID migration. |

### Structure, provenance and meaning

| Criterion | Executed evidence | Actual result | Limitation |
| --- | --- | --- | --- |
| **3. Structure** — headings/levels/paths, repeated/skipped levels, heading-free prose, nested lists/quotes, code, tables, links/images, footnotes, explicit unsupported content | All `document_contract` structure tests; `acceptance` heading-free, surplus-cell, duplicate-definition, heading-attribute and HTML-only regressions; `validation_boundary::recognized_container_markers_are_not_false_content_loss` | **PASS.** Resolved heading titles/IDs remain intact while omitted attribute syntax is retained raw. Surplus cells do not acquire invented headers. Nonempty HTML-only documents receive warnings, not false empty-document failures. | Raw/HTML retention is not semantic interpretation. Heading suffix fallback intentionally retains the complete unrepresented suffix, including presentation syntax. |
| **4. Provenance** — immutable revision references, half-open original UTF-8 spans, recoverable substrings, separate contributions, hierarchy-aware overlaps and supplied mappings | `unicode_spans_index_the_unchanged_markdown`, `supplied_extractor_structure_and_locations_are_retained`, literal source assertions in `acceptance`, all 480 generated cases | **PASS.** Bounds/boundaries, parent containment and revision links checked. Disjoint inline contributions remain separate; broad container ranges may legitimately overlap children. Missing original pages/coordinates remain unavailable. | Spans index Markdown syntax, never normalized text. Supplied extractor coordinates are retained, not independently verified against a PDF. |
| **5. No silent loss or meaning change** — exhaustive byte accounting, markup distinct from content, explicit fallback findings, preserved negation/numbers/units/versions/identifiers/headers and independent source quotes | `every_original_byte_has_explicit_nonoverlapping_accounting`, `meaning_sensitive_text_and_source_quotes_are_separate`, `tables_preserve_headers_cells_alignment_and_values`, duplicate/surplus/heading regressions, `nul_replacement_is_visible_and_original_quotes_remain_exact`; generated marker-role checks and corpus accounting | **PASS.** Sampled source bytes partition completely; no unaccounted ranges. Meaning-sensitive literals are independently asserted, not compared only to the parser itself. NUL/replacement handling is visible through findings; original bytes remain exact. | A complete ledger does not prove earlier extraction accuracy or the semantics of unsupported syntax. Generated cases are bounded, not a proof over every possible Markdown input. |

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

Run the first five commands from `canonicalization/`. Logs and machine-readable command outcomes are under ignored `target/acceptance/`; the wrapper captured real exit codes, not pipeline-tail status.

| Exact command | Exit / elapsed | Artifact |
| --- | --- | --- |
| `cargo fmt --all --check` | 0 / 0.129 s | `fmt.log` |
| `cargo check --all-targets --locked --offline` | 0 / 0.240 s | `check.log` |
| `cargo clippy --all-targets --locked --offline -- -D warnings` | 0 / 1.708 s | `clippy.log` |
| `cargo test --locked --offline` | 0 / 1.453 s; 43 tests + one doctest | `tests.log` |
| `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --locked --offline` | 0 / 0.700 s | `rustdoc.log` |

From the repository root, `PATH="$PWD/.tools/bin:$PATH" just check` exited **0 in 20.483 s** (21 s printed). This ran hooks, hook/wrapper regression checks and the actual golden development tier: quality, coverage, MSRV, feature eligibility, unused dependencies, licenses and advisory audit. The crate declares no feature combinations, so that gate explicitly reported skipped. Hosted-workflow hooks likewise skipped absent workflow files. Final rerun includes the excessive-nesting refusal and all documentation updates. Logs: `target/acceptance/development-final.log`, `target/acceptance/commands.json`, `target/golden-reports/coverage.lcov`.

A strict Clippy run first caught a needless test reference; it was fixed and rerun. The spelling hook then flagged the real POSIX `WRONLY` identifier; an exact identifier exception was added, not a broad exclusion. No security scanner rule was suppressed. Live LSP probes were inconclusive; fresh compiler/Clippy executions, not silent diagnostics, are the compilation evidence.

## Repairs and red→green evidence

| Repair | Before | After |
| --- | --- | --- |
| Operational envelope | `operational-red.log`: sidecar rejected the new field | CLI identity/provenance assertions pass; operations remain outside identities |
| Verified snapshot loading | `snapshot-red.log`: three loader/recovery tests failed against the initial unavailable loader | Immutable loader, corruption/path/refusal tests and recovery pass |
| Heading omissions / HTML-only rejection | `dialect-red.log`: both independent-review counterexamples failed | Exact raw retention without changed heading titles; warning-only HTML passes |
| NUL visibility | `nul-red.log`: no finding for source NUL | Explicit source-span finding, original quote unchanged |

Independent read-only review: workflow `df5a1b1c-9e5c-4936-b250-98b4535c96ce`, artifact `review/phase-a-source-audit.md`. Its initial verdict was **BLOCK**, with the two source-backed defects above. The parent reproduced both failures, repaired them and ran the passing regressions/full suite; this is not presented as an independent post-fix PASS. Pinned parser and rustix APIs were checked against locally cached upstream source.

## Measured corpus workload

Corpus sampling runs in the private collection repository, next to the corpus it reads.

The final measured sampler invocation exited **0**. Selection is deterministic: top-level source groups, sorted by size then relative path, up to ten equally spaced quantiles including smallest/largest. There were zero skipped symlink files in the census. No sidecars or permissions were fabricated.

| Source group | Census | Sampled | Accepted with warnings | Blocked |
| --- | ---: | ---: | ---: | ---: |
| Root document | 1 | 1 | 1 | 0 |
| BMC attachment/log4j | 5 | 5 | 5 | 0 |
| BMC web | 16,482 | 10 | 10 | 0 |
| Community BMC | 922 | 10 | 10 | 0 |
| GitHub Control-M | 815 | 10 | 10 | 0 |
| Internal | 17 | 10 | 7 | 3 |
| Support KB | 5,246 | 10 | 10 | 0 |
| **Total** | **23,488** | **56** | **53** | **3** |

The three internal inputs have blocking **`metadata_conflict`** findings. Their exact paths, source hashes, failed snapshots and error codes are in the local manifest; no captured text or metadata values are copied here. They remain excluded from accepted downstream inputs. Accepted fraction of the sample: **94.64%**. Verified accepted fraction of the census: **0.2256%**, not an estimate of unsampled quality.

- **Volume:** 1,625,434 unique source bytes; two CLI passes per selected input. Original/copy bytes and hashes matched; repeated summaries and serialized artifacts were identical.
- **Time:** **21.856 s**, including selection and two passes; existing immutable snapshot files were reused on this final run. Debug CLI build, local Linux filesystem. This is not a cold-release throughput benchmark.
- **Peak RSS:** CLI **519,756 KiB (507.57 MiB)**; Python verification driver **727,952 KiB (710.89 MiB)**. Measured separately using fresh GNU time processes and Linux `VmHWM`, not summed or extrapolated. Initial inherited `getrusage` measurements were replaced by this corrected method.
- **Accounting:** 840,025 parsed-content bytes, 217,534 structural bytes, 566,878 metadata/reference bytes, 997 explicitly unsupported bytes: **1,625,434 / 1,625,434 accounted**, including retained failed inputs. No unaccounted ranges. This is byte-accounting coverage, not semantic extraction accuracy.
- **Warnings:** 2,084 missing-asset findings, 135 missing-metadata findings, 55 raw-HTML findings, 37 outside-root asset findings and 27 raw-fallback findings. These are finding counts, not distinct-document counts.

Memory is material: parsing/replay and JSON verification hold whole documents and derived structures in memory. There is no streaming/RAM-bound guarantee, invented performance threshold or production-scale readiness claim.

Local evidence: `target/acceptance/corpus/report.json`, `target/acceptance/corpus-final.log`, immutable snapshots under `target/acceptance/corpus/snapshots/`. The report pins the measured CLI SHA-256 `3576650077573a55eb74673064c1ce83dcf216036f429a34232d842e3d526d80` and sampler SHA-256 `7bfa0fe7675f6f0467d07f71159afb8516b2a06986f9cf5d0fd2497274018206`. These are evidence fingerprints, not distribution/hosting claims. Do not publish snapshots or path-bearing reports without review.

## Preservation and next boundary

The example JSON was regenerated through the real CLI. Its document/revision/content identities and supplied/merged provenance stayed equal; only derived contract output changed. Original `examples/input.md` and `examples/expected/original.md` remain byte-identical, SHA-256 `532551db335beb56018d9fcee91f87492a5d0f27b94bf78f0486e0c378217272`.

No project files were staged or committed; HEAD remained `2ffe556a782ab3a211767eb69f7d085829387fbf` on `work/canonical-acceptance-chunks`. Unrelated work, acquisition settings and captured fixtures were preserved.

Phase A permitted the agreed Phase B tokenizer qualification work, subsequently completed in [TOKENIZER.md](TOKENIZER.md) using the authoritative vocabulary-only counter. Scoped deduplication is now covered by [its separate library report](DEDUPLICATION.md); the [chunking library](CHUNKING.md) has separate evidence and Phase B CLI/manifests remain next. No approximate token counts, silent model downloads, embeddings, BM25 indexing or graph/inference work are authorized by this acceptance.
