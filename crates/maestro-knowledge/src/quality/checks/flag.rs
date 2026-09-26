//! What a check says of a revision it flags: the rule, the outcome the rule
//! gives, and why, in words that count and name but quote nothing of the
//! document.

use maestro_kernel::document::Outcome;

/// A check's finding on a revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quality) struct Flag {
    /// The rule that flags it, such as `body.near-empty`.
    pub(in crate::quality) rule: &'static str,
    /// The outcome the rule gives it.
    pub(in crate::quality) outcome: Outcome,
    /// Why, for people.
    pub(in crate::quality) reason: String,
}

/// `count` things named `thing`, the name made plural with an `s` unless
/// there is one: `1 word`, `5 words`.
pub(super) fn counted(count: u64, thing: &str) -> String {
    if count == 1 {
        format!("1 {thing}")
    } else {
        format!("{count} {thing}s")
    }
}

/// `count` as a `u64`, which every count of a document fits.
pub(super) fn number(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}
