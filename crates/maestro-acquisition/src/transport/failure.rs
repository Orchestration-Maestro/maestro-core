//! Content-free transport and decode refusal contract.
use crate::{Refusal, policy::authority::AuthorityRefusal};

/// Content-free failures; no source URL, credentials or partial bytes escape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Current policy/URL/address controls refused this hop.
    Admission(Refusal),
    /// Current N05 grant was refused or expired.
    Authority(AuthorityRefusal),
    /// Invalid effective settings, including fewer than five robots redirects.
    Configuration,
    /// The same canonical identity was visited twice.
    RedirectLoop,
    /// The composed redirect ceiling was reached.
    RedirectLimit,
    /// The cumulative request ceiling was reached.
    Requests,
    /// Shared pacing refused concurrency or finite budgets; no dial happened.
    Pacing(super::pacing::Pending),
    /// Connection/HTTP work failed; eligible for a bounded freshly admitted retry.
    Transport,
    /// Checked acquisition profile selected a different content transport.
    TransportMismatch,
    /// Owned connection and driver were dropped on deadline.
    Timeout,
    /// Authentication is required; no automatic credential escalation.
    Authentication,
    /// Forbidden/challenge response; no challenge-solving fallback.
    Challenge,
    /// Truncated or unsolicited range response; no partial promotion.
    Partial,
    /// Unknown, malformed or incomplete content encoding.
    Content,
    /// Encoded wire-byte ceiling, distinct from expanded bytes.
    EncodedBytes,
    /// Cumulative bytes produced by all decode stages exceeded the ceiling.
    ExpandedBytes,
    /// Cumulative expanded/wire ratio exceeded the ceiling.
    ExpansionRatio,
    /// Decoder workspace or retained bytes exceed the memory ceiling.
    Memory,
    /// Content encoding layers exceed the nested decode ceiling.
    Nesting,
    /// Cumulative gzip/deflate/container members exceed the decode ceiling.
    Members,
    /// XML entity expansion is disabled by the protected policy.
    Entities,
    /// Cumulative decoded image pixels exceeded the ceiling.
    Pixels,
    /// Owned parser crashed; partial output must not be promoted.
    ParserCrash,
}

/// Precise stage retained on parser-IPC preflight refusals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodeStage {
    /// HTTP content-encoding expansion.
    Http,
    /// Office container decoding.
    Office,
    /// PDF stream decoding.
    Pdf,
    /// Nested attachment/archive expansion.
    Attachment,
    /// Image pixel decoding.
    Image,
}
