//! Backend-neutral cursor ordering for identifier payload pagination.

use crate::index::ProjectionCursor;

/// Whether a scroll cursor strictly advances its typed point ID.
pub(super) fn advances(previous: Option<&ProjectionCursor>, next: &ProjectionCursor) -> bool {
    match previous {
        None => true,
        Some(ProjectionCursor::Number(previous)) => {
            matches!(next, ProjectionCursor::Number(next) if next > previous)
        }
        Some(ProjectionCursor::Text(previous)) => {
            matches!(next, ProjectionCursor::Text(next) if next > previous)
        }
    }
}
