//! Cold native opens bind durable stamps, not just live process registry entries.
use super::{
    public_fixture::{Fixture, settings},
    public_tests::publish,
};
#[cfg(unix)]
use crate::graph::projection::health::PublishedFile as _;
use crate::graph::projection::{
    EngineSettings, InputMismatchKind, ProjectionEngine, ProjectionError, ProjectionFactory,
};
use crate::graph::projection::{health::ProbeError, receipts::ProjectionReceiptInventory};
use maestro_filesystem::SystemFileLock;
#[cfg(unix)]
use maestro_kernel::facts::ProjectionReceipt;
use maestro_kernel::{
    artifact::Digest,
    facts::{
        EXACT_RESOLVER_VERSION, Error as FactError, InventoryState, ProjectionBuildRequest,
        ProjectionInventory, ProjectionReservation, ResolutionInput,
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
            fixture.native.path.join(receipt.identity.file_name),
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
pub(super) struct Inventory<'a>(pub(super) &'a Fixture);
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
fn native_health_reports_exact_input_mismatch_kind() {
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
        change_stamp(&fixture, &receipt, field, value);
        let error = fixture
            .factory()
            .reader(
                &fixture.authority.database,
                &fixture.authority.scopes,
                fixture.build.scope.clone(),
            )
            .expect_err("changed native or admitted pins must refuse");
        assert_eq!(error, ProjectionError::InputMismatch(kind));
        let super::probe::ProbeReceipt::Files(files) = super::probe::Probe::receipt_with(
            &fixture.native.path,
            &SystemFileLock,
            &Inventory(&fixture),
            Some(settings()),
        )
        .unwrap() else {
            panic!("published file expected");
        };
        assert_eq!(
            files[0].open_read_only().unwrap().query_one().unwrap_err(),
            ProbeError::InputMismatch(kind),
            "{field}: health category"
        );
        let message = error.to_string();
        assert!(
            message.contains("maestro knowledge graph rebuild"),
            "{field}: {error}"
        );
    }
}

/// Change one durable stamp using a bound native mutation, then checkpoint and close.
#[cfg(unix)]
fn change_stamp(fixture: &Fixture, receipt: &ProjectionReceipt, field: &str, value: &str) {
    let database = super::open::open(
        &fixture.native.root,
        &receipt.identity.file_name,
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
                        lbug::Value::String(receipt.identity.collection_id.clone()),
                    ),
                    (
                        "generation",
                        lbug::Value::Int64(receipt.identity.generation_id),
                    ),
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

#[test]
fn supplied_build_matches_every_reserved_field_before_native_io() {
    let fixture = Fixture::new();
    let build = &fixture.build;
    let reserved = ProjectionReservation {
        build_id: build.build_id,
        job: build.lease.job,
        request: ProjectionBuildRequest {
            collection_id: build.scope.collection_id.clone(),
            generation_id: build.scope.generation_id,
            claim_set_id: build.claim_set_id.clone(),
            resolution_id: build.resolution_id.clone(),
            resolver_version: build.resolver_version.clone(),
            settings_identity: build.settings_identity.clone(),
            frozen_lock: build.frozen_lock.clone(),
            expected_active_build_id: None,
        },
    };
    super::input_pins::reserved(build, &reserved).unwrap();
    for field in 0..9 {
        let mut changed = build.clone();
        let other = Digest::of(b"foreign");
        match field {
            0 => changed.build_id += 1,
            1 => changed.lease.job = "00000000000000000000000000".parse().unwrap(),
            2 => changed.scope.collection_id = "foreign".into(),
            3 => changed.scope.generation_id += 1,
            4 => changed.claim_set_id = other,
            5 => changed.resolution_id = other,
            6 => changed.resolver_version = "other/1".into(),
            7 => changed.settings_identity = other,
            _ => changed.frozen_lock = other,
        }
        assert!(
            super::input_pins::reserved(&changed, &reserved).is_err(),
            "field {field}"
        );
    }
}
