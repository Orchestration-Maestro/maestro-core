//! Official Qdrant inspection and cleanup, restricted to the empty owned service.

use super::wipe_safety::QdrantOwner;
use maestro_kernel::{
    chunk_set::Chunk, document::Revision, generation::Generation, retrieval::IDENTIFIER_PROFILE,
};
use qdrant_client::{
    Payload, Qdrant as Client,
    qdrant::{DeleteAlias, DeleteCollection, ScrollPointsBuilder, point_id::PointIdOptions},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use tokio::runtime::{Builder, Runtime};

const ALIAS: &str = "maestro-synthetic";
const SCOPE_TAGS: [&str; 3] = [
    "workspace/default",
    "workspace/default/collection/synthetic",
    "workspace/default/collection/synthetic/source/handbook",
];

pub(super) struct QdrantScratch {
    pub(super) url: String,
    pub(super) version: String,
}

impl QdrantScratch {
    pub(super) fn new(owner: &QdrantOwner) -> Result<Self, String> {
        let mut scratch = Self {
            url: owner.url.clone(),
            version: String::new(),
        };
        let runtime = runtime()?;
        let health = runtime
            .block_on(client(&scratch.url).health_check())
            .map_err(|error| error.to_string())?;
        if health.version != "1.19.1" {
            return Err(format!(
                "expected scratch Qdrant 1.19.1, got {}",
                health.version
            ));
        }
        scratch.version = health.version;
        scratch.require_empty()?;
        Ok(scratch)
    }

    pub(super) fn require_empty(&self) -> Result<(), String> {
        let runtime = runtime()?;
        let aliases = runtime
            .block_on(client(&self.url).list_aliases())
            .map_err(|error| error.to_string())?;
        let collections = runtime
            .block_on(client(&self.url).list_collections())
            .map_err(|error| error.to_string())?;
        if !aliases.aliases.is_empty() || !collections.collections.is_empty() {
            return Err("owned scratch Qdrant must start and end empty".to_owned());
        }
        Ok(())
    }

    pub(super) fn delete_alias(&self, generation: i64) -> Result<(), String> {
        let runtime = runtime()?;
        let alias = runtime
            .block_on(client(&self.url).list_aliases())
            .map_err(|error| error.to_string())?
            .aliases
            .into_iter()
            .find(|alias| alias.alias_name == ALIAS);
        let Some(alias) = alias else {
            return Ok(());
        };
        let expected = format!("maestro-synthetic-g{generation}");
        if alias.collection_name != expected {
            return Err(format!(
                "refusing to delete alias {ALIAS} targeting {}",
                alias.collection_name
            ));
        }
        runtime
            .block_on(client(&self.url).delete_alias(DeleteAlias {
                alias_name: ALIAS.to_owned(),
            }))
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub(super) fn delete_generation(&self, generation: i64) -> Result<(), String> {
        let runtime = runtime()?;
        let client = client(&self.url);
        let name = format!("maestro-synthetic-g{generation}");
        let aliases = runtime
            .block_on(client.list_aliases())
            .map_err(|error| error.to_string())?;
        for alias in aliases.aliases {
            if alias.alias_name != ALIAS {
                continue;
            }
            if alias.collection_name != name {
                return Err(format!(
                    "refusing to delete alias {ALIAS} targeting {}",
                    alias.collection_name
                ));
            }
            runtime
                .block_on(client.delete_alias(DeleteAlias {
                    alias_name: ALIAS.to_owned(),
                }))
                .map_err(|error| error.to_string())?;
        }
        let exists = runtime
            .block_on(client.collection_exists(&name))
            .map_err(|error| error.to_string())?;
        if exists {
            runtime
                .block_on(client.delete_collection(DeleteCollection {
                    collection_name: name,
                    timeout: None,
                }))
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub(super) fn aliases(&self) -> Result<BTreeMap<String, String>, String> {
        let runtime = runtime()?;
        let response = runtime
            .block_on(client(&self.url).list_aliases())
            .map_err(|error| error.to_string())?;
        Ok(response
            .aliases
            .into_iter()
            .map(|alias| (alias.alias_name, alias.collection_name))
            .collect())
    }

    pub(super) fn collections(&self) -> Result<BTreeSet<String>, String> {
        let runtime = runtime()?;
        let response = runtime
            .block_on(client(&self.url).list_collections())
            .map_err(|error| error.to_string())?;
        Ok(response
            .collections
            .into_iter()
            .map(|collection| collection.name)
            .collect())
    }

    pub(super) fn points_with_payload(
        &self,
        collection: &str,
    ) -> Result<Vec<(String, Value)>, String> {
        let runtime = runtime()?;
        let response = runtime
            .block_on(
                client(&self.url).scroll(
                    ScrollPointsBuilder::new(collection)
                        .limit(10_000)
                        .with_payload(true)
                        .with_vectors(false),
                ),
            )
            .map_err(|error| error.to_string())?;
        if response.next_page_offset.is_some() {
            return Err("synthetic Qdrant collection exceeds one payload page".to_owned());
        }
        Ok(response
            .result
            .into_iter()
            .map(|point| {
                let id = match point.id.unwrap().point_id_options.unwrap() {
                    PointIdOptions::Uuid(id) => id,
                    PointIdOptions::Num(id) => id.to_string(),
                };
                (id, Value::from(Payload::from(point.payload)))
            })
            .collect())
    }

    pub(super) fn cleanup(&self) -> Result<(), String> {
        let runtime = runtime()?;
        let client = client(&self.url);
        let aliases = runtime
            .block_on(client.list_aliases())
            .map_err(|error| error.to_string())?;
        for alias in aliases.aliases {
            if alias.alias_name != ALIAS {
                continue;
            }
            if !alias.collection_name.starts_with("maestro-synthetic-g") {
                return Err(format!(
                    "refusing to remove unowned alias target {}",
                    alias.collection_name
                ));
            }
            runtime
                .block_on(client.delete_alias(DeleteAlias {
                    alias_name: alias.alias_name,
                }))
                .map_err(|error| error.to_string())?;
        }
        let collections = runtime
            .block_on(client.list_collections())
            .map_err(|error| error.to_string())?;
        for collection in collections.collections {
            if collection.name.starts_with("maestro-synthetic-g") {
                runtime
                    .block_on(client.delete_collection(DeleteCollection {
                        collection_name: collection.name,
                        timeout: None,
                    }))
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }
}

impl Drop for QdrantScratch {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("T033b could not clean its owned Qdrant collections: {error}");
        }
    }
}

pub(super) fn point_ids(
    url: &str,
    generation: &Generation,
    chunks: &HashMap<String, Chunk>,
    revisions: &HashMap<String, Revision>,
) -> BTreeSet<String> {
    let runtime = runtime().unwrap();
    let collection = format!("maestro-{}-g{}", generation.collection_id, generation.id);
    let request = ScrollPointsBuilder::new(&collection)
        .limit(10_000)
        .with_payload(true)
        .with_vectors(false);
    let page = runtime.block_on(client(url).scroll(request)).unwrap();
    assert!(
        page.next_page_offset.is_none(),
        "synthetic points fit one page"
    );
    page.result
        .into_iter()
        .map(|point| {
            let point_id = match point.id.unwrap().point_id_options.unwrap() {
                PointIdOptions::Uuid(id) => id,
                PointIdOptions::Num(id) => id.to_string(),
            };
            let payload = Value::from(Payload::from(point.payload));
            let chunk_id = payload["chunk_id"].as_str().unwrap();
            let chunk = chunks.get(chunk_id).unwrap();
            assert_eq!(payload["revision_id"], chunk.revision_id);
            assert_eq!(payload["identifier_profile"], IDENTIFIER_PROFILE);
            assert_eq!(payload["scope_tags"], json!(SCOPE_TAGS));
            assert!(revisions.contains_key(&chunk.revision_id));
            point_id
        })
        .collect()
}

fn client(url: &str) -> Client {
    Client::from_url(url)
        .skip_compatibility_check()
        .build()
        .unwrap()
}

fn runtime() -> Result<Runtime, String> {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
}
