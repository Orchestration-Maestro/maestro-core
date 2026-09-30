//! Read-only kernel readiness inventory used by graph health probes.

use maestro_kernel::{
    facts::ProjectionInventory,
    scope::ScopeSet,
    store::{HealthOpen, open_health_in},
};
use std::path::{Path, PathBuf};

/// A current receipt inventory or an unavailable kernel state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReceiptInventoryState {
    /// The kernel database does not exist.
    Missing,
    /// The existing kernel lacks migrations required by this binary.
    NeedsMigration,
    /// Current published generations visible to the requested scopes.
    Ready(Vec<ProjectionInventory>),
}

/// Replaceable read-only source for readiness checks; health never repairs the kernel.
pub(crate) trait ProjectionReceiptInventory {
    /// Open the kernel read-only and list current scoped readiness receipts.
    fn inventory(&self, scopes: &ScopeSet) -> Result<ReceiptInventoryState, String>;
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
    fn inventory(&self, scopes: &ScopeSet) -> Result<ReceiptInventoryState, String> {
        match open_health_in(&self.data).map_err(|error| error.to_string())? {
            HealthOpen::Missing => Ok(ReceiptInventoryState::Missing),
            HealthOpen::NeedsMigration(_) => Ok(ReceiptInventoryState::NeedsMigration),
            HealthOpen::Ready(kernel) => kernel
                .current_projection_inventory(scopes)
                .map(ReceiptInventoryState::Ready)
                .map_err(|error| error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_kernel::store::Database;
    use maestro_test_scratch::scratch_directory;
    use std::cell::Cell;

    struct FakeReceipts {
        calls: Cell<usize>,
        result: ReceiptInventoryState,
    }

    impl ProjectionReceiptInventory for FakeReceipts {
        fn inventory(&self, _scopes: &ScopeSet) -> Result<ReceiptInventoryState, String> {
            self.calls.set(self.calls.get() + 1);
            Ok(self.result.clone())
        }
    }

    #[test]
    fn health_can_inject_a_readiness_inventory_without_opening_a_kernel() {
        let fake = FakeReceipts {
            calls: Cell::new(0),
            result: ReceiptInventoryState::Missing,
        };
        let data = scratch_directory().unwrap();
        let kernel = Database::open_in(&data).unwrap();
        let scopes = kernel.visible("no-grants").unwrap();
        assert_eq!(
            fake.inventory(&scopes).unwrap(),
            ReceiptInventoryState::Missing
        );
        assert_eq!(fake.calls.get(), 1);
    }
}
