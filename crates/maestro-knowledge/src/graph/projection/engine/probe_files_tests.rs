//! Receipt/file binding, deterministic neighbours, and independent native lifetimes.

use super::{
    probe::{Probe, ProbeReceipt},
    probe_tests::guards,
    schema,
    tests::{Fixture, config, scope},
};
use crate::graph::projection::tests::contract;
use crate::graph::projection::{
    EngineSettings, ProjectionScope, content,
    health::{ProbeError, PublishedFile, Receipt},
    receipts::ProjectionReceiptInventory,
    writer::{ProjectionBackendReader as _, receipt_from_verification},
};
use lbug::Connection;
use maestro_filesystem::{ControlFile, LockMode, SystemFileLock};
use maestro_kernel::{
    artifact::Digest,
    facts::{Error, InventoryState, ProjectionInventory, ProjectionReceipt},
    scope::LOCAL,
};
use std::{cell::Cell, fs};

struct Inventory {
    calls: Cell<usize>,
    rows: InventoryState,
}
impl ProjectionReceiptInventory for Inventory {
    fn inventory(&self, principal: &str) -> Result<InventoryState, Error> {
        assert_eq!(principal, LOCAL);
        self.calls.set(self.calls.get() + 1);
        Ok(self.rows.clone())
    }
}

fn inventory(receipt: Option<ProjectionReceipt>) -> Inventory {
    Inventory {
        calls: Cell::new(0),
        rows: InventoryState::Ready(vec![ProjectionInventory {
            collection_id: scope().collection_id,
            generation_id: scope().generation_id,
            receipt,
        }]),
    }
}

fn receipt(fixture: &Fixture) -> ProjectionReceipt {
    {
        let database = fixture.writer();
        let connection = Connection::new(&database).unwrap();
        #[cfg(not(windows))]
        schema::create(&connection, &scope(), &contract::pins()).unwrap();
        #[cfg(windows)]
        schema::tests::install_reader_fixture(&connection, &scope(), &contract::pins());
        connection.query("CHECKPOINT").unwrap();
    }
    #[cfg(windows)]
    super::open::tests::private_windows_fixture(&fixture.path);
    let verification = super::reader::Reader::open(&fixture.root, "rows.lbdb", config(), &scope())
        .unwrap()
        .verification()
        .unwrap();
    let set = Digest::of(b"set");
    let name = content::basename(&scope(), &set).unwrap();
    fs::rename(fixture.path.join("rows.lbdb"), fixture.path.join(&name)).unwrap();
    receipt_from_verification(&scope(), set, name, &verification, &contract::pins()).unwrap()
}

fn files(fixture: &Fixture, inventory: &Inventory) -> Vec<super::probe::ProbeFile> {
    match Probe::receipt_with(&fixture.path, &SystemFileLock, inventory, Some(settings())).unwrap()
    {
        ProbeReceipt::Files(files) => files,
        ProbeReceipt::NonePublished => panic!("published inventory expected"),
    }
}

#[test]
fn graph_probe_independent_opens_validate_receipts_and_release_native_handles() {
    let fixture = Fixture::new();
    let root = guards(&fixture);
    let receipt = receipt(&fixture);
    let before = fs::read(fixture.path.join(&receipt.identity.file_name)).unwrap();
    let inventory = inventory(Some(receipt.clone()));
    let published = files(&fixture, &inventory);
    assert_eq!(inventory.calls.get(), 1);
    let exclusive = root.open_control(ControlFile::Access).unwrap();
    assert!(
        exclusive
            .lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .is_err()
    );
    super::open::tests::OPEN_CALLS.set(0);
    let interface: &dyn PublishedFile = &published[0];
    for _ in 0..2 {
        let open = interface.open_read_only().unwrap();
        open.query_one().unwrap();
        drop(open);
        // Windows rename refuses while a native handle is alive; verifies real drop.
        let away = fixture.path.join("closed.lbdb");
        fs::rename(interface.path(), &away).unwrap();
        fs::rename(&away, interface.path()).unwrap();
    }
    assert_eq!(
        super::open::tests::OPEN_CALLS.get(),
        2,
        "both opens construct a native database"
    );
    let open = published[0].open_native().unwrap();
    drop(published);
    assert!(
        exclusive
            .lock_with(&SystemFileLock, LockMode::Exclusive, false)
            .is_err()
    );
    drop(open);
    exclusive
        .lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    assert_eq!(
        fs::read(fixture.path.join(&receipt.identity.file_name)).unwrap(),
        before
    );
}

#[test]
fn graph_probe_missing_stale_and_corrupt_have_valid_neighbour() {
    let fixture = Fixture::new();
    let _root = guards(&fixture);
    let receipt = receipt(&fixture);
    let neighbour = fs::read(fixture.path.join(&receipt.identity.file_name)).unwrap();
    let mut rows = inventory(Some(receipt.clone()));
    let mut missing = receipt.clone();
    missing.identity.generation_id += 1;
    missing.identity.file_name = content::basename(
        &ProjectionScope {
            collection_id: missing.identity.collection_id.clone(),
            generation_id: missing.identity.generation_id,
        },
        &missing.identity.claim_set_id,
    )
    .unwrap();
    rows.rows = InventoryState::Ready(vec![
        ProjectionInventory {
            collection_id: receipt.identity.collection_id.clone(),
            generation_id: receipt.identity.generation_id,
            receipt: Some(receipt.clone()),
        },
        ProjectionInventory {
            collection_id: missing.identity.collection_id.clone(),
            generation_id: missing.identity.generation_id,
            receipt: Some(missing.clone()),
        },
    ]);
    let neighbour_files = files(&fixture, &inventory(Some(receipt.clone())));
    neighbour_files[0]
        .open_read_only()
        .unwrap()
        .query_one()
        .unwrap();
    super::open::tests::OPEN_CALLS.set(0);
    assert_eq!(
        Probe::receipt_with(&fixture.path, &SystemFileLock, &rows, None).err(),
        Some(ProbeError::MissingFile)
    );
    assert_eq!(super::open::tests::OPEN_CALLS.get(), 0);
    assert_eq!(
        Probe::receipt_with(&fixture.path, &SystemFileLock, &rows, Some(settings())).err(),
        Some(ProbeError::MissingFile)
    );
    drop(neighbour_files);
    fs::write(
        fixture.path.join(&missing.identity.file_name),
        b"not a graph",
    )
    .unwrap();
    let published = files(&fixture, &rows);
    assert!(matches!(
        published[1].open_read_only().err(),
        Some(ProbeError::Unreadable(_))
    ));
    published[0].open_read_only().unwrap().query_one().unwrap();
    drop(published);
    // A valid graph for a different generation is stale, not a missing file.
    fs::write(fixture.path.join(&missing.identity.file_name), &neighbour).unwrap();
    let published = files(&fixture, &rows);
    assert!(matches!(
        published[1].open_read_only().unwrap().query_one(),
        Err(ProbeError::Corrupt(_))
    ));
    drop(published);
    let mut stale = receipt.clone();
    stale.identity.content_digest = Digest::of(b"stale");
    let published = files(&fixture, &inventory(Some(stale)));
    let stale_file: &dyn PublishedFile = &published[0];
    assert_eq!(
        stale_file.open_read_only().unwrap().query_one(),
        Err(ProbeError::Stale)
    );
    assert_eq!(
        fs::read(fixture.path.join(&receipt.identity.file_name)).unwrap(),
        neighbour
    );
}

#[test]
fn graph_probe_refuses_receipt_scope_and_filename_mismatches_and_absent_readiness() {
    let fixture = Fixture::new();
    let _root = guards(&fixture);
    let receipt = receipt(&fixture);
    assert_eq!(
        Probe::receipt_with(
            &fixture.path,
            &SystemFileLock,
            &inventory(None),
            Some(settings())
        )
        .err(),
        Some(ProbeError::MissingReceipt)
    );
    for change in 0..3 {
        let mut rows = inventory(Some(receipt.clone()));
        let InventoryState::Ready(entries) = &mut rows.rows else {
            panic!("ready inventory expected");
        };
        let stored = entries[0].receipt.as_mut().unwrap();
        match change {
            0 => stored.identity.file_name = "../outside.lbdb".into(),
            1 => stored.identity.collection_id = "other".into(),
            _ => stored.identity.generation_id += 1,
        }
        assert_eq!(
            Probe::receipt_with(&fixture.path, &SystemFileLock, &rows, Some(settings())).err(),
            Some(ProbeError::Stale)
        );
    }
}

#[test]
fn graph_probe_guard_failure_makes_zero_inventory_calls() {
    let fixture = Fixture::new();
    let root = guards(&fixture);
    let held = root.open_control(ControlFile::Access).unwrap();
    held.lock_with(&SystemFileLock, LockMode::Exclusive, false)
        .unwrap();
    let rows = inventory(None);
    assert_eq!(
        Probe::receipt_with(&fixture.path, &SystemFileLock, &rows, Some(settings())).err(),
        Some(ProbeError::CleanupInProgress)
    );
    assert_eq!(rows.calls.get(), 0);
}

#[test]
fn graph_probe_all_inventory_authority_states_remain_distinct() {
    let fixture = Fixture::new();
    let _root = guards(&fixture);
    for (rows, expected) in [
        (InventoryState::Missing, ProbeError::AuthorityMissing),
        (
            InventoryState::NeedsMigration(vec!["0019_graph_projection"]),
            ProbeError::NeedsMigration,
        ),
        (
            InventoryState::NewerSchema("future".into()),
            ProbeError::NewerSchema,
        ),
    ] {
        let inventory = Inventory {
            calls: Cell::new(0),
            rows,
        };
        assert_eq!(
            Probe::receipt_with(&fixture.path, &SystemFileLock, &inventory, Some(settings())).err(),
            Some(expected)
        );
        assert_eq!(inventory.calls.get(), 1);
    }
}

fn settings() -> EngineSettings {
    EngineSettings::new(
        16 * 1024 * 1024,
        64 * 1024 * 1024,
        1,
        Digest::of(b"frozen-lock"),
    )
    .unwrap()
}

#[test]
fn graph_probe_unactivated_with_published_receipts_makes_zero_native_calls() {
    let fixture = Fixture::new();
    let _root = guards(&fixture);
    let stored = receipt(&fixture);
    let before = fs::read(fixture.path.join(&stored.identity.file_name)).unwrap();
    let rows = inventory(Some(stored.clone()));
    super::open::tests::OPEN_CALLS.set(0);
    assert_eq!(
        Probe::receipt_with(&fixture.path, &SystemFileLock, &rows, None).err(),
        Some(ProbeError::NotActivated)
    );
    assert_eq!(rows.calls.get(), 1);
    assert_eq!(super::open::tests::OPEN_CALLS.get(), 0);
    assert_eq!(
        fs::read(fixture.path.join(&stored.identity.file_name)).unwrap(),
        before
    );
}

#[test]
fn graph_probe_converts_real_native_receipts_to_shared_health_ports() {
    assert!(matches!(
        ProbeReceipt::NonePublished.into_receipt(),
        Receipt::NonePublished
    ));
    let fixture = Fixture::new();
    let _guards = guards(&fixture);
    let expected = receipt(&fixture);
    let rows = inventory(Some(expected.clone()));
    let receipt = Probe::receipt_with(&fixture.path, &SystemFileLock, &rows, Some(settings()))
        .unwrap()
        .into_receipt();
    assert_eq!(format!("{receipt:?}"), "Receipt::Files(1)");
    let Receipt::Files(files) = receipt else {
        panic!("expected files")
    };
    assert_eq!(
        files[0].path(),
        fixture.path.join(expected.identity.file_name)
    );
    files[0].open_read_only().unwrap().query_one().unwrap();
}
