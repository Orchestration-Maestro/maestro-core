//! Strict native decoding must retain corruption, never classify malformed pins as drift.
use super::{
    input_pins_tests::Inventory,
    public_fixture::{Fixture, settings},
    public_tests::publish,
};
use crate::graph::projection::{
    ProjectionError,
    health::{ProbeError, PublishedFile as _},
};
use maestro_filesystem::SystemFileLock;

#[cfg(unix)]
#[test]
fn native_malformed_pins_keep_corruption_category() {
    for (field, value) in [
        ("settings", Some("bad")),
        ("lock", Some("")),
        ("resolution", Some("bad")),
        ("resolver", Some("unknown/1")),
        ("resolution", None),
        ("resolver", None),
        ("settings", None),
        ("lock", None),
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
                &receipt.identity.file_name,
                super::config::native(&settings()),
            )
            .unwrap();
            let connection = lbug::Connection::new(&database).unwrap();
            if let Some(value) = value {
                let mut statement = connection
                    .prepare(&format!("MATCH (p:Projection) SET p.{field} = $value"))
                    .unwrap();
                connection
                    .execute(
                        &mut statement,
                        vec![("value", lbug::Value::String(value.into()))],
                    )
                    .unwrap();
            } else {
                connection
                    .query(&format!("MATCH (p:Projection) SET p.{field} = NULL"))
                    .unwrap();
            }
            connection.query("CHECKPOINT").unwrap();
        }
        assert!(
            matches!(
                fixture.factory().reader(
                    &fixture.authority.database,
                    &fixture.authority.scopes,
                    fixture.build.scope.clone()
                ),
                Err(ProjectionError::Backend(_))
            ),
            "{field}={value:?}: reader corruption"
        );
        let super::probe::ProbeReceipt::Files(files) = super::probe::Probe::receipt_with(
            &fixture.native.path,
            &SystemFileLock,
            &Inventory(&fixture),
            Some(settings()),
        )
        .unwrap() else {
            panic!("published file expected");
        };
        assert!(
            matches!(
                files[0].open_read_only().unwrap().query_one(),
                Err(ProbeError::Corrupt(_))
            ),
            "{field}={value:?}: health corruption"
        );
    }
}
