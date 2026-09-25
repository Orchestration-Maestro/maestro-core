# Mapped structural chunking

**Task #14: PASS for the library-only contract and measured synthetic acceptance.**
No CLI/manifests, inference, embeddings, indexing, model downloads, permission
inference or source rewriting.

## Use

The example below is exercised by the explicitly invoked
`public_chunk_example_uses_native_complete_input` acceptance test. It authorizes
its own synthetic document. Production grants must come from a trusted authority,
not from document metadata or possession of a content hash.

```rust
use maestro_canonicalization::{
    CanonicalizeInput, DedupInput, DedupScope, Error, NativeTokenizer,
    RevisionKey, WarningPolicy, canonicalize, chunk_documents,
};

fn main() -> Result<(), Error> {
    let markdown = "Hello world";
    let document = canonicalize(CanonicalizeInput::new(markdown, "example"))?;
    let scope = DedupScope {
        tenant_id: "example-tenant".into(),
        workspace_id: "example-workspace".into(),
        authorized_revisions: [RevisionKey {
            document_id: document.document_id.clone(),
            revision_id: document.revision_id.clone(),
        }].into_iter().collect(),
    };
    let tokenizer = NativeTokenizer::open()?;
    let batch = chunk_documents(
        &scope, &[DedupInput { document: &document, markdown }],
        WarningPolicy::Preserve, &tokenizer,
    )?;
    assert_eq!(batch.chunks[0].content.prepared_input, "Hello world");
    assert_eq!(batch.chunks[0].content.token_count, 4);
    Ok(())
}
```

The qualified GGUF, native counter, libraries and source fingerprints must already
exist where the local binding says (see [TOKENIZER.md](TOKENIZER.md)).
See [TOKENIZER.md](TOKENIZER.md) for qualification, prerequisites and limitations.
There is no alternate-counter constructor or public callback bypass.

## Contract

Every call freshly invokes [scoped exact deduplication](DEDUPLICATION.md): all
revisions must be authorized, unique in the request, replay-valid and eligible
under the explicit warning policy. Only then are runtime artifacts verified and
source text prepared. A successful batch is returned only after final artifact
revalidation. Any refusal returns no partial batch.

The profiles are `mapped-structural-chunks/2`, `canonical-context-parts/v1` and
`ordered-input-parts/v1`. Complete input means the verbatim concatenation of
`input_parts`; native BOS/EOS and all context/separators count toward **target 500,
hard maximum 700**. There is no artificial minimum, clipping, hidden normalization,
truncation or primary-body overlap. Counts are cached by exact complete strings
only inside the authorized call. Context copies do not add primary coverage.

Whole compatible structures are preferred. Sections and unrelated containers are
not merged to reach the target. Continuations retain heading ancestry, directly
owned ancestor-item text, own ordinals/task status, code information, corresponding
table headers, definition terms and meaningful inline wrappers. Code-line and
cell fragments carry explicit split kinds; selected table columns are recorded,
not filled with invented blanks. Original typed documents, assets, extractor
relationships, policies and independent revisions remain in `deduplication`.

Scalar fallback repeatedly measures complete inputs and halves the candidate,
then tries a preferred boundary. A rest that fits stays one last piece, named
like the cuts before it, and prose is cut inside a word only when no whitespace
cut fits. It does not assume tokenizer monotonicity or claim a maximal fitting
prefix. Oversized mandatory context, unsupported unsafe
structure, incomplete accounting, changed artifacts and runtime errors refuse
rather than silently dropping text. No fallback to approximate token counts exists.

## Coordinates and coverage

All ranges are half-open **UTF-8 byte offsets**. Their containing record states
which coordinate space applies:

| Record | Coordinates |
| --- | --- |
| `SourceOrigin.span` | Original unchanged Markdown; valid for exact quotations. |
| `SourceUnit.mappings`, `InlineEnvelope` | That unit's canonical rendered text. |
| `Contribution.range` | Referenced source unit, not original Markdown. |
| `InputPart.mappings` | That input part's text. |
| `InputPart.prepared_range` | Complete concatenated tokenizer input. |

For original `&amp;`, rendered `&` occupies `[0,1)`, but its transformation origin
is original syntax `[0,5)`. A rendered fragment never invents an original `[0,1)`
quotation. Exact-copy origins can narrow only after byte equality is verified.
Transformed and hierarchical origins may overlap legitimately. Generated spacing
has no source quotation; source-derived ordinals and task markers retain mappings.

Primary fragment ranges partition every eligible unit exactly once. Separately,
every entry in the original-byte accounting ledger has an explicit disposition.
Metadata/reference declarations and structural syntax remain preserved without
becoming standalone searchable prose. Already-failed empty or metadata-only
Phase A documents are still refused. An accepted structural-only document, such
as an empty list item, yields `no_searchable_content` and zero chunks, not a
context-only input.

Chunk identities include scope, source revision, parser/preparation/tokenizer
profiles and ordered primary coordinates. Prepared groups compare actual bytes
of `(preparation_profile, tokenizer_contract_id, prepared_input)` after SHA-256
candidate matching. Group IDs include tenant/workspace but not membership;
singletons and every source occurrence remain. Unequal-byte collisions refuse.
Operational metadata and request ordering do not change chunk/group identities.
Saved batches are not authorization certificates; later consumers must reauthorize.

## Verification and limits

```bash
cargo test --manifest-path canonicalization/Cargo.toml --locked --offline
cargo test --manifest-path canonicalization/Cargo.toml --locked --offline \
  --test it chunk_native -- --ignored --nocapture
cargo clippy --manifest-path canonicalization/Cargo.toml --locked --offline \
  --all-targets -- -D warnings
just check
```

Ordinary tests deliberately ignore machine-specific native acceptance. An ignored
test is not a pass. Private scalar-counter tests establish structure, not native
budget compliance.

The explicit native run passed **four tests in 209.498 seconds**: independent
ordered-ID goldens; the public example; **21 synthetic documents / 61 chunks**
independently recounted and checked for complete nonoverlapping primary coverage;
and heading/parent-context refusal. Cases include plain/contextual
499/500/501/699/700/701 boundaries, long prose/lists/code/cells, Unicode, NUL,
literal specials, deletion continuations and checked/unchecked tasks.

One independent read-only review found three issues: missing task status on
continuations, origin-free own-item ordinals, and early loose-definition splits.
Each was reproduced by a failing regression, fixed by the parent, and followed
by passing full Rust tests and the native run. The original review verdict was
BLOCK; the three findings are parent-verified resolved, not a new independent
clean-review verdict. Review artifact: workflow
`1051cb6c-9df5-4b26-8523-543fb14cf88e`, `review/mapped-structural-chunks.md`.

Five targeted mutations were caught as behavioral failures in a disposable copy:
hard maximum, rendered coverage, authorization, collision byte comparison and
group scope. Its unmodified baseline passed. Each run uses a fresh build target
to prevent reuse of a prior mutated binary. These checks are not the full golden
mutation gate.

Final development verification passed **94 tests + one doctest**, strict format,
Clippy and rustdoc, and the unchanged `just check` gate in **33.697 seconds**.
Line coverage is **4,721 / 5,116 = 92.28%**, above the unchanged 80% floor.
Active LSP probes were inconclusive: no files confirmed clean; six informational
let-chain suggestions do not supersede the declared Rust 1.85 MSRV. Compiler
and executed gates, not silent language-server output, establish these results.

Preservation checks compared 25 preexisting source/test/example/contract files:
only intended `lib.rs` exports and `parse.rs` option-function visibility changed.
Original fixtures and the pinned contract are unchanged; native runs reverified
runtime artifacts. The protected fixture was checked by inode/size/mtime only.
HEAD is unchanged and no files are staged or committed.

Reproduction logs and command records are under ignored
`target/chunk-acceptance/`: `review-fixes-commands.json`,
`chunk-native-reviewed.{json,log}`, `targeted-mutations.json` and
`mutation-checks.py`, `final-commands.json`, `final-coverage.json` and
`preservation-final.json`. Corpus acceptance, throughput at corpus scale, live encoder
identity, full-local security/release and hosted CI are **not established**.
The CLI/manifests and whole Phase B acceptance remain separate work.
