//! The token budgets drafts grow toward and never exceed.

/// The size a draft grows toward: once combined drafts reach this many tokens, the draft is
/// complete.
pub(crate) const TARGET_TOKENS: usize = 500;
/// The hard limit on a prepared input's tokens; larger content splits and is never clipped.
pub(crate) const MAX_TOKENS: usize = 700;
