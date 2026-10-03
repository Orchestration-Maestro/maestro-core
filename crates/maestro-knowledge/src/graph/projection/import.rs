//! Frozen authoritative inputs for the one parameterized projection loader.
use super::{
    port::{EdgeFamily, EntityFact, ProjectionEdge, ProjectionError, ProjectionScope},
    writer::BuildVerification,
};
use crate::graph::resolve::resolve_snapshot;
use maestro_kernel::{
    artifact::Digest,
    facts::{ClaimSetRecord, Endpoint, Mention, Object, ResolutionSnapshot, ReviewRecord},
    scope::ScopeSet,
    store::Database,
};
use std::collections::BTreeMap;

/// Both row families derived from the same frozen claim membership and resolution.
#[derive(Debug)]
pub struct ProjectionSnapshot {
    /// Frozen projection scope, including for an empty claim set.
    pub scope: ProjectionScope,
    /// Exact ordered membership, with review states frozen by the resolution pin.
    pub claim_set: ClaimSetRecord,
    /// Caller-selected resolution identity, never a latest-snapshot lookup.
    pub resolution_id: Digest,
    /// Recorded version of the resolver that produced the endpoints.
    pub resolver_version: String,
    /// Frozen attributed decisions, including supersession and alias history.
    pub history: Vec<ReviewRecord>,
    /// Entity-valued claims, including rejected and superseded membership.
    pub edges: Vec<ProjectionEdge>,
    /// Literal-valued subject records, never literal nodes or edges.
    pub facts: Vec<EntityFact>,
}

impl ProjectionSnapshot {
    /// Read a caller-selected claim set and resolution pin through kernel authority.
    /// No latest-snapshot lookup, live review substitution or membership filtering occurs.
    ///
    /// # Errors
    /// Refuses invisible, incomplete or incompatible frozen sources.
    pub fn read(
        kernel: &Database,
        scopes: &ScopeSet,
        principal: &str,
        pins: (&Digest, &Digest),
        scope: &ProjectionScope,
    ) -> Result<Self, ProjectionError> {
        let set = kernel
            .claim_set(scopes, pins.0)
            .map_err(|error| ProjectionError::Backend(error.to_string()))?
            .ok_or(ProjectionError::Unauthorized)?;
        let snapshot = kernel
            .resolution(scopes, principal, pins.1)
            .map_err(|error| ProjectionError::Backend(error.to_string()))?
            .ok_or(ProjectionError::Unauthorized)?;
        Self::derive(&set, &snapshot, scope)
    }

    /// Derive only pinned members using the existing exact snapshot resolver.
    pub(super) fn derive(
        set: &ClaimSetRecord,
        snapshot: &ResolutionSnapshot,
        scope: &ProjectionScope,
    ) -> Result<Self, ProjectionError> {
        if set.collection_id != scope.collection_id
            || scope.generation_id <= 0
            || !snapshot.sets.contains(&set.id)
        {
            return Err(refusal());
        }
        let entities = resolve_snapshot(snapshot).map_err(|_| refusal())?;
        let endpoints: BTreeMap<_, _> = entities
            .into_iter()
            .flat_map(|entity| {
                entity
                    .mentions
                    .into_iter()
                    .map(move |mention| (mention, entity.id.clone()))
            })
            .collect();
        let frozen: BTreeMap<_, _> = snapshot
            .claims
            .iter()
            .map(|claim| (&claim.id, claim))
            .collect();
        let mut rows = Self {
            scope: scope.clone(),
            claim_set: ClaimSetRecord {
                id: set.id.clone(),
                collection_id: set.collection_id.clone(),
                claims: Vec::new(),
            },
            resolution_id: snapshot.id.clone(),
            resolver_version: snapshot.resolver_version.clone(),
            history: snapshot.history.clone(),
            edges: Vec::new(),
            facts: Vec::new(),
        };
        for member in &set.claims {
            let claim = frozen.get(&member.id).ok_or_else(refusal)?;
            if claim.collection_id != set.collection_id || claim.claim != member.claim {
                return Err(refusal());
            }
            let endpoint = |endpoint| {
                endpoints
                    .get(&Mention {
                        claim: member.id.clone(),
                        endpoint,
                    })
                    .cloned()
                    .ok_or_else(refusal)
            };
            let subject = endpoint(Endpoint::Subject)?;
            rows.claim_set.claims.push((*claim).clone());
            match &claim.claim.object {
                Object::Literal(_) => rows.facts.push(EntityFact {
                    claim: (*claim).clone(),
                    subject,
                    scope: scope.clone(),
                }),
                Object::Entity(_) => rows.edges.push(ProjectionEdge {
                    id: member.id.clone(),
                    scope: scope.clone(),
                    family: EdgeFamily::KnowledgeClaim,
                    source: subject,
                    target: endpoint(Endpoint::Object)?,
                    relation: claim.claim.predicate.as_str().into(),
                }),
            }
        }
        BuildVerification::expected(&rows.edges, &rows.facts)?;
        Ok(rows)
    }
}

/// A frozen mismatch never falls back to current inputs.
fn refusal() -> ProjectionError {
    ProjectionError::Invalid(
        "frozen projection snapshot differs; rebuild with matching pins".into(),
    )
}
