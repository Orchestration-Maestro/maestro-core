//! Ownership checks and cleanup for the test-owned Qdrant instance.

use qdrant_client::{
    Qdrant,
    qdrant::{DeleteAlias, DeleteCollection},
};
use std::{collections::BTreeSet, fmt};

pub(super) const REAL_URL: &str = "http://127.0.0.1:16634";
const COLLECTION_PREFIX: &str = "maestro-synthetic-g";
const ALIAS: &str = "maestro-synthetic";

#[derive(Debug)]
pub(super) enum PreflightError {
    Client(String),
    UnsupportedVersion(String),
    NotEmpty,
}

impl fmt::Display for PreflightError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Client(error) => write!(formatter, "Qdrant request failed: {error}"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "expected Qdrant 1.19.x, found {version}")
            }
            Self::NotEmpty => formatter.write_str("service has aliases or collections"),
        }
    }
}

/// Requires an empty test-owned service before creating synthetic names.
pub(super) async fn require_empty(url: &str) -> Result<(), PreflightError> {
    let client = client(url).map_err(PreflightError::Client)?;
    let aliases = client
        .list_aliases()
        .await
        .map_err(|error| PreflightError::Client(format!("list aliases: {error}")))?;
    if !aliases.aliases.is_empty() {
        return Err(PreflightError::NotEmpty);
    }
    let collections = client
        .list_collections()
        .await
        .map_err(|error| PreflightError::Client(format!("list collections: {error}")))?;
    if !collections.collections.is_empty() {
        return Err(PreflightError::NotEmpty);
    }
    Ok(())
}

/// Checks the required pinned-service version and empty test-owned service.
pub(super) async fn require_real_qdrant(url: &str) -> Result<String, PreflightError> {
    let client = client(url).map_err(PreflightError::Client)?;
    let health = client
        .health_check()
        .await
        .map_err(|error| PreflightError::Client(format!("health check: {error}")))?;
    if !health.version.starts_with("1.19.") {
        return Err(PreflightError::UnsupportedVersion(health.version));
    }
    require_empty(url).await?;
    Ok(health.version)
}

/// Deletes only the alias and generation collections this invocation created.
pub(super) async fn cleanup(url: &str, generations: &[i64]) -> Result<(), String> {
    if generations.is_empty() {
        return Ok(());
    }
    let client = client(url)?;
    let collections: BTreeSet<String> = generations
        .iter()
        .map(|generation| format!("{COLLECTION_PREFIX}{generation}"))
        .collect();
    let aliases = client
        .list_aliases()
        .await
        .map_err(|error| format!("list aliases for cleanup: {error}"))?;
    for alias in aliases.aliases {
        if alias.alias_name == ALIAS {
            if !collections.contains(&alias.collection_name) {
                return Err("synthetic alias points outside this run".to_owned());
            }
            client
                .delete_alias(DeleteAlias {
                    alias_name: ALIAS.to_owned(),
                })
                .await
                .map_err(|error| format!("delete synthetic alias: {error}"))?;
        }
    }
    for collection in collections {
        if client
            .collection_exists(&collection)
            .await
            .map_err(|error| format!("check synthetic collection {collection}: {error}"))?
        {
            client
                .delete_collection(DeleteCollection {
                    collection_name: collection,
                    timeout: None,
                })
                .await
                .map_err(|error| format!("delete synthetic collection: {error}"))?;
        }
    }
    Ok(())
}

fn client(url: &str) -> Result<Qdrant, String> {
    Qdrant::from_url(url)
        .skip_compatibility_check()
        .build()
        .map_err(|error| error.to_string())
}
