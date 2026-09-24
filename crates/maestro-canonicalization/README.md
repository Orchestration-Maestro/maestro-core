# Document canonicalization

Local Rust library and CLI: **completed Markdown + supplied metadata → parsed structure → validation → `CanonicalDocument`**. This crate is independent of `orchr/` and does not change harvesting.

No document networking, model inference, crawling, OCR, embeddings, graph extraction, indexing, retrieval, ranking or answers. Build-time dependency/security checks are separate and may need network access.

Phase B library APIs provide [scoped exact grouping](DEDUPLICATION.md) and [mapped structural chunks](CHUNKING.md), preserving every source occurrence. Chunking invokes the qualified local vocabulary-only counter described in [TOKENIZER.md](TOKENIZER.md) and the pinned [counter contract](tokenizer-contract.json). The Phase A CLI remains unchanged; Phase B CLI/manifests remain pending.

## 1. Run the example

From this directory:

```sh
cargo test --locked --offline
just example
```

`--offline` assumes the locked dependencies are already cached; otherwise fetch them once with `cargo fetch --locked`. Rust is pinned in `rust-toolchain.toml`.

The CLI prints a JSON summary containing the output path. Each immutable artifact directory contains `original.md` and `canonical.json`:

```sh
cargo run --locked --offline -- /path/to/completed.md \
  --metadata /path/to/metadata.json --output /path/to/canonical-output
```

`--metadata` is optional. `--document-id` supplies an authoritative stable identity; `--identity-key` supplies a stable local fallback instead of the input pathname. Conflicting CLI and sidecar IDs are rejected.

The reproducible fixture is `examples/input.md` + `examples/metadata.json`. The full expected JSON and byte-identical Markdown copy are under `examples/expected/`. An integration test compares actual CLI output to both checked-in files and revalidates the deserialized JSON. Its one expected warning is that **access policy is unknown**, not public.

## 2. Use the library

```rust
use maestro_canonicalization::{canonicalize, CanonicalizeInput, ValidationStatus};

let markdown = "# Checks\n\nDo not restart before waiting 20 ms.\n";
let mut input = CanonicalizeInput::new(markdown, "stable-local-key");
input.document_id = Some("source-system:document-42");
let document = canonicalize(input)?;
assert_ne!(document.validation_status, ValidationStatus::Failed);
# Ok::<(), maestro_canonicalization::Error>(())
```

`canonicalize` is pure and never reads files. `CanonicalizeInput` also accepts `SourceMetadata`, existing `ExtractorBlock` values, `ParserOptions`, an operational metadata map, and an explicit asset-availability snapshot. `save_document` checks deterministic replay before storing JSON; `validate_document` replays against the exact Markdown and retained inputs, rather than trusting a saved status flag. `load_document(path)` returns the verified document and original Markdown, or an error. It verifies fixed filenames, hashed artifact/identity directories, exact JSON serialization, source hashes and replay. It accepts the current schema/parser profile only; old artifacts are never overwritten or silently migrated.

### Contract and identities

| Field | Meaning |
| --- | --- |
| `document_id` | Explicit ID, else a namespaced SHA-256 of the supplied source reference, else the stable local key. Never content equality. |
| `revision_id` | SHA-256 over identity, exact Markdown hash, supplied and merged metadata, and extractor blocks. |
| `content_hash`, `original_markdown_reference` | SHA-256 and byte length of the **unchanged** Markdown; stored reference is `original.md`. |
| `parser_version`, `parser_options`, `schema_version` | Explicit derivation/schema versions. Block IDs include revision, parser configuration and source order. |
| `source_metadata`, `input_metadata`, `source_reference`, `access_policy` | Retained inputs and merged provenance. Missing information is null; no permission or extraction history is guessed. |
| `operational_metadata` | Explicit run IDs/timestamps, excluded from document/revision/block identities. They still change the serialized artifact hash. |

The optional sidecar field `"operational_metadata": {"run_id": "local-run-42", "processed_at": "2026-09-21T00:00:00Z"}` records caller-supplied operations. Nothing reads a clock or automatically strips similarly named **source** metadata. Actual source metadata continues to version provenance.

`blocks` is an ordered arena of typed natural blocks, not retrieval chunks. Each block has its ID/revision, parent block, parent section, heading path, original spans, readable derived text, typed structure, asset references and extractor mappings. Ordered child nodes preserve interleaved inline text and nested blocks. `sections` records actual heading levels and distinct IDs, including repeated headings; nested container headings do not change the enclosing section after the container ends. `links` indexes source-positioned links/images without turning them into a graph.

The parser uses `pulldown-cmark::Parser::into_offset_iter()`. Spans are **half-open UTF-8 byte offsets into the entire original Markdown**, including frontmatter and CRLF. They are syntax ranges, not offsets into normalized text. Source bytes are authoritative for quotations. Derived text preserves numbers, units, versions and negation; smart punctuation is disabled. Code retains indentation, fenced/indented style and supplied language/info strings. Tables retain header/body cells, order and alignment. Parser-omitted surplus cells, duplicate definitions and heading-attribute suffixes are retained as exact `Raw` blocks with `source_fallback` warnings, not assigned invented headers or heading titles. Nonempty HTML-only documents are accepted with `raw_html` warnings; HTML is never interpreted.

`source_accounting` partitions every original byte into parsed content, structural syntax, metadata/reference definitions, unsupported raw source, or blocking unaccounted content. It is separate from legitimately overlapping structural spans. Raw retention does not claim semantic understanding of unsupported syntax. NUL/replacement characters and extraction markers receive explicit findings; source quotations always use the original bytes.

Original frontmatter values are retained in `source_metadata.extra.markdown_frontmatter`; arbitrary sidecar metadata belongs in `source_metadata.extra`. Duplicate YAML and JSON mapping keys are rejected. No malformed frontmatter repairs are made. Explicit extraction details are merged before the `converter` fallback; genuine conflicts fail.

Existing extractor JSON, anchors and original locations remain in `extractor_blocks`. Their opaque semantics cannot be independently proven here and produce a warning. Valid overlapping Markdown anchors populate each block's extractor IDs. Missing PDF pages remain unknown; none are invented.

### Storage and assets

Artifact paths are hashes of document ID, revision ID and serialized artifact, so caller IDs cannot become path traversal. Different parser options, operational metadata or asset observations can produce distinct artifacts for one source revision. Storage targets a local Unix filesystem with hard links and directory `fsync` (tested on Linux). The pinned, locally cached `rustix` dependency supplies safe directory-relative operations: each path component and artifact is opened without following symlinks. Captured directory handles prevent ancestor replacement from redirecting reads/writes. Nonregular files are refused; FIFO opens cannot hang the loader.

Publication uses synced temporary files and no-overwrite hard links, with directory entries synced and JSON published last. An unequal existing file, symlink or parent traversal is refused. Retry fills absent files only after verifying existing bytes. A missing/corrupt component makes `load_document` fail; it never silently repairs corruption. Orphan staging files are not accepted snapshots. After inspection, an operator may move a corrupt artifact aside and rerun from retained inputs; no automatic deletion is performed.

Storage is application-immutable, not a filesystem ACL or signed archive. Use a locally owned output directory. A hostile owner can still delete or mutate files, move directories or replace all inputs/hashes; trusted references and OS access controls remain necessary.

The CLI observes local assets within the input Markdown's directory; remote assets are never fetched. It records missing/unchecked/outside-root assets without pretending they exist. It does **not** copy asset files into snapshots. Availability is an observation at canonicalization time, not permission, current availability, or a promise that relative links resolve from the output directory.

## 3. Interpret validation

| Result | CLI exit | Meaning |
| --- | --- | --- |
| `valid` | 0 | All implemented checks pass, with no findings. |
| `valid_with_warnings` | 0 | Structurally usable, but inspect the limitations. Unknown policy still grants no access. |
| `failed` | 2 | JSON and Markdown remain inspectable; do not use as trusted canonical content. |
| Input/execution refusal | 1 | Invalid UTF-8/sidecar, unsafe nesting, conflicting IDs or storage failure; no successful artifact is claimed. |

Findings carry severity, code, message, optional block ID and original byte spans where available. Checks cover provenance/hash consistency, UTF-8 bounds, containment/hierarchy, code/table fidelity and extractor anchors. They also surface metadata conflicts, unrepresented content (including duplicate link definitions hidden inside containers), empty documents, unsupported raw HTML, suspicious extraction markers and unavailable assets.

`validate_document` detects altered derived text, links, duplicated provenance, revision IDs, status and findings by reconstructing the canonical result. It establishes **consistency, not authenticity**: an attacker who replaces all source inputs and hashes cannot be detected without an independently trusted source or signature. Raw HTML is preserved as data; consumers must not execute it. Nesting beyond 128 parser nodes is refused instead of risking an unbounded recursive traversal.

## 4. Reproduce Phase A evidence

See [ACCEPTANCE.md](ACCEPTANCE.md) for the criterion/evidence matrix, current results and limitations. Corpus sampling runs in the private collection repository, next to the corpus it reads.

The Linux sampler needs Python 3.10+ and `/usr/bin/time`. It selects up to ten size quantiles per top-level source group, including smallest/largest inputs. Each selected input is processed twice. Assertions check unchanged source/copy bytes, hash/length agreement, repeated JSON, statuses and exhaustive UTF-8 accounting. Reports contain paths and hashes but no source text; snapshots remain under ignored `target/`. Do not publish them without reviewing sensitive content and provenance. RSS uses fresh GNU time processes for each CLI run and Linux `VmHWM` for the driver, avoiding inherited parent high-water measurements.

Sampling is not full corpus acceptance, nor a production performance guarantee. A consistent `failed` snapshot remains inspectable through the loader; subsequent stages must reject its document status.

## 5. Verify

From the repository root:

```sh
just check    # the local gate; it must pass before every push
just native   # the native tokenizer tests, through the local binding
just mutants  # every mutant of the workspace; a survivor fails
```

`just check` runs formatting, strict Clippy, the tests, rustdoc with warnings denied, at least 90 % line coverage, licence, ban and source checks, unused dependencies, the workflow and secret checks, and the repository's policies. Pull requests run the same checks through the organization's `rust-workflows`. Rust 1.98.1 is the pinned toolchain; the declared MSRV is 1.85. The native tests read the tokenizer's artifacts through the binding described in [TOKENIZER.md](TOKENIZER.md).
