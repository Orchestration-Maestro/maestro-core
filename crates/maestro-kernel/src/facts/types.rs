//! What a claim says, the source locations that support it, and the records
//! the kernel keeps of both.

use crate::{artifact::Digest, evidence::Span};
use std::collections::BTreeMap;

/// Claims of one collection to admit together, in order: all of them, or
/// none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimSet {
    /// The collection every claim belongs to.
    pub collection_id: String,
    /// The claims, in the order the set keeps them.
    pub claims: Vec<Claim>,
}

/// A qualified assertion about an entity, as a rule or an extractor states
/// it, with the source locations that support it. Its content is its
/// identity: the same claim is recorded once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    /// What it is about.
    pub subject: EntityName,
    /// What it says of its subject.
    pub predicate: Predicate,
    /// The value it gives, as the source writes it.
    pub object: Literal,
    /// The conditions it holds under, by name, as the source states them:
    /// data, never expressions. Empty when it states none.
    pub conditions: BTreeMap<String, String>,
    /// The product versions it holds for.
    pub version: Validity,
    /// When it holds in the world.
    pub world: Validity,
    /// What produced it.
    pub provenance: Provenance,
    /// The source locations that support it: at least one, none twice.
    pub supports: Vec<Support>,
}

/// How a claim names an entity: its kind and its exact source spelling.
/// Which entity that spelling resolves to is decided elsewhere; `Entity` is
/// left for that resolved record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityName {
    /// Its kind, such as `Parameter`.
    pub kind: String,
    /// Its name, spelled as the source spells it.
    pub name: String,
}

/// What a claim says of its subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Predicate {
    /// The subject's default is the object, a literal.
    DefaultsTo,
}

/// A typed literal, its lexeme unchanged from the source: no number is
/// rewritten and no unit guessed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Literal {
    /// Its type.
    pub kind: LiteralKind,
    /// Its text, as the source writes it.
    pub lexeme: String,
}

/// The type of a [`Literal`], which its lexeme must have the form of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralKind {
    /// Any text.
    Text,
    /// `true` or `false`.
    Boolean,
    /// Decimal digits, after an optional `-`.
    Integer,
    /// Decimal digits, a `.` and decimal digits, after an optional `-`.
    Decimal,
}

/// Where a claim holds, as far as its source says: unknown, or between
/// bounds, the start included and the end excluded, an absent bound open.
/// Unknown is never replaced by a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validity {
    /// The source does not say.
    Unknown,
    /// Between these bounds.
    Bounded {
        /// The first value it holds for, if there is one.
        start: Option<String>,
        /// The first value it no longer holds for, if there is one.
        end: Option<String>,
    },
}

/// What produced a claim: the rule or the extractor, and the digest of the
/// profile it ran under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// The rule or the extractor, such as `synthetic-defaults/1`.
    pub extractor: String,
    /// The digest of its rule or profile.
    pub profile: Digest,
}

/// A source location that supports a claim: a half-open UTF-8 byte span of
/// one revision's original Markdown, inside one canonical block, with the
/// SHA-256 of its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Support {
    /// The revision whose original Markdown holds the quote.
    pub revision_id: String,
    /// The canonical block that holds it.
    pub block_id: String,
    /// Where it lies in the original Markdown.
    pub span: Span,
    /// The SHA-256 of the bytes of the span.
    pub quote_digest: Digest,
}

/// Where a person's review of a claim stands. A valid quote never moves it:
/// every claim is admitted unreviewed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewState {
    /// Nobody reviewed it yet.
    Unreviewed,
    /// A reviewer accepted it.
    Accepted,
    /// A reviewer rejected it.
    Rejected,
    /// A reviewer flagged it for a ruling.
    Flagged,
}

/// A claim as the kernel records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimRecord {
    /// The SHA-256 of its content and collection: its application id.
    pub id: Digest,
    /// Its collection.
    pub collection_id: String,
    /// Its content, its supports in their recorded order.
    pub claim: Claim,
    /// Its review state.
    pub review: ReviewState,
    /// When the kernel first recorded it.
    pub recorded_at: String,
}

/// A claim set as the kernel records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimSetRecord {
    /// The SHA-256 of its collection and its ordered claim ids.
    pub id: Digest,
    /// Its collection.
    pub collection_id: String,
    /// Its claims, in order.
    pub claims: Vec<ClaimRecord>,
}

impl Predicate {
    /// Its name, as the `predicate` column holds it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DefaultsTo => "DEFAULTS_TO",
        }
    }

    /// The predicate named `name`.
    pub(super) fn parse(name: &str) -> Option<Self> {
        (name == "DEFAULTS_TO").then_some(Self::DefaultsTo)
    }
}

impl LiteralKind {
    /// Its name, as the `object_type` column holds it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Decimal => "decimal",
        }
    }

    /// The type named `name`, as a table's type column or the
    /// `object_type` column spells it.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "text" => Some(Self::Text),
            "boolean" => Some(Self::Boolean),
            "integer" => Some(Self::Integer),
            "decimal" => Some(Self::Decimal),
            _ => None,
        }
    }

    /// Whether `lexeme` has the form of this type: the check a claim's
    /// object passes before it is admitted.
    #[must_use]
    pub fn admits(self, lexeme: &str) -> bool {
        let digits =
            |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
        let unsigned = lexeme.strip_prefix('-').unwrap_or(lexeme);
        match self {
            Self::Text => true,
            Self::Boolean => matches!(lexeme, "true" | "false"),
            Self::Integer => digits(unsigned),
            Self::Decimal => unsigned
                .split_once('.')
                .is_some_and(|(whole, fraction)| digits(whole) && digits(fraction)),
        }
    }
}

impl ReviewState {
    /// Its name, as the `review_state` column holds it.
    pub(super) fn parse(name: &str) -> Option<Self> {
        match name {
            "unreviewed" => Some(Self::Unreviewed),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            "flagged" => Some(Self::Flagged),
            _ => None,
        }
    }
}
