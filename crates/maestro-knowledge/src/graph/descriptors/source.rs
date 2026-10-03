//! Read-only authority reader for generation-bound original artifacts.

use super::{
    build::{DescriptorInput, refused},
    types::{DescriptorError, DescriptorPin},
};
use crate::graph::verify::Source;
use maestro_kernel::{
    artifact::Digest,
    chunk_set::ChunkSetState,
    document::{Outcome, RevisionStatus},
    generation::GenerationState,
    scope::ScopeSet,
    store::Database,
};
use std::collections::{BTreeMap, BTreeSet};

impl DescriptorInput {
    /// Read the attached frozen set, reviewed identities and pinned original artifacts.
    /// Current grants are refreshed; no projection is ever an input authority.
    ///
    /// # Errors
    /// Refuses hidden/unattached pins, inconsistent resolution or unverified sources.
    pub fn read(
        database: &Database,
        request: &ScopeSet,
        principal: &str,
        selection: (&DescriptorPin, &Digest),
    ) -> Result<Self, DescriptorError> {
        let (pin, resolution) = selection;
        let scopes = database
            .visible(principal)
            .map_err(|_| refused("authority read failed"))?;
        let snapshot = database
            .resolution(request, principal, resolution)
            .map_err(|_| refused("authority read failed"))?
            .ok_or_else(|| refused("unknown descriptor authority"))?;
        let generation = database
            .generation(&scopes, pin.generation_id)
            .map_err(|_| refused("authority read failed"))?
            .ok_or_else(|| refused("unknown descriptor authority"))?;
        if generation.collection_id != pin.collection_id
            || generation.state == GenerationState::Failed
        {
            return Err(refused("invalid generation selection"));
        }
        let chunks = database
            .chunk_set(&scopes, &generation.chunk_set_id)
            .map_err(|_| refused("authority read failed"))?
            .ok_or_else(|| refused("unknown descriptor authority"))?;
        if chunks.state != ChunkSetState::Complete {
            return Err(refused("incomplete source generation"));
        }
        let attachment = database
            .graph_attachment(&scopes, generation.id)
            .map_err(|_| refused("authority read failed"))?
            .ok_or_else(|| refused("unattached generation"))?;
        if !snapshot.sets.contains(&attachment.claim_set_id) {
            return Err(refused("resolution does not include attached claims"));
        }
        let set = database
            .claim_set(&scopes, &attachment.claim_set_id)
            .map_err(|_| refused("authority read failed"))?
            .ok_or_else(|| refused("unknown descriptor authority"))?;
        let ids: BTreeSet<_> = set.claims.iter().map(|claim| &claim.id).collect();
        let claims: Vec<_> = snapshot
            .claims
            .iter()
            .filter(|claim| ids.contains(&claim.id))
            .cloned()
            .collect();
        let members: BTreeSet<_> = database
            .chunks(&scopes, &generation.chunk_set_id)
            .map_err(|_| refused("authority read failed"))?
            .into_iter()
            .map(|chunk| chunk.revision_id)
            .collect();
        let mut sources = BTreeMap::new();
        let needed: BTreeSet<_> = claims
            .iter()
            .flat_map(|record| &record.claim.supports)
            .map(|support| &support.revision_id)
            .collect();
        for revision_id in needed {
            if !members.contains(revision_id) {
                return Err(refused("support is outside generation"));
            }
            let revision = database
                .revision(&scopes, revision_id)
                .map_err(|_| refused("authority read failed"))?
                .ok_or_else(|| refused("unknown descriptor authority"))?;
            let disposition = database
                .disposition(&scopes, &revision.id)
                .map_err(|_| refused("authority read failed"))?
                .ok_or_else(|| refused("unverified source"))?;
            if revision.status == RevisionStatus::Failed
                || !matches!(
                    disposition.outcome,
                    Outcome::Accepted | Outcome::AcceptedWithWarnings
                )
            {
                return Err(refused("ineligible source"));
            }
            if pin.version.as_ref().is_some_and(|version| {
                revision
                    .metadata
                    .get("version")
                    .and_then(serde_json::Value::as_str)
                    != Some(version.as_str())
            }) {
                return Err(refused("source version mismatch"));
            }
            let source = Source::read(database, &revision)
                .map_err(|_| refused("source artifact read failed"))?;
            sources.insert(revision.id, source);
        }
        Ok(Self {
            pin: pin.clone(),
            claim_set: attachment.claim_set_id,
            snapshot,
            claims,
            sources,
        })
    }
}
