//! Fixed, actionable public diagnostics for typed knowledge health failures.

use super::check::Check;
use maestro_kernel::facts::{InputMismatchKind, PROJECTION_REBUILD_REPAIR};
use maestro_knowledge::graph::projection::health::ProbeError;
use std::path::Path;

/// A projection is disposable, but authority and artifacts must be preserved.
pub(super) const REBUILD: &str = PROJECTION_REBUILD_REPAIR;

/// Never expose raw native diagnostics (which can contain private paths).
pub(super) fn failed(path: &Path, error: &ProbeError) -> Check {
    let (problem, repair) = match error {
        ProbeError::NotActivated => (
            "graph.engine = ladybug is configured but not activated",
            "use a maestro with the frozen graphdb settings activation \
                handoff, or set graph.engine to none",
        ),
        ProbeError::InputMismatch(kind) => (
            match kind {
                InputMismatchKind::Settings => "published graph input pins mismatch (settings)",
                InputMismatchKind::Lock => "published graph input pins mismatch (lock)",
                InputMismatchKind::Resolution => "published graph input pins mismatch (resolution)",
                InputMismatchKind::Format => "published graph input pins mismatch (format)",
            },
            REBUILD,
        ),
        ProbeError::InvalidSettings => (
            "explicit graphdb settings are invalid",
            "fix graphdb.buffer_pool_size, graphdb.max_db_size and \
                graphdb.max_num_threads to the documented D14 ranges",
        ),
        ProbeError::CleanupInProgress => (
            "cleanup in progress",
            "let cleanup finish, then run `maestro doctor` again",
        ),
        ProbeError::Locked(_) => (
            "a writer holds the graph file's lock",
            "let the writer finish, then run `maestro doctor` again",
        ),
        ProbeError::GuardUnavailable => (
            "a permanent graph guard is missing or unsafe",
            "restore the private graph directory and regular guards, then run \
                `maestro setup --yes`",
        ),
        ProbeError::LockUnavailable => (
            "graph file locking is unavailable",
            "move the data directory to a filesystem supporting file locks, \
                then run `maestro setup --yes`",
        ),
        ProbeError::AuthorityMissing => (
            "the kernel authority is missing",
            "restore the kernel database from a backup, or add a collection \
                with `maestro knowledge collection add <collection.json>`",
        ),
        ProbeError::NeedsMigration => (
            "the kernel authority needs migration",
            "run a normal `maestro knowledge` command to migrate the kernel, \
                then run `maestro doctor`",
        ),
        ProbeError::NewerSchema => (
            "the kernel authority uses a newer schema",
            "use a maestro as new as the one that migrated the kernel",
        ),
        ProbeError::MissingReceipt => (
            "a published generation is missing its graph readiness receipt",
            REBUILD,
        ),
        ProbeError::MissingFile => ("the receipt-named graph file is missing", REBUILD),
        ProbeError::Stale => (
            "the graph file is stale: its identity or content differs from the receipt",
            REBUILD,
        ),
        ProbeError::Unreadable(_) => (
            "the graph file does not open read-only, corrupt or unreadable",
            REBUILD,
        ),
        ProbeError::Corrupt(_) => (
            "the graph file opens but does not answer or validate, likely corrupt",
            REBUILD,
        ),
        ProbeError::InventoryUnreadable => ("the projection receipt cannot be read", REBUILD),
    };
    Check::failed("graph", &path.display().to_string(), problem, repair)
}
