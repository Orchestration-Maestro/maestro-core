//! Authoritative section reads and bounded evidence assembly.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "assembly helpers are wired by the next T032 commit"
    )
)]

/// Counts compact serialized passages with the selected counter.
mod budget;
/// Detects explicit structured disagreements in candidate tables.
pub(super) mod conflicts;
/// Computes case-sensitive shingles and material-difference signatures.
mod features;
/// Reads one authorized canonical section from a completed chunk set.
mod section_reader;
/// Derives canonical section extents and sibling-safe evidence windows.
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

pub use section_reader::{SectionExcerpt, SectionReadError, read_section};
