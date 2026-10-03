//! Native catalog mapping for the logical entity-key and adjacency access paths.

use crate::graph::projection::{
    ProjectionScope,
    schema::{REQUIRED_INDEXES, SCHEMA_VERSION},
};
use lbug::{Connection, Value};
use std::collections::BTreeSet;

/// Version of the subject property's strict binary fact encoding.
pub(super) const FACT_VERSION: &[u8] = b"maestro-projection-fact/1\0";
/// Fixed schema statements: no application data is interpolated.
const DDL: [&str; 3] = [
    "CREATE NODE TABLE Projection(schema STRING, collection STRING,
        generation INT64, PRIMARY KEY(schema))",
    "CREATE NODE TABLE Entity(id STRING, facts BLOB[], PRIMARY KEY(id))",
    "CREATE REL TABLE Edge(FROM Entity TO Entity, id STRING, family STRING,
        relation STRING, collection STRING, generation INT64)",
];

/// Create the schema in a native transaction; Windows writers explicitly refuse.
pub(super) fn create(connection: &Connection<'_>, scope: &ProjectionScope) -> Result<(), String> {
    writable()?;
    connection
        .query("BEGIN TRANSACTION")
        .map_err(|error| error.to_string())?;
    let result = install(connection, scope);
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
pub(super) fn writable() -> Result<(), String> {
    if cfg!(windows) {
        return Err(
            "native graph writes are unavailable on Windows; open a published graph read-only"
                .into(),
        );
    }
    Ok(())
}

/// Install fixed tables and one bound collection/generation stamp.
fn install(connection: &Connection<'_>, scope: &ProjectionScope) -> Result<(), String> {
    for ddl in DDL {
        connection.query(ddl).map_err(|error| error.to_string())?;
    }
    let mut statement = connection
        .prepare(
            "CREATE (:Projection {schema: $schema, collection: $collection,
            generation: $generation})",
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            &mut statement,
            vec![
                ("schema", Value::String(SCHEMA_VERSION.into())),
                ("collection", Value::String(scope.collection_id.clone())),
                ("generation", Value::Int64(scope.generation_id)),
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Read catalog structure and the single scope stamp, not a trusted saved count.
pub(super) fn verify(
    connection: &Connection<'_>,
    scope: &ProjectionScope,
) -> Result<BTreeSet<String>, String> {
    let tables: Vec<_> = connection
        .query("CALL show_tables() RETURN name, type ORDER BY name")
        .map_err(|error| error.to_string())?
        .collect();
    if tables
        != vec![
            vec![text("Edge"), text("REL")],
            vec![text("Entity"), text("NODE")],
            vec![text("Projection"), text("NODE")],
        ]
    {
        return Err("unexpected native projection tables".into());
    }
    check_properties(
        connection,
        "CALL table_info('Entity') RETURN name, type, `primary key`",
        &[
            ("id", "STRING", Value::Bool(true)),
            ("facts", "BLOB[]", Value::Bool(false)),
        ],
    )?;
    check_properties(
        connection,
        "CALL table_info('Projection') RETURN name, type, `primary key`",
        &[
            ("schema", "STRING", Value::Bool(true)),
            ("collection", "STRING", Value::Bool(false)),
            ("generation", "INT64", Value::Bool(false)),
        ],
    )?;
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
    )?;
    let endpoints: Vec<_> = connection
        .query("CALL show_connection('Edge') RETURN *")
        .map_err(|error| error.to_string())?
        .collect();
    if endpoints != vec![vec![text("Entity"), text("Entity"), text("id"), text("id")]] {
        return Err("native typed adjacency endpoints differ".into());
    }
    let stamps: Vec<_> = connection
        .query("MATCH (p:Projection) RETURN p.schema, p.collection, p.generation")
        .map_err(|error| error.to_string())?
        .collect();
    if stamps
        != vec![vec![
            text(SCHEMA_VERSION),
            text(&scope.collection_id),
            Value::Int64(scope.generation_id),
        ]]
    {
        return Err("native projection schema or scope stamp differs".into());
    }
    // These logical names follow only after catalog evidence proves their mapping.
    Ok(REQUIRED_INDEXES
        .iter()
        .map(|name| (*name).to_owned())
        .collect())
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
#[cfg(windows)]
pub(super) mod tests {
    use super::*;
    /// Only the legacy immutable reader fixture may bypass Windows's write guard.
    pub(in crate::graph::projection::engine) fn install_reader_fixture(
        connection: &Connection<'_>,
        scope: &ProjectionScope,
    ) {
        install(connection, scope).unwrap();
    }
}
