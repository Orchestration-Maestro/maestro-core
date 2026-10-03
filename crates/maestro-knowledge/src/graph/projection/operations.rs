//! Backend-neutral public operations on opaque lifecycle handles.
use super::{
    BuildVerification, CatalogRelationVocabulary, EngineSettings, EntityFact, ProjectionEdge,
    ProjectionError, ProjectionProducer, PublishedProjection,
};

impl ProjectionProducer<'_> {
    /// Write a fully validated knowledge edge/fact batch atomically.
    ///
    /// # Errors
    /// Refuses invalid records, denied scope, lost/expired leases and native failures.
    pub fn write_batch(
        &mut self,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
    ) -> Result<(), ProjectionError> {
        self.session.write_batch(edges, facts, None)
    }

    /// Write a batch with the catalog owner's registered relation vocabulary.
    ///
    /// # Errors
    /// Refuses rejected catalog relations and the ordinary atomic-batch refusals.
    pub fn write_batch_with_catalog_vocabulary(
        &mut self,
        edges: &[ProjectionEdge],
        facts: &[EntityFact],
        vocabulary: &dyn CatalogRelationVocabulary,
    ) -> Result<(), ProjectionError> {
        self.session.write_batch(edges, facts, Some(vocabulary))
    }

    /// Checkpoint, close/reopen and verify durable rows; further writes invalidate verification.
    ///
    /// # Errors
    /// Refuses lost/expired leases, poisoned sessions and malformed durable content.
    pub fn verify(&mut self) -> Result<BuildVerification, ProjectionError> {
        self.session.verify()
    }

    /// Reverify against independently expected content, install without overwrite, then
    /// record readiness under the matching live lease. Always consumes the writer.
    /// Failures preserve orphans and require explicit recovery, never blind retry.
    ///
    /// # Errors
    /// Refuses verification/authority/lease mismatches, existing files and durability failures.
    pub fn publish(
        self,
        expected: &BuildVerification,
    ) -> Result<PublishedProjection, ProjectionError> {
        self.session.publish(expected)
    }

    /// Close native staging, release guards, preserve orphans and cancel the exact project job.
    ///
    /// # Errors
    /// Refuses lost/mismatched leases or kernel failures, still consuming the session.
    /// A current holder may cancel even after expiry; takeover is fenced atomically.
    pub fn cancel(self) -> Result<(), ProjectionError> {
        self.session.cancel()
    }

    /// The explicit frozen settings admitted by the factory.
    #[must_use]
    pub fn settings(&self) -> &EngineSettings {
        self.session.settings()
    }
}
