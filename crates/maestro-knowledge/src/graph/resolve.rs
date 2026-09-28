//! Exact sourced identities and replaceable validity ordering; no fuzzy linking.

use crate::lexical::fold;
use maestro_kernel::{
    artifact::Digest,
    facts::{
        DecisionKind, Endpoint, EntityName, Mention, Object, ResolutionSnapshot, Support, Validity,
    },
};
use serde_json::json;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    error, fmt,
};

/// Immutable algorithm identity. Changed normalization or grouping needs a new
/// version and new snapshots; the golden derivation test pins this version.
pub const EXACT_RESOLVER_VERSION: &str = "maestro-exact-resolution/1";

/// How a subject resolves: one entity for its kind and exact spelling, or
/// ambiguous with the others whose name normalizes alike.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// The subject: its kind and exact spelling.
    pub subject: EntityName,
    /// Its name without accents or case.
    pub normalized: String,
    /// Other subjects whose name normalizes alike, another spelling or
    /// another kind: empty when it resolves to one entity.
    pub colliding: Vec<EntityName>,
}

/// How each distinct subject of `subjects`, of one collection, resolves:
/// one entity for a kind and an exact spelling, however many documents name
/// it, and ambiguous when another spelling or another kind normalizes to the
/// same name. Similarity alone never merges two subjects. Ordered by
/// normalized name, kind and spelling.
#[must_use]
pub fn resolve(subjects: &[EntityName]) -> Vec<Resolution> {
    let distinct: BTreeMap<(String, &str, &str), &EntityName> = subjects
        .iter()
        .map(|subject| {
            (
                (
                    normalize(&subject.name),
                    subject.kind.as_str(),
                    subject.name.as_str(),
                ),
                subject,
            )
        })
        .collect();
    distinct
        .iter()
        .map(|((normalized, kind, name), subject)| Resolution {
            subject: (*subject).clone(),
            normalized: normalized.clone(),
            colliding: distinct
                .iter()
                .filter(|((other_normalized, other_kind, other_name), _)| {
                    other_normalized == normalized && (*other_kind, *other_name) != (*kind, *name)
                })
                .map(|(_, other)| (*other).clone())
                .collect(),
        })
        .collect()
}

/// `name` without accents or case, its spaces collapsed.
fn normalize(name: &str) -> String {
    fold(name)
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Domain-specific ordering of version or world-time values. Unknown order
/// must remain unknown; callers never substitute record time.
pub trait ValidityOrder {
    /// Compare two source values, or decline when their order is not known.
    fn compare(&self, left: &str, right: &str) -> Option<Ordering>;
}

/// The default adapter assumes only exact equality, never lexical version order.
#[derive(Debug)]
pub struct EqualityOnly;

impl ValidityOrder for EqualityOnly {
    fn compare(&self, left: &str, right: &str) -> Option<Ordering> {
        (left == right).then_some(Ordering::Equal)
    }
}

/// Whether a point lies in a known half-open range. Unknown validity or any
/// unknown comparison returns none, even if another bound is comparable.
#[must_use]
pub fn contains(validity: &Validity, point: &str, order: &dyn ValidityOrder) -> Option<bool> {
    let Validity::Bounded { start, end } = validity else {
        return None;
    };
    let after_start = match start {
        Some(start) => order.compare(point, start)? != Ordering::Less,
        None => true,
    };
    let before_end = match end {
        Some(end) => order.compare(point, end)? == Ordering::Less,
        None => true,
    };
    Some(after_start && before_end)
}

/// One resolved identity with its sourced mentions, never a literal node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcedEntity {
    /// Stable application identity of the reviewed target or exact source name.
    pub id: Digest,
    /// The canonical target's collection; all source collections remain on claims.
    pub collection_id: String,
    /// The target's exact source spelling and kind.
    pub subject: EntityName,
    /// Its normalized spelling, using the original G03 resolver.
    pub normalized: String,
    /// Unreviewed spelling/kind collisions in this collection.
    pub colliding: Vec<EntityName>,
    /// Sourced occurrences, including reviewed aliases in other collections.
    pub mentions: Vec<Mention>,
    /// Unique quote digests: duplicated supporting copies never add votes.
    pub support_groups: Vec<Digest>,
}

/// A snapshot cannot be resolved without inventing a target or choosing a cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionError;

impl fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "unsupported resolver version, unknown endpoint, cycle or unmatched separation",
        )
    }
}

impl error::Error for ResolutionError {}

/// A sourced occurrence and the base identity the original resolver gives it.
struct Occurrence<'a> {
    /// Its source-backed endpoint.
    mention: Mention,
    /// The source collection, never discarded by a cross-collection review.
    collection: &'a str,
    /// Its exact entity name.
    name: &'a EntityName,
    /// Verified supports, shared with their immutable claim.
    supports: &'a [Support],
}

/// Resolve exact names within collections, then apply attributed alias decisions.
/// Supersession never removes a claim; consumers retain the snapshot's qualifiers
/// and review history. A review applies to the whole exact identity group;
/// reversing it restores that group, never splits identical sourced names.
///
/// # Errors
/// Returns [`ResolutionError`] for dangling mentions, cyclic aliases or unmatched separations.
pub fn resolve_snapshot(
    snapshot: &ResolutionSnapshot,
) -> Result<Vec<SourcedEntity>, ResolutionError> {
    if snapshot.resolver_version != EXACT_RESOLVER_VERSION {
        return Err(ResolutionError);
    }
    // ponytail: in-memory scans are quadratic; index occurrences for larger snapshots.
    let occurrences = occurrences(snapshot);
    let mut by_collection = BTreeMap::<&str, Vec<EntityName>>::new();
    for occurrence in &occurrences {
        by_collection
            .entry(occurrence.collection)
            .or_default()
            .push(occurrence.name.clone());
    }
    let resolved: BTreeMap<_, _> = by_collection
        .into_iter()
        .map(|(collection, names)| (collection, resolve(&names)))
        .collect();
    let identities: BTreeMap<_, _> = occurrences
        .iter()
        .map(|occurrence| (occurrence.mention.clone(), identity(occurrence)))
        .collect();
    let aliases = aliases(snapshot, &identities)?;
    let mut entities = BTreeMap::<Digest, SourcedEntity>::new();
    for occurrence in &occurrences {
        let original = identities.get(&occurrence.mention).ok_or(ResolutionError)?;
        let id = target(original, &aliases)?;
        let canonical = occurrences
            .iter()
            .find(|item| identities.get(&item.mention) == Some(&id))
            .ok_or(ResolutionError)?;
        let resolution = resolved
            .get(canonical.collection)
            .and_then(|items| items.iter().find(|item| item.subject == *canonical.name))
            .ok_or(ResolutionError)?;
        let entity = entities.entry(id.clone()).or_insert_with(|| SourcedEntity {
            id,
            collection_id: canonical.collection.to_owned(),
            subject: canonical.name.clone(),
            normalized: resolution.normalized.clone(),
            colliding: resolution.colliding.clone(),
            mentions: Vec::new(),
            support_groups: Vec::new(),
        });
        entity.mentions.push(occurrence.mention.clone());
        entity.support_groups.extend(
            occurrence
                .supports
                .iter()
                .map(|support| support.quote_digest.clone()),
        );
    }
    for entity in entities.values_mut() {
        let mut colliding = Vec::new();
        for name in &entity.colliding {
            let occurrence = occurrences
                .iter()
                .find(|item| item.collection == entity.collection_id && item.name == name)
                .ok_or(ResolutionError)?;
            if target(&identity(occurrence), &aliases)? != entity.id {
                colliding.push(name.clone());
            }
        }
        entity.colliding = colliding;
        entity.mentions.sort();
        entity.support_groups.sort();
        entity.support_groups.dedup();
    }
    Ok(entities.into_values().collect())
}

/// Entity endpoints only; typed literal values remain claim properties.
fn occurrences(snapshot: &ResolutionSnapshot) -> Vec<Occurrence<'_>> {
    let mut occurrences = Vec::new();
    for record in &snapshot.claims {
        let claim = &record.claim;
        let mut endpoints = vec![(Endpoint::Subject, &claim.subject)];
        if let Object::Entity(name) = &claim.object {
            endpoints.push((Endpoint::Object, name));
        }
        occurrences.extend(endpoints.into_iter().map(|(endpoint, name)| Occurrence {
            mention: Mention {
                claim: record.id.clone(),
                endpoint,
            },
            collection: &record.collection_id,
            name,
            supports: &claim.supports,
        }));
    }
    occurrences
}

/// Stable scoped identity: no revision, record time or supporting-copy count.
fn identity(occurrence: &Occurrence<'_>) -> Digest {
    Digest::of(
        json!([
            "maestro-entity/1",
            occurrence.collection,
            normalize(&occurrence.name.name),
            occurrence.name.kind.as_str(),
            occurrence.name.name
        ])
        .to_string()
        .as_bytes(),
    )
}

/// Latest identity disposition per exact group, with all sourced history intact.
fn aliases(
    snapshot: &ResolutionSnapshot,
    identities: &BTreeMap<Mention, Digest>,
) -> Result<BTreeMap<Digest, Digest>, ResolutionError> {
    let mut aliases = BTreeMap::new();
    for review in &snapshot.history {
        let decision = &review.decision;
        let left = identities.get(&decision.left).ok_or(ResolutionError)?;
        let right = identities.get(&decision.right).ok_or(ResolutionError)?;
        match decision.kind {
            DecisionKind::Alias if left != right => {
                aliases.insert(left.clone(), right.clone());
            }
            DecisionKind::Separate => {
                if aliases.get(left) != Some(right) {
                    return Err(ResolutionError);
                }
                aliases.remove(left);
            }
            DecisionKind::Alias | DecisionKind::Supersedes => {}
        }
    }
    Ok(aliases)
}

/// Follow only explicit reviewed aliases; refuse cycles instead of selecting a name.
fn target(
    identity: &Digest,
    aliases: &BTreeMap<Digest, Digest>,
) -> Result<Digest, ResolutionError> {
    let mut seen = BTreeSet::new();
    let mut current = identity;
    while let Some(next) = aliases.get(current) {
        if !seen.insert(current) {
            return Err(ResolutionError);
        }
        current = next;
    }
    Ok(current.clone())
}

/// Replaceable identity resolution over kernel-authorized, source-backed pins.
pub trait IdentityResolver {
    /// Algorithm identity that callers freeze in the authoritative snapshot.
    fn version(&self) -> &str;

    /// Resolve a snapshot without modifying its claims or source evidence.
    ///
    /// # Errors
    /// Returns [`ResolutionError`] when decisions cannot resolve unambiguously.
    fn resolve(&self, snapshot: &ResolutionSnapshot)
    -> Result<Vec<SourcedEntity>, ResolutionError>;
}

/// The rules-first adapter: exact scoped names plus explicit reviewed aliases.
#[derive(Debug)]
pub struct ExactResolver;

impl IdentityResolver for ExactResolver {
    fn version(&self) -> &str {
        EXACT_RESOLVER_VERSION
    }

    fn resolve(
        &self,
        snapshot: &ResolutionSnapshot,
    ) -> Result<Vec<SourcedEntity>, ResolutionError> {
        resolve_snapshot(snapshot)
    }
}
