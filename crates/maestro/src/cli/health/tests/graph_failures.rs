//! Every typed refusal has an actionable, path-safe public diagnosis.

use super::super::{check::Outcome, graph_failure::failed};
use maestro_kernel::facts::InputMismatchKind;
use maestro_knowledge::graph::projection::health::ProbeError;
use std::path::Path;

#[test]
fn graph_failures_are_distinct_actionable_and_never_expose_native_diagnostics() {
    let private = "private native path containing lock";
    let cases = [
        (
            ProbeError::NotActivated,
            "configured but not activated",
            "activation handoff",
        ),
        (
            ProbeError::InputMismatch(InputMismatchKind::Settings),
            "input pins mismatch",
            "maestro knowledge graph rebuild",
        ),
        (
            ProbeError::InvalidSettings,
            "settings are invalid",
            "D14 ranges",
        ),
        (
            ProbeError::CleanupInProgress,
            "cleanup in progress",
            "cleanup finish",
        ),
        (
            ProbeError::Locked(private.into()),
            "a writer holds",
            "writer finish",
        ),
        (
            ProbeError::GuardUnavailable,
            "guard is missing or unsafe",
            "setup --yes",
        ),
        (
            ProbeError::LockUnavailable,
            "locking is unavailable",
            "supporting file locks",
        ),
        (
            ProbeError::AuthorityMissing,
            "authority is missing",
            "collection add",
        ),
        (
            ProbeError::NeedsMigration,
            "needs migration",
            "normal `maestro knowledge`",
        ),
        (ProbeError::NewerSchema, "newer schema", "as new as"),
        (
            ProbeError::MissingReceipt,
            "missing its graph readiness receipt",
            "rebuild",
        ),
        (ProbeError::MissingFile, "file is missing", "rebuild"),
        (ProbeError::Stale, "file is stale", "rebuild"),
        (
            ProbeError::Unreadable(private.into()),
            "corrupt or unreadable",
            "rebuild",
        ),
        (
            ProbeError::Corrupt(private.into()),
            "likely corrupt",
            "rebuild",
        ),
        (
            ProbeError::InventoryUnreadable,
            "receipt cannot be read",
            "maestro knowledge graph rebuild",
        ),
    ];
    for (error, diagnosis, repair) in cases {
        let check = failed(Path::new("graph"), &error);
        let Outcome::Failed { problem, next } = check.outcome else {
            panic!("refusal expected");
        };
        assert!(problem.contains(diagnosis), "{error:?}: {problem}");
        assert!(next.contains(repair), "{error:?}: {next}");
        assert!(!problem.contains(private));
        assert!(!next.contains(private));
    }
}
