//! Internal algorithms for assembling authoritative, bounded evidence.

/// Counts compact serialized passages with the selected counter.
mod budget;
/// Detects explicit structured disagreements in candidate tables.
pub(super) mod conflicts;
/// Computes case-sensitive shingles and material-difference signatures.
mod features;
/// Derives canonical section extents and sibling-safe windows.
mod sections;
/// Orders passages and derives trace metadata and known gaps.
mod signals;
/// Unions candidate source spans without crossing revisions.
mod spans;
/// Exercises the helper contracts with canonical source fixtures.
#[cfg(test)]
mod tests;
/// Compares numeric version components without integer conversion.
mod versions;
