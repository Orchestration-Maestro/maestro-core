//! Validate the immutable graph before it crosses the persistence boundary.

use super::{
    coverage,
    error::{Error, require},
    group_parts, hierarchy,
    mapping::MappingLedger,
    membership,
    types::DeliveryGraph,
};
use crate::artifact::Digest;

impl DeliveryGraph {
    /// Validates exact source accounting, mapping, memberships and ancestry.
    ///
    /// # Errors
    /// Any mismatched digest, unsupported schema, invalid range, ownership,
    /// membership, structural relation or family key is refused.
    pub fn validate(&self, ledger: &MappingLedger, source: &str) -> Result<(), Error> {
        self.validate_inner(ledger, source, true)
    }

    /// Validates a CAS-loaded graph whose artifact hashes were verified by the store.
    pub(crate) fn validate_recorded(
        &self,
        ledger: &MappingLedger,
        source: &str,
    ) -> Result<(), Error> {
        self.validate_inner(ledger, source, false)
    }

    /// Runs graph invariants with optional digest recomputation.
    fn validate_inner(
        &self,
        ledger: &MappingLedger,
        source: &str,
        verify_digests: bool,
    ) -> Result<(), Error> {
        let descriptor = &self.descriptor;
        require(
            descriptor.schema_version == "maestro-unit-graph/1"
                && ledger.schema_version == "maestro-unit-mapping/1",
            "unsupported graph or mapping schema",
        )?;
        require(
            ledger.original_markdown_digest == descriptor.original_markdown_digest,
            "source digest metadata mismatch",
        )?;
        if verify_digests {
            require(
                descriptor.original_markdown_digest == Digest::of(source.as_bytes())
                    && descriptor.mapping_digest == Digest::of(&ledger.to_bytes()?),
                "source or mapping digest mismatch",
            )?;
        }
        for identity in [
            &descriptor.collection_id,
            &descriptor.source_namespace,
            &descriptor.document_id,
            &descriptor.revision_id,
            &descriptor.counter_contract,
        ] {
            require(!identity.trim().is_empty(), "empty descriptor identity")?;
        }
        require(
            descriptor.profile_name == "mapped-structural-chunks/4"
                && descriptor.preparation_name == "canonical-context-parts/v3",
            "unsupported profile or preparation",
        )?;
        coverage::validate(self, ledger, source)?;
        hierarchy::validate(self)?;
        group_parts::validate(self)?;
        membership::validate(self)
    }
}
