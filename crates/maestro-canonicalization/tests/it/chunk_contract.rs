//! Public API boundary: production callers cannot supply a substitute counter.
#![cfg(test)]
use maestro_canonicalization::{
    ChunkBatch, DedupInput, DedupScope, Error, NativeTokenizer, WarningPolicy, chunk_documents,
};

#[test]
fn public_chunk_api_requires_the_qualified_counter_type() {
    let _: for<'a> fn(
        &'a DedupScope,
        &[DedupInput<'a>],
        WarningPolicy,
        &NativeTokenizer,
    ) -> Result<ChunkBatch<'a>, Error> = chunk_documents;
}
