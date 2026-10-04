//! Application health ports and typed failures, independent of the optional engine.

use super::port::InputMismatchKind;
use std::{fmt, path::Path};

/// Typed health failure; raw native diagnostics never become public CLI text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    /// The permanent access guard is exclusively held by cleanup.
    CleanupInProgress,
    /// Frozen backend settings have not been activated; no native call was made.
    NotActivated,
    /// Durable settings or complete lock differ from current admission.
    InputMismatch(InputMismatchKind),
    /// Explicit native settings violate the approved D14 ranges.
    InvalidSettings,
    /// A writer guard or an observed native lock diagnostic refused access.
    Locked(String),
    /// A setup-created control is missing or unsafe.
    GuardUnavailable,
    /// File locking is unsupported or otherwise refused.
    LockUnavailable,
    /// No existing kernel authority is available.
    AuthorityMissing,
    /// The existing kernel needs migration by a normal command.
    NeedsMigration,
    /// This binary cannot read the newer kernel schema.
    NewerSchema,
    /// The current published generation has no readiness receipt.
    MissingReceipt,
    /// The receipt-named graph file is absent.
    MissingFile,
    /// The physical identity or content differs from the immutable receipt.
    Stale,
    /// Native construction failed for an unclassified reason.
    Unreadable(String),
    /// An open graph failed its query or schema/content validation.
    Corrupt(String),
    /// Receipt authority could not be decoded or read.
    InventoryUnreadable,
}

/// Current scoped files named by kernel readiness receipts, not a directory scan.
pub trait PublishedGraph {
    /// What the receipt publishes.
    ///
    /// # Errors
    ///
    /// Why the receipt cannot be read.
    fn receipt(&self) -> Result<Receipt<'_>, ProbeError>;
}

/// What a projection receipt publishes.
pub enum Receipt<'a> {
    /// No graph is published yet: there is nothing to open.
    NonePublished,
    /// These files, in the receipt's order.
    Files(Vec<Box<dyn PublishedFile + 'a>>),
}

/// A graph file a receipt published.
pub trait PublishedFile {
    /// Its path, which the check requires inside the graph's directory.
    fn path(&self) -> &Path;

    /// Opens it read-only with the engine, following no link.
    ///
    /// # Errors
    ///
    /// Typed lock, missing, stale or unreadable refusal from the knowledge adapter.
    fn open_read_only(&self) -> Result<Box<dyn OpenGraph + '_>, ProbeError>;
}

/// A published graph file, open read-only until dropped.
pub trait OpenGraph {
    /// Runs the smallest query, `RETURN 1`.
    ///
    /// # Errors
    ///
    /// Typed schema/content or query validation failure.
    fn query_one(&self) -> Result<(), ProbeError>;
}

impl fmt::Debug for Receipt<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonePublished => formatter.write_str("Receipt::NonePublished"),
            Self::Files(files) => formatter
                .debug_tuple("Receipt::Files")
                .field(&files.len())
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file with a private diagnostic location and an opaque native opener.
    struct PrivateFile;
    impl PublishedFile for PrivateFile {
        fn path(&self) -> &Path {
            Path::new("private-health-path-marker")
        }
        fn open_read_only(&self) -> Result<Box<dyn OpenGraph + '_>, ProbeError> {
            panic!("Debug must not open a native handle")
        }
    }

    #[test]
    fn receipt_debug_names_type_and_redacts_published_paths_and_handles() {
        let receipt = Receipt::Files(vec![Box::new(PrivateFile)]);
        let debug = format!("{receipt:?}");
        assert_eq!(debug, "Receipt::Files(1)");
        assert!(!debug.contains("private-health-path-marker"));
        assert_eq!(
            format!("{:?}", Receipt::NonePublished),
            "Receipt::NonePublished"
        );
    }
}
