//! Shared classification of graph authority failures.

use crate::failure::Failure;
use maestro_kernel::facts;

/// A refusal for a claim set the kernel refused as input, exit 2; a
/// failure, exit 1, when its store failed or a stored original no longer
/// holds what knowledge located in it.
pub(super) fn claim_failure(error: &facts::Error) -> Failure {
    match error {
        facts::Error::Store(_)
        | facts::Error::Job(_)
        | facts::Error::DigestMismatch { .. }
        | facts::Error::SpanOutOfRange { .. }
        | facts::Error::SpanOffBoundary { .. }
        | facts::Error::QuoteMismatch { .. } => Failure::failed_by(error),
        facts::Error::Unauthorized
        | facts::Error::Invalid(_)
        | facts::Error::UnknownBuild(_)
        | facts::Error::Unfinished { .. }
        | facts::Error::OverBudget { .. }
        | facts::Error::Conflict(_)
        | facts::Error::UnknownRevision { .. }
        | facts::Error::IneligibleRevision { .. } => Failure::refused_by(error),
    }
}
