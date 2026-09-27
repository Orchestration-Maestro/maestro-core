//! Public identifier data and internal source-span candidates.

/// A recognized identifier family, ordered by explicit overlap precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Backtick contents, or a lowercase command with lowercase subcommands
    /// followed by one or more parameters.
    Command,
    /// Unix absolute, `./`, `../` or `~/` paths; Windows drive paths; or a
    /// filename with an alphabetic extension.
    Path,
    /// Lowercase `v` followed by digits, or digits with one to three dotted
    /// numeric suffixes.
    Version,
    /// A decimal number from 1 through 65535 after `port ` or `:`.
    Port,
    /// One to six capitals, an optional `-` or `_`, two to six digits and an
    /// optional capital.
    ErrorCode,
    /// `--long-flag`, `-f`, `NAME=value`, `$VAR` or `${VAR}`.
    Parameter,
}

/// An exact query span classified into one identifier family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier {
    /// The pattern family that claimed this span.
    pub family: Family,
    /// The exact source text of the recognized span.
    pub text: String,
}

/// A half-open byte span proposed by one identifier family.
#[derive(Debug, Clone, Copy)]
pub(super) struct Candidate {
    /// The family claiming the span.
    pub(super) family: Family,
    /// The first byte of the span in the normalized query.
    pub(super) start: usize,
    /// The byte immediately after the span in the normalized query.
    pub(super) end: usize,
}

/// Returns the fixed rank used to resolve overlapping family claims.
pub(super) fn priority(family: Family) -> u8 {
    match family {
        Family::Command => 0,
        Family::Path => 1,
        Family::Version => 2,
        Family::Port => 3,
        Family::ErrorCode => 4,
        Family::Parameter => 5,
    }
}

/// Whether two half-open source spans share any byte.
pub(super) fn overlaps(left: Candidate, right: Candidate) -> bool {
    left.start < right.end && right.start < left.end
}
