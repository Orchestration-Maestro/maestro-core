//! Public API boundary: a counter from another crate chunks through the crate-root trait.
//! The native tokenizer remains an accepted counter.
#![cfg(test)]
use maestro_canonicalization::{
    CanonicalizeInput, ChunkBatch, DedupInput, DedupScope, Error, NativeTokenizer, RevisionKey,
    TokenCounter, WarningPolicy, canonicalize, chunk_documents,
};
use std::cell::Cell;

/// A counter defined outside the crate, as the router's is: one ID per byte between BOS 0 and
/// EOS 2. Its `verify` can fail at a chosen call.
struct OutsideCounter {
    /// The `verify` call, counted from one, that fails; `None` never fails.
    failing_verify: Option<usize>,
    /// How many times `verify` ran.
    verifications: Cell<usize>,
}

impl OutsideCounter {
    fn new(failing_verify: Option<usize>) -> Self {
        Self {
            failing_verify,
            verifications: Cell::new(0),
        }
    }
}

impl TokenCounter for OutsideCounter {
    fn contract_id(&self) -> &'static str {
        "outside/bytes"
    }

    fn verify(&self) -> Result<(), Error> {
        let call = self.verifications.get() + 1;
        self.verifications.set(call);
        if self.failing_verify == Some(call) {
            return Err(Error("outside counter changed".into()));
        }
        Ok(())
    }

    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        let bytes = input.bytes().map(u32::from);
        Ok([0].into_iter().chain(bytes).chain([2]).collect())
    }
}

/// The Markdown every batch here chunks: 11 characters in 13 bytes, so the number of the outside
/// counter's IDs (15: one per byte, then BOS and EOS) differs from any count of characters (13).
const MARKDOWN: &str = "Héllo wörld";

/// The contract ID and the only chunk's token count of a one-document batch that `counter`
/// counts.
fn chunk_one(counter: &(impl TokenCounter + ?Sized)) -> Result<(String, usize), Error> {
    let document = canonicalize(CanonicalizeInput::new(MARKDOWN, "outside"))?;
    let scope = DedupScope {
        tenant_id: "outside-tenant".into(),
        workspace_id: "outside-workspace".into(),
        authorized_revisions: [RevisionKey {
            document_id: document.document_id.clone(),
            revision_id: document.revision_id.clone(),
        }]
        .into_iter()
        .collect(),
    };
    let inputs = [DedupInput {
        document: &document,
        markdown: MARKDOWN,
    }];
    let batch = chunk_documents(&scope, &inputs, WarningPolicy::Preserve, counter)?;
    assert_eq!(batch.chunks.len(), 1);
    Ok((
        batch.tokenizer_contract_id,
        batch.chunks[0].content.token_count,
    ))
}

#[test]
fn a_counter_from_another_crate_chunks_under_its_contract() {
    let counter = OutsideCounter::new(None);
    let (contract_id, token_count) = chunk_one(&counter).unwrap();
    assert_eq!(contract_id, "outside/bytes");
    assert_eq!(token_count, MARKDOWN.len() + 2);
    assert_eq!(counter.verifications.get(), 2);
}

#[test]
fn a_counter_behind_dyn_chunks_as_its_type_does() {
    let counter = OutsideCounter::new(None);
    let behind_dyn: &dyn TokenCounter = &counter;
    assert_eq!(chunk_one(behind_dyn).unwrap(), chunk_one(&counter).unwrap());
    assert_eq!(counter.verifications.get(), 4);
}

#[test]
fn a_failing_verify_of_an_outside_counter_refuses_the_batch() {
    for call in [1, 2] {
        let refused = chunk_one(&OutsideCounter::new(Some(call))).unwrap_err();
        assert_eq!(
            refused.to_string(),
            "outside counter changed",
            "call {call}"
        );
    }
}

#[test]
fn the_native_tokenizer_remains_an_accepted_counter() {
    let _: for<'a> fn(
        &'a DedupScope,
        &[DedupInput<'a>],
        WarningPolicy,
        &NativeTokenizer,
    ) -> Result<ChunkBatch<'a>, Error> = chunk_documents;
}
