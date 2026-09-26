//! The seam every token counter fills: what the chunker counts through, whoever counts.
use crate::error::Error;

/// Counts tokens exactly as the selected embedding model will see them.
///
/// [`chunk_documents`](crate::chunk_documents) verifies the counter before and after each batch
/// and puts its contract ID in every chunk and prepared-input identity, so batches counted by
/// different counters never share an identity (ADR-0008).
pub trait TokenCounter {
    /// Identity of the model, tokenizer build and preparation policy that count.
    fn contract_id(&self) -> &str;

    /// Recheck what the counts depend on; the chunker calls it before and after each batch.
    ///
    /// # Errors
    /// Refuses a counter whose artifacts are unavailable or changed.
    fn verify(&self) -> Result<(), Error>;

    /// The ordered token IDs of one complete input, special tokens included, without padding or
    /// truncation.
    ///
    /// # Errors
    /// Refuses an input the counter cannot tokenize exactly.
    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error>;
}
