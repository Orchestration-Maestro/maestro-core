//! Optional G35 composition; canonical source data and frozen profiles remain authoritative.
use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::{artifact::Digest, scope::LOCAL};
use maestro_knowledge::graph::descriptors::{
    self, DescriptorInput, DescriptorProfile, DescriptorProjection, EmbeddedDescriptors,
};
use maestro_knowledge::graph::projection::ProjectionScope;

/// Existing checked embeddings and a replaceable rebuild/readiness operation.
/// The rule-only CLI never constructs this; no model or Qdrant port is activated.
pub(super) struct Descriptors<'a> {
    /// G35's immutable checked output, reusable only under identical profile/content pins.
    pub(super) output: &'a EmbeddedDescriptors,
    /// Independently frozen embedding/preprocessing/linking profile.
    pub(super) profile: &'a DescriptorProfile,
    /// Composition of the existing projection's rebuild and verify operations.
    pub(super) project: &'a dyn Fn(&EmbeddedDescriptors) -> Result<(), Failure>,
}
impl Descriptors<'_> {
    /// Reconstruct canonical documents from the same scope, membership and resolution.
    pub(super) fn run(
        &self,
        kernel: &Kernel,
        pins: (&ProjectionScope, &Digest, &Digest),
    ) -> Result<(), Failure> {
        let (scope, claim_set, resolution) = pins;
        let receipt = self.output.receipt();
        if receipt.profile != *self.profile
            || receipt.pin.collection_id != scope.collection_id
            || receipt.pin.generation_id != scope.generation_id
        {
            return Err(Failure::refused(
                "descriptor profile or generation differs from graph rebuild",
            ));
        }
        let input = DescriptorInput::read(
            &kernel.database,
            &kernel.scopes,
            LOCAL,
            (&receipt.pin, resolution),
        )
        .map_err(|error| Failure::refused_by(&error))?;
        let documents = descriptors::build(&input).map_err(|error| Failure::refused_by(&error))?;
        if documents != self.output.descriptors()
            || documents.iter().any(|document| {
                document.claim_set != *claim_set || document.resolution != *resolution
            })
        {
            return Err(Failure::refused(
                "descriptor content differs from frozen graph rebuild",
            ));
        }
        (self.project)(self.output)
    }
}

/// Adapter composition is asynchronous like G35; callers own runtime and model selection.
#[cfg_attr(
    any(not(test), windows),
    expect(
        dead_code,
        reason = "rule-only pilot leaves descriptor composition disabled"
    )
)]
pub(super) async fn project<P: DescriptorProjection>(
    port: &P,
    output: &EmbeddedDescriptors,
) -> Result<(), Failure> {
    port.rebuild(output)
        .await
        .map_err(|error| Failure::refused_by(&error))?;
    port.verify(output)
        .await
        .map_err(|error| Failure::refused_by(&error))
}
