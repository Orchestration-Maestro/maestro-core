//! Page-level capture binding for partition inspection and acceptance.
use super::{
    capture::Captures,
    partition_record::DiscoveredItem,
    privacy::{Handle, ReceiptError},
    record::{COLUMNS, item_row},
};
use crate::{artifact::Digest, scope::Scope, store::Database};
use rusqlite::params;
use std::collections::BTreeMap;

/// Resolve every exact request/context together, then compare immutable change claims.
pub(super) fn bound(
    db: &Database,
    scope: &Scope,
    source: &str,
    discovered: &[DiscoveredItem],
) -> Result<Vec<Option<Handle>>, ReceiptError> {
    if discovered.len() > 1000 {
        return Err(ReceiptError::Invalid);
    }
    if discovered.is_empty() {
        return Ok(vec![]);
    }
    let requests: Vec<_> = discovered.iter().map(|item| &item.request).collect();
    let reader = db.reader()?;
    let columns = COLUMNS
        .split(", ")
        .map(|column| format!("f.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut query = reader.prepare(&format!(
        "SELECT {columns} FROM acquisition_frontier f JOIN json_each(?1) r
         ON f.fetch_identity = json_extract(r.value, '$.fetch_identity')
         AND f.authorization_context = json_extract(r.value, '$.authorization_context')
         AND f.representation_profile = json_extract(r.value, '$.representation_profile')
         WHERE f.source = ?2"
    ))?;
    let items = query
        .query_map(params![serde_json::to_string(&requests)?, source], item_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let captures = db.capture_page(scope, &items)?;
    let matched: BTreeMap<_, _> = items
        .iter()
        .filter_map(|item| {
            captures
                .get(&item.id)
                .filter(|capture| capture.acknowledged)
                .map(|capture| {
                    (
                        (
                            &item.request.fetch_identity,
                            &item.request.authorization_context,
                            &item.request.representation_profile,
                        ),
                        capture,
                    )
                })
        })
        .collect();
    discovered
        .iter()
        .map(|item| {
            let capture = matched.get(&(
                &item.request.fetch_identity,
                &item.request.authorization_context,
                &item.request.representation_profile,
            ));
            let Some(capture) = capture else {
                return Ok(None);
            };
            let validator = Digest::of(&serde_json::to_vec(&capture.envelope.headers)?);
            if item.keys.revision.is_some()
                || item
                    .keys
                    .validator
                    .as_ref()
                    .is_some_and(|key| *key != validator)
                || item
                    .keys
                    .representation
                    .as_ref()
                    .is_some_and(|key| *key != capture.envelope.artifact)
            {
                return Ok(None);
            }
            Ok(Some(capture.handle))
        })
        .collect()
}
