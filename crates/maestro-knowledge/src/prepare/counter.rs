//! The router tokenizer as the chunker counts through it: the chunker's
//! refusal holds text only, so this counter keeps the typed refusal the
//! tokenizer gave, which tells a counter that stopped the batch from a
//! document the chunker refused.

use super::{error::TokenizerError, router_tokenizer::RouterTokenizer};
use maestro_canonicalization::{Error, TokenCounter};
use std::cell::RefCell;

/// The router tokenizer, keeping the last refusal it gave the chunker.
#[derive(Debug)]
pub(super) struct Counting<'a> {
    /// The tokenizer that counts.
    tokenizer: &'a RouterTokenizer,
    /// Its last refusal, until taken.
    refusal: RefCell<Option<TokenizerError>>,
}

impl<'a> Counting<'a> {
    /// Counts through `tokenizer`.
    pub(super) fn new(tokenizer: &'a RouterTokenizer) -> Self {
        Self {
            tokenizer,
            refusal: RefCell::new(None),
        }
    }

    /// The refusal the tokenizer gave since the last call, if it gave one.
    pub(super) fn take_refusal(&self) -> Option<TokenizerError> {
        self.refusal.borrow_mut().take()
    }

    /// Keeps `refusal`, and gives the chunker its text.
    fn keep(&self, refusal: TokenizerError) -> Error {
        let text = Error(refusal.to_string());
        *self.refusal.borrow_mut() = Some(refusal);
        text
    }
}

impl TokenCounter for Counting<'_> {
    fn contract_id(&self) -> &str {
        self.tokenizer.contract_id()
    }

    fn verify(&self) -> Result<(), Error> {
        self.tokenizer.check().map_err(|refusal| self.keep(refusal))
    }

    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        self.tokenizer
            .count(input)
            .map_err(|refusal| self.keep(refusal))
    }
}
