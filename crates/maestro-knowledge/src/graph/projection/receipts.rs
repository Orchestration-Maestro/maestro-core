//! Read-only kernel readiness inventory used by graph health probes.

use maestro_kernel::facts::{self, Error, InventoryState, ProjectionInventory};
use std::path::{Path, PathBuf};

/// A current receipt inventory or an unavailable kernel state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReceiptInventoryState {
    /// The kernel database does not exist.
    Missing,
    /// The existing kernel lacks migrations required by this binary.
    NeedsMigration(Vec<&'static str>),
    /// The database records a migration this binary does not carry.
    NewerSchema(String),
    /// Current published generations visible to the principal.
    Ready(Vec<ProjectionInventory>),
}

/// Replaceable read-only source for readiness checks; health never repairs the kernel.
pub(crate) trait ProjectionReceiptInventory {
    /// Open the kernel read-only and list readiness receipts visible to `principal`.
    fn inventory(&self, principal: &str) -> Result<ReceiptInventoryState, Error>;
}

/// Production receipt inventory backed by the existing kernel file.
#[derive(Debug, Clone)]
pub(crate) struct KernelProjectionReceipts {
    /// Kernel data directory supplied by the application.
    data: PathBuf,
}

impl KernelProjectionReceipts {
    /// Use the kernel data directory resolved by the application configuration.
    pub(crate) fn new(data: &Path) -> Self {
        Self {
            data: data.to_path_buf(),
        }
    }
}

impl ProjectionReceiptInventory for KernelProjectionReceipts {
    fn inventory(&self, principal: &str) -> Result<ReceiptInventoryState, Error> {
        match facts::projection_inventory_in(&self.data, principal)? {
            InventoryState::Missing => Ok(ReceiptInventoryState::Missing),
            InventoryState::NeedsMigration(names) => {
                Ok(ReceiptInventoryState::NeedsMigration(names))
            }
            InventoryState::NewerSchema(name) => Ok(ReceiptInventoryState::NewerSchema(name)),
            InventoryState::Ready(inventory) => Ok(ReceiptInventoryState::Ready(inventory)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_kernel::store::Database;
    use maestro_test_scratch::scratch_directory;
    use rusqlite::Connection;
    use std::cell::Cell;

    struct FakeReceipts {
        calls: Cell<usize>,
        result: ReceiptInventoryState,
    }

    impl ProjectionReceiptInventory for FakeReceipts {
        fn inventory(&self, _principal: &str) -> Result<ReceiptInventoryState, Error> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.result.clone())
        }
    }

    fn open_kernel(data: &Path) {
        drop(Database::open(&data.join("kernel.sqlite3"), &data.join("artifacts")).unwrap());
    }

    #[test]
    fn kernel_receipts_reports_missing_without_opening_a_kernel() {
        let data = scratch_directory().unwrap();
        let receipts = KernelProjectionReceipts::new(&data);
        assert_eq!(
            receipts.inventory("local").unwrap(),
            ReceiptInventoryState::Missing
        );
    }

    #[test]
    fn kernel_receipts_reports_pending_migration_without_writing() {
        let data = scratch_directory().unwrap();
        open_kernel(&data);
        Connection::open(data.join("kernel.sqlite3"))
            .unwrap()
            .execute(
                "DELETE FROM migrations WHERE name = '0019_graph_projection'",
                [],
            )
            .unwrap();
        let receipts = KernelProjectionReceipts::new(&data);
        assert_eq!(
            receipts.inventory("local").unwrap(),
            ReceiptInventoryState::NeedsMigration(vec!["0019_graph_projection"])
        );
    }

    #[test]
    fn kernel_receipts_reports_newer_schema_by_migration_name() {
        let data = scratch_directory().unwrap();
        open_kernel(&data);
        Connection::open(data.join("kernel.sqlite3"))
            .unwrap()
            .execute(
                "INSERT INTO migrations (name, applied_at) VALUES ('9999_future', 'now')",
                [],
            )
            .unwrap();
        let receipts = KernelProjectionReceipts::new(&data);
        assert_eq!(
            receipts.inventory("local").unwrap(),
            ReceiptInventoryState::NewerSchema("9999_future".to_owned())
        );
    }

    #[test]
    fn kernel_receipts_lists_ready_kernel_for_principal() {
        let data = scratch_directory().unwrap();
        open_kernel(&data);
        let receipts = KernelProjectionReceipts::new(&data);
        assert_eq!(
            receipts.inventory("local").unwrap(),
            ReceiptInventoryState::Ready(Vec::new())
        );
    }

    #[test]
    fn health_can_inject_a_readiness_inventory_without_opening_a_kernel() {
        let fake = FakeReceipts {
            calls: Cell::new(0),
            result: ReceiptInventoryState::Missing,
        };
        assert_eq!(
            fake.inventory("no-grants").unwrap(),
            ReceiptInventoryState::Missing
        );
        assert_eq!(fake.calls.get(), 1);
    }
}
