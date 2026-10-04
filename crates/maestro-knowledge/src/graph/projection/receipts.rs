//! Read-only kernel readiness inventory used by graph health probes.

use maestro_kernel::{
    facts::{self, Error, InventoryState},
    scope::Config,
};
use std::path::{Path, PathBuf};

/// Replaceable read-only source for readiness checks; health never repairs the kernel.
pub(crate) trait ProjectionReceiptInventory {
    /// Open the kernel read-only and list readiness receipts visible to `principal`.
    fn inventory(&self, principal: &str) -> Result<InventoryState, Error>;
}

/// Production receipt inventory backed by the existing kernel file.
#[derive(Debug, Clone)]
pub(crate) struct KernelProjectionReceipts {
    /// Kernel data directory supplied by the application.
    data: PathBuf,
    /// Current local read policy; absent for stored-principal inventory tests.
    config: Option<Config>,
}

impl KernelProjectionReceipts {
    /// Health evaluates local configuration without persisting grants.
    pub(crate) fn configured(data: &Path, config: &Config) -> Self {
        Self {
            data: data.to_path_buf(),
            config: Some(config.clone()),
        }
    }

    /// Use the kernel data directory resolved by the application configuration.
    pub(crate) fn new(data: &Path) -> Self {
        Self {
            data: data.to_path_buf(),
            config: None,
        }
    }
}

impl ProjectionReceiptInventory for KernelProjectionReceipts {
    fn inventory(&self, principal: &str) -> Result<InventoryState, Error> {
        match &self.config {
            Some(config) => facts::projection_inventory_with_config(&self.data, config),
            None => facts::projection_inventory_in(&self.data, principal),
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
        result: InventoryState,
    }

    impl ProjectionReceiptInventory for FakeReceipts {
        fn inventory(&self, _principal: &str) -> Result<InventoryState, Error> {
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
            InventoryState::Missing
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
            InventoryState::NeedsMigration(vec!["0019_graph_projection"])
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
            InventoryState::NewerSchema("9999_future".to_owned())
        );
    }

    #[test]
    fn kernel_receipts_lists_ready_kernel_for_principal() {
        let data = scratch_directory().unwrap();
        open_kernel(&data);
        let receipts = KernelProjectionReceipts::new(&data);
        assert_eq!(
            receipts.inventory("local").unwrap(),
            InventoryState::Ready(Vec::new())
        );
    }

    #[test]
    fn health_can_inject_a_readiness_inventory_without_opening_a_kernel() {
        let fake = FakeReceipts {
            calls: Cell::new(0),
            result: InventoryState::Missing,
        };
        assert_eq!(
            fake.inventory("no-grants").unwrap(),
            InventoryState::Missing
        );
        assert_eq!(fake.calls.get(), 1);
    }
    #[test]
    fn configured_health_inventory_uses_current_config_not_stored_grants() {
        use maestro_kernel::{
            document::Collection,
            generation::NewGeneration,
            scope::{LOCAL, Right},
        };
        use std::collections::BTreeMap;
        let data = scratch_directory().unwrap();
        let database = Database::open_in(&data).unwrap();
        database
            .record_collection(&Collection {
                id: "visible".into(),
                title: "Visible".into(),
                visibility: "private".into(),
                profiles: BTreeMap::new(),
            })
            .unwrap();
        Connection::open(data.join("kernel.sqlite3")).unwrap().execute_batch(
            "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id, state)
             VALUES ('set', 'visible', 'structural/1', 'native', 'building');
             UPDATE chunk_sets SET state = 'complete', manifest_digest =
             '4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945' WHERE id = 'set';"
        ).unwrap();
        let generation = database
            .create_generation(&NewGeneration {
                collection_id: "visible".into(),
                chunk_set_id: "set".into(),
                embedding_profile: "embed:test".into(),
                sparse_profile: "bm25-en-fr/1".into(),
            })
            .unwrap()
            .id;
        database.verify_generation(generation, 0).unwrap();
        database.publish_generation(generation).unwrap();
        let config: Config = "[access]\nread = ['workspace/default/collection/visible']"
            .parse()
            .unwrap();
        let configured = KernelProjectionReceipts::configured(&data, &config);
        let InventoryState::Ready(rows) = configured.inventory(LOCAL).unwrap() else {
            panic!("current kernel expected");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].generation_id, generation);
        assert_eq!(rows[0].receipt, None);
        assert!(database.visible(LOCAL).unwrap().is_empty());
        database
            .grant(
                LOCAL,
                &"workspace/default".parse().unwrap(),
                Right::Read,
                "test",
            )
            .unwrap();
        assert_eq!(
            KernelProjectionReceipts::configured(&data, &Config::default())
                .inventory(LOCAL)
                .unwrap(),
            InventoryState::Ready(Vec::new())
        );
        assert_eq!(
            KernelProjectionReceipts::new(&data)
                .inventory(LOCAL)
                .unwrap(),
            InventoryState::Ready(rows)
        );
    }
}
