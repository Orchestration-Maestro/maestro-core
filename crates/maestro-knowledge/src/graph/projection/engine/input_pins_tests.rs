//! Cold native opens bind durable stamps, not just live process registry entries.
use super::{
    public_fixture::{Fixture, settings},
    public_tests::publish,
};
use crate::graph::projection::{
    EngineSettings, InputMismatchKind, ProjectionEngine, ProjectionError, ProjectionFactory,
};
use crate::graph::projection::{health::ProbeError, receipts::ProjectionReceiptInventory};
use maestro_filesystem::SystemFileLock;
use maestro_kernel::{
    artifact::Digest,
    facts::{
        EXACT_RESOLVER_VERSION, Error as FactError, InventoryState, ProjectionInventory,
        ResolutionInput,
    },
};
#[cfg(unix)]
use std::fs;

#[test]
fn matching_cold_open_keeps_published_resolution_after_new_snapshot() {
    let fixture = Fixture::new();
    publish(&fixture);
    let receipt = fixture
        .authority
        .database
        .projection_ready(&fixture.authority.scopes, fixture.build.scope.generation_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        receipt.frozen_lock,
        *settings().frozen_lock(),
        "persisted digest equals admitted complete frozen lock"
    );
    assert_eq!(receipt.settings_identity, settings().identity());
    assert_eq!(receipt.resolution_id, fixture.build.resolution_id);
    let newer = fixture
        .authority
        .database
        .record_resolution(
            &fixture.authority.scopes,
            "builder",
            &ResolutionInput {
                resolver_version: EXACT_RESOLVER_VERSION.into(),
                sets: vec![fixture.build.claim_set_id.clone()],
                previous: Some(fixture.build.resolution_id.clone()),
                decisions: vec![],
            },
            &|_| Ok(()),
        )
        .unwrap();
    assert_ne!(newer.id, receipt.resolution_id);
    fixture
        .factory()
        .reader(
            &fixture.authority.database,
            &fixture.authority.scopes,
            fixture.build.scope.clone(),
        )
        .expect("matching receipt/native/admission pins must open");
}

#[test]
fn cold_open_and_health_refuse_changed_settings_and_complete_lock_before_native_open() {
    let fixture = Fixture::new();
    publish(&fixture);
    #[cfg(unix)]
    {
        let receipt = fixture
            .authority
            .database
            .projection_ready(&fixture.authority.scopes, fixture.build.scope.generation_id)
            .unwrap()
            .unwrap();
        fs::write(
            fixture.native.path.join(receipt.file_name),
            b"corrupt native bytes: must not open",
        )
        .unwrap();
    }
    for (kind, changed) in [
        (
            InputMismatchKind::Settings,
            EngineSettings::new(
                32 * 1024 * 1024,
                64 * 1024 * 1024,
                1,
                settings().frozen_lock().clone(),
            )
            .unwrap(),
        ),
        (
            InputMismatchKind::Lock,
            EngineSettings::new(
                16 * 1024 * 1024,
                64 * 1024 * 1024,
                1,
                Digest::of(b"changed lock"),
            )
            .unwrap(),
        ),
    ] {
        let factory = ProjectionFactory::new(
            &fixture.native.path,
            ProjectionEngine::Ladybug,
            changed.clone(),
            &SystemFileLock,
        );
        let error = factory
            .reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone(),
            )
            .expect_err("changed native or admitted pins must refuse");
        assert_eq!(error, ProjectionError::InputMismatch(kind));
        assert!(
            error
                .to_string()
                .contains("maestro knowledge graph rebuild")
        );
        assert_eq!(
            super::probe::Probe::receipt_with(
                &fixture.native.path,
                &SystemFileLock,
                &Inventory(&fixture),
                Some(changed)
            )
            .unwrap_err(),
            ProbeError::InputMismatch(kind)
        );
    }
}

/// Existing replaceable inventory seam, retaining the kernel's exact receipt.
struct Inventory<'a>(&'a Fixture);
impl ProjectionReceiptInventory for Inventory<'_> {
    fn inventory(&self, _: &str) -> Result<InventoryState, FactError> {
        let receipt = self
            .0
            .authority
            .database
            .projection_ready(&self.0.authority.scopes, self.0.build.scope.generation_id)?;
        Ok(InventoryState::Ready(vec![ProjectionInventory {
            collection_id: self.0.build.scope.collection_id.clone(),
            generation_id: self.0.build.scope.generation_id,
            receipt,
        }]))
    }
}

#[cfg(unix)]
#[test]
fn native_stamp_changed_snapshot_settings_lock_or_old_version_refuses_with_rebuild() {
    for (field, value, kind) in [
        (
            "resolution",
            Digest::of(b"other snapshot").as_str(),
            InputMismatchKind::Resolution,
        ),
        (
            "settings",
            Digest::of(b"other settings").as_str(),
            InputMismatchKind::Settings,
        ),
        (
            "lock",
            Digest::of(b"other lock").as_str(),
            InputMismatchKind::Lock,
        ),
        ("schema", "maestro-typed-edges/1", InputMismatchKind::Format),
    ] {
        let fixture = Fixture::new();
        publish(&fixture);
        let receipt = fixture
            .authority
            .database
            .projection_ready(&fixture.authority.scopes, fixture.build.scope.generation_id)
            .unwrap()
            .unwrap();
        {
            let database = super::open::open(
                &fixture.native.root,
                &receipt.file_name,
                super::config::native(&settings()),
            )
            .unwrap();
            let connection = lbug::Connection::new(&database).unwrap();
            if field == "schema" {
                connection.query("MATCH (p:Projection) DELETE p").unwrap();
                let mut statement = connection
                    .prepare(
                        "CREATE (:Projection {schema: $value,
                    collection: $collection, generation: $generation, resolution: $resolution,
                    resolver: $resolver, settings: $settings, lock: $lock})",
                    )
                    .unwrap();
                connection
                    .execute(
                        &mut statement,
                        vec![
                            ("value", lbug::Value::String(value.into())),
                            (
                                "collection",
                                lbug::Value::String(receipt.collection_id.clone()),
                            ),
                            ("generation", lbug::Value::Int64(receipt.generation_id)),
                            (
                                "resolution",
                                lbug::Value::String(receipt.resolution_id.as_str().into()),
                            ),
                            (
                                "resolver",
                                lbug::Value::String(receipt.resolver_version.clone()),
                            ),
                            (
                                "settings",
                                lbug::Value::String(receipt.settings_identity.as_str().into()),
                            ),
                            (
                                "lock",
                                lbug::Value::String(receipt.frozen_lock.as_str().into()),
                            ),
                        ],
                    )
                    .unwrap();
            } else {
                let mut statement = connection
                    .prepare(&format!("MATCH (p:Projection) SET p.{field} = $value"))
                    .unwrap();
                connection
                    .execute(
                        &mut statement,
                        vec![("value", lbug::Value::String(value.into()))],
                    )
                    .unwrap();
            }
            connection.query("CHECKPOINT").unwrap();
        }
        let error = fixture
            .factory()
            .reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone(),
            )
            .expect_err("changed native or admitted pins must refuse");
        assert_eq!(error, ProjectionError::InputMismatch(kind));
        let message = error.to_string();
        assert!(
            message.contains("maestro knowledge graph rebuild"),
            "{field}: {error}"
        );
    }
}
