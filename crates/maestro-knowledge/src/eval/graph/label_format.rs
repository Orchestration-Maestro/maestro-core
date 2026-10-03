//! Closed deserialization model for the graph-label JSONL format.

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// One strict JSONL label.
pub(super) struct Label {
    /// Versioned schema identifier.
    pub(super) schema: String,
    /// Suite question ID.
    pub(super) id: String,
    /// Independent question family.
    pub(super) family: String,
    /// Question category.
    pub(super) kind: String,
    /// Suite language code.
    pub(super) language: String,
    /// Accepted complete alternatives.
    pub(super) proofs: Vec<Proof>,
    #[serde(default)]
    /// Why the source cannot answer this item.
    pub(super) unanswerable_reason: Option<String>,
    /// Independent review record.
    pub(super) review: Option<Review>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// One complete proof alternative.
pub(super) struct Proof {
    /// All required relations.
    pub(super) links: Vec<Link>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// One required relation in a proof.
pub(super) struct Link {
    /// Relation subject.
    pub(super) subject: String,
    /// Closed vocabulary predicate.
    pub(super) predicate: String,
    /// Relation object.
    pub(super) object: String,
    #[serde(default)]
    /// Required conditions.
    pub(super) conditions: Vec<String>,
    #[serde(default)]
    /// Version qualifier.
    pub(super) version: Option<String>,
    /// Required source anchors.
    pub(super) anchors: Vec<Anchor>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// Digest-bound original source slice.
pub(super) struct Anchor {
    /// Authority source reference.
    pub(super) source_ref: String,
    /// Original content digest.
    pub(super) original: String,
    /// Half-open byte offsets.
    pub(super) span: [usize; 2],
    /// Exact quote digest.
    pub(super) quote: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
/// Independent review and optional owner ruling.
pub(super) struct Review {
    /// Independent reviewer identity.
    pub(super) reviewer_card: String,
    /// Review disposition.
    pub(super) disposition: Disposition,
    #[serde(default)]
    /// Owner ruling for a disputed label.
    pub(super) owner: Option<OwnerRuling>,
}

/// The independent review either accepts the claim or flags it for the owner.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Disposition {
    /// Independently accepted claim.
    Accepted,
    /// Claim needs an owner ruling.
    Flagged,
}

/// Explicit owner resolution of a flagged claim.
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum OwnerRuling {
    /// Owner accepts the claim.
    Accepted,
    /// Owner rejects the claim.
    Rejected,
}
