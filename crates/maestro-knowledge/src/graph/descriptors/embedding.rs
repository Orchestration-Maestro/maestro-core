//! Descriptor embeddings through the existing dense gateway and checks.

use super::{
    build::refused,
    port::{DescriptorEmbedder, EmbeddedDescriptors},
    types::{
        BUILDER_VERSION, Descriptor, DescriptorError, DescriptorPin, DescriptorProfile,
        DescriptorReceipt,
    },
};
use crate::index::{check_dense, embed_dense, embedding_profile};
use maestro_kernel::{artifact::Digest, gateway::ModelPort};
use std::num::NonZeroUsize;

impl<P: ModelPort> DescriptorEmbedder<'_, P> {
    /// Embed canonical documents or reuse a compatible checked output across arms.
    /// Empty inputs are refused without a model call; rule-only callers skip this.
    ///
    /// # Errors
    /// Refuses incompatible pins, cards, cached outputs or malformed vectors.
    pub async fn prepare(
        &self,
        pin: &DescriptorPin,
        documents: &[Descriptor],
        cached: Option<&EmbeddedDescriptors>,
    ) -> Result<EmbeddedDescriptors, DescriptorError> {
        let dimensions = self
            .card
            .fields()
            .dimensions
            .map(NonZeroUsize::get)
            .ok_or_else(|| refused("descriptor card is not an embedder"))?;
        if documents.is_empty()
            || documents
                .iter()
                .any(|document| document.pin != *pin || document.id != document.identity())
        {
            return Err(refused("inconsistent descriptor content or pin"));
        }
        let mut documents = documents.to_vec();
        documents.sort_by(|left, right| left.id.cmp(&right.id));
        documents.dedup();
        let receipt = DescriptorReceipt {
            pin: pin.clone(),
            profile: DescriptorProfile {
                builder: Digest::of(BUILDER_VERSION.as_bytes()),
                embedding: self.card.digest().clone(),
                preprocessing: Digest::of(embedding_profile(self.card).as_bytes()),
                linking: self.linking.clone(),
                dimensions,
            },
            content: Digest::of(serde_json::json!(documents).to_string().as_bytes()),
            count: documents.len(),
        };
        if let Some(cached) = cached {
            if cached.receipt != receipt || cached.documents != documents {
                return Err(refused("incompatible descriptor projection receipt"));
            }
            check_dense(&cached.vectors, documents.len(), dimensions)
                .map_err(|_| refused("invalid cached vectors"))?;
            return Ok(cached.clone());
        }
        let inputs: Vec<_> = documents
            .iter()
            .map(|document| document.text.clone())
            .collect();
        let vectors = embed_dense(self.models, self.card, &inputs, self.deadline)
            .await
            .map_err(|_| refused("descriptor embedding failed"))?;
        Ok(EmbeddedDescriptors {
            receipt,
            documents,
            vectors,
        })
    }
}
