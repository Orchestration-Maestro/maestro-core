//! Native catalog mapping for the logical entity-key and adjacency access paths.

use crate::graph::projection::binding;
use crate::graph::projection::{
    port::{InputMismatchKind, ProjectionError, ProjectionScope},
    schema::{REQUIRED_INDEXES, SCHEMA_VERSION},
};
use lbug::{Connection, Value};
use std::collections::BTreeSet;

/// Version of the subject property's strict binary fact encoding.
pub(super) const FACT_VERSION: &[u8] = b"maestro-projection-fact/1\0";
/// Fixed schema statements: no application data is interpolated.
const DDL: [&str; 3] = [
    "CREATE NODE TABLE Projection(schema STRING, collection STRING,
        generation INT64, resolution STRING, resolver STRING, settings STRING,
        lock STRING, PRIMARY KEY(schema))",
    "CREATE NODE TABLE Entity(id STRING, facts BLOB[], PRIMARY KEY(id))",
    "CREATE REL TABLE Edge(FROM Entity TO Entity, id STRING, family STRING,
        relation STRING, collection STRING, generation INT64)",
];

/// Create the schema in a native transaction; Windows writers explicitly refuse.
pub(super) fn create(
    connection: &Connection<'_>,
    scope: &ProjectionScope,
    pins: &[String; 4],
) -> Result<(), String> {
    writable(cfg!(windows))?;
    connection
        .query("BEGIN TRANSACTION")
        .map_err(|error| error.to_string())?;
    let result = install(connection, scope, pins);
    match result {
        Ok(()) => connection
            .query("COMMIT")
            .map(|_| ())
            .map_err(|error| error.to_string()),
        Err(error) => {
            connection
                .query("ROLLBACK")
                .map_err(|error| format!("schema rollback failed: {error}"))?;
            Err(error)
        }
    }
}

/// Guard every production mutation before executing any native operation.
pub(super) fn writable(windows: bool) -> Result<(), String> {
    if windows {
        return Err(
            "native graph writes are unavailable on Windows; open a published graph read-only"
                .into(),
        );
    }
    Ok(())
}

/// Install fixed tables and one bound collection/generation stamp.
fn install(
    connection: &Connection<'_>,
    scope: &ProjectionScope,
    pins: &[String; 4],
) -> Result<(), String> {
    binding::validate(pins)?;
    for ddl in DDL {
        connection.query(ddl).map_err(|error| error.to_string())?;
    }
    let mut statement = connection
        .prepare(
            "CREATE (:Projection {schema: $schema, collection: $collection,
            generation: $generation, resolution: $resolution, resolver: $resolver,
            settings: $settings, lock: $lock})",
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            &mut statement,
            vec![
                ("schema", Value::String(SCHEMA_VERSION.into())),
                ("collection", Value::String(scope.collection_id.clone())),
                ("generation", Value::Int64(scope.generation_id)),
                ("resolution", text(&pins[0])),
                ("resolver", text(&pins[1])),
                ("settings", text(&pins[2])),
                ("lock", text(&pins[3])),
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Read catalog structure and the single scope stamp, not a trusted saved count.
pub(super) fn verify(
    connection: &Connection<'_>,
    scope: &ProjectionScope,
) -> Result<(BTreeSet<String>, [String; 4]), ProjectionError> {
    let pins = read_pins(connection, scope)?;
    let tables: Vec<_> = connection
        .query("CALL show_tables() RETURN name, type ORDER BY name")
        .map_err(|error| ProjectionError::Backend(error.to_string()))?
        .collect();
    if tables
        != vec![
            vec![text("Edge"), text("REL")],
            vec![text("Entity"), text("NODE")],
            vec![text("Projection"), text("NODE")],
        ]
    {
        return Err(ProjectionError::Backend(
            "unexpected native projection tables".into(),
        ));
    }
    check_properties(
        connection,
        "CALL table_info('Entity') RETURN name, type, `primary key`",
        &[
            ("id", "STRING", Value::Bool(true)),
            ("facts", "BLOB[]", Value::Bool(false)),
        ],
    )
    .map_err(ProjectionError::Backend)?;
    check_properties(
        connection,
        "CALL table_info('Projection') RETURN name, type, `primary key`",
        &[
            ("schema", "STRING", Value::Bool(true)),
            ("collection", "STRING", Value::Bool(false)),
            ("generation", "INT64", Value::Bool(false)),
            ("resolution", "STRING", Value::Bool(false)),
            ("resolver", "STRING", Value::Bool(false)),
            ("settings", "STRING", Value::Bool(false)),
            ("lock", "STRING", Value::Bool(false)),
        ],
    )
    .map_err(ProjectionError::Backend)?;
    check_properties(
        connection,
        "CALL table_info('Edge') RETURN name, type, storage_direction",
        &[
            ("id", "STRING", text("both")),
            ("family", "STRING", text("both")),
            ("relation", "STRING", text("both")),
            ("collection", "STRING", text("both")),
            ("generation", "INT64", text("both")),
        ],
    )
    .map_err(ProjectionError::Backend)?;
    let endpoints: Vec<_> = connection
        .query("CALL show_connection('Edge') RETURN *")
        .map_err(|error| ProjectionError::Backend(error.to_string()))?
        .collect();
    if endpoints != vec![vec![text("Entity"), text("Entity"), text("id"), text("id")]] {
        return Err(ProjectionError::Backend(
            "native typed adjacency endpoints differ".into(),
        ));
    }
    // These logical names follow only after catalog evidence proves their mapping.
    Ok((
        REQUIRED_INDEXES
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
        pins,
    ))
}

/// Read the version before querying new columns, so old stamps get the rebuild repair.
fn read_pins(
    connection: &Connection<'_>,
    scope: &ProjectionScope,
) -> Result<[String; 4], ProjectionError> {
    use maestro_kernel::facts::PROJECTION_REBUILD_REPAIR;
    let stamps: Vec<_> = connection
        .query("MATCH (p:Projection) RETURN p.schema, p.collection, p.generation")
        .map_err(|error| ProjectionError::Backend(error.to_string()))?
        .collect();
    if stamps
        == vec![vec![
            text("maestro-typed-edges/1"),
            text(&scope.collection_id),
            Value::Int64(scope.generation_id),
        ]]
    {
        return Err(ProjectionError::InputMismatch(InputMismatchKind::Format));
    }
    if stamps
        != vec![vec![
            text(SCHEMA_VERSION),
            text(&scope.collection_id),
            Value::Int64(scope.generation_id),
        ]]
    {
        return Err(ProjectionError::Backend(format!(
            "native projection schema or scope stamp differs; {PROJECTION_REBUILD_REPAIR}"
        )));
    }
    let rows: Vec<_> = connection
        .query("MATCH (p:Projection) RETURN p.resolution, p.resolver, p.settings, p.lock")
        .map_err(|error| {
            ProjectionError::Backend(format!(
                "invalid native input pins: {error}; {PROJECTION_REBUILD_REPAIR}"
            ))
        })?
        .collect();
    let [row] = rows.as_slice() else {
        return Err(ProjectionError::Backend(format!(
            "missing native input pins; {PROJECTION_REBUILD_REPAIR}"
        )));
    };
    let [
        Value::String(resolution),
        Value::String(resolver),
        Value::String(settings),
        Value::String(lock),
    ] = row.as_slice()
    else {
        return Err(ProjectionError::Backend(format!(
            "malformed native input pins; {PROJECTION_REBUILD_REPAIR}"
        )));
    };
    let pins = [
        resolution.clone(),
        resolver.clone(),
        settings.clone(),
        lock.clone(),
    ];
    binding::validate(&pins).map_err(ProjectionError::Backend)?;
    Ok(pins)
}

/// Compare exact property order, types, keys and adjacency storage direction.
fn check_properties(
    connection: &Connection<'_>,
    query: &str,
    expected: &[(&str, &str, Value)],
) -> Result<(), String> {
    let rows: Vec<_> = connection
        .query(query)
        .map_err(|error| error.to_string())?
        .collect();
    let expected: Vec<_> = expected
        .iter()
        .map(|(name, kind, key)| vec![text(name), text(kind), key.clone()])
        .collect();
    if rows != expected {
        return Err("native projection properties or access path differ".into());
    }
    Ok(())
}

/// Build a fixed catalog string value.
fn text(value: &str) -> Value {
    Value::String(value.to_owned())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    #[test]
    fn native_write_policy_refuses_windows_on_every_host() {
        assert_eq!(
            writable(true).unwrap_err(),
            "native graph writes are unavailable on Windows; open a published graph read-only"
        );
        assert_eq!(writable(false), Ok(()));
        assert_eq!(writable(cfg!(windows)).is_err(), cfg!(windows));
    }

    /// Only the legacy immutable reader fixture may bypass Windows's write guard.
    #[cfg(windows)]
    pub(in crate::graph::projection::engine) fn install_reader_fixture(
        connection: &Connection<'_>,
        scope: &ProjectionScope,
        pins: &[String; 4],
    ) {
        install(connection, scope, pins).unwrap();
    }
}
