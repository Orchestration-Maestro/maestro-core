use super::ask::tests::select_reranker;
use super::search::{search_with, selected_reranker};
use super::{KnowledgeError, collections_with, get_with, section_read_failure};
use crate::knowledge::SearchRequest;
use crate::{
    failure::Failure,
    kernel::{Kernel, pinned_embedder},
    knowledge::GetRequest,
};
use maestro_kernel::gateway::{
    CardFields, Limits, ModelCard, Role, RouterClient, RouterEntry, Url,
};
use maestro_kernel::{
    artifact::{Digest, Store},
    chunk_set::{Chunk, NewChunkSet},
    document::{Collection, Document, Revision, RevisionStatus, Source},
    evidence::Span,
    generation::{Error as GenerationError, NewGeneration},
    scope::Config,
    store::Database,
};
use maestro_knowledge::{
    index::Qdrant,
    search::{SearchError, evidence::SectionReadError, routes::error::RouteError},
};
use serde_json::Map;
use std::{
    collections::BTreeMap,
    env, fs, io,
    num::{NonZeroU32, NonZeroUsize},
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

const SOURCE: &str = "exact source text";
const ACCESS_CHANGED: KnowledgeError = KnowledgeError::Refused {
    code: "access_changed",
    message: "permissions changed during the request; no result was delivered",
};

pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = env::temp_dir().join(format!(
            "maestro-knowledge-refresh-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let data = root.join("data");
        let config = root.join("config");
        fs::create_dir_all(&data).expect("create data directory");
        fs::create_dir_all(&config).expect("create config directory");
        fs::write(
            config.join("config.toml"),
            "[access]\nread = ['workspace/default']\n",
        )
        .expect("write initial grant");
        let scratch = Self(root);
        scratch.seed();
        scratch
    }

    fn seed(&self) {
        let database = Database::open_in(&self.data()).expect("open scratch database");
        database
            .record_collection(&Collection {
                id: "collection".to_owned(),
                title: "Visible collection".to_owned(),
                visibility: "private".to_owned(),
                profiles: BTreeMap::new(),
            })
            .expect("record collection");
        database
            .record_source(&Source {
                collection_id: "collection".to_owned(),
                id: "source".to_owned(),
                kind: "import".to_owned(),
                transport: None,
                reference: "source:docs".to_owned(),
                profiles: BTreeMap::new(),
            })
            .expect("record source");
        database
            .record_document(&Document {
                id: "document".to_owned(),
                collection_id: "collection".to_owned(),
                source_id: "source".to_owned(),
                source_ref: "source:docs".to_owned(),
            })
            .expect("record document");
        let revision = Revision {
            id: "revision".to_owned(),
            document_id: "document".to_owned(),
            original_digest: database
                .put(SOURCE.as_bytes(), "text/plain")
                .expect("store source"),
            canonical_digest: database
                .put(b"{}", "application/json")
                .expect("store canonical source"),
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: Map::new(),
        };
        database
            .record_revision(&revision)
            .expect("record revision");
        database
            .begin_chunk_set(&NewChunkSet {
                id: "chunk-set",
                collection_id: "collection",
                chunk_profile: "structural-500-700/1",
                counter_contract_id: "test",
            })
            .expect("begin chunk set");
        database
            .record_chunks(
                "chunk-set",
                &revision.id,
                &[Chunk {
                    id: "chunk".to_owned(),
                    revision_id: revision.id.clone(),
                    section_id: Some("section".to_owned()),
                    digest: database
                        .put(b"prepared chunk", "text/plain")
                        .expect("store prepared chunk"),
                    token_count: 1,
                    span: Span {
                        start: 0,
                        end: SOURCE.len(),
                    },
                }],
            )
            .expect("record chunk");
        let manifest = database
            .put(b"{}", "application/json")
            .expect("store manifest");
        database
            .complete_chunk_set("chunk-set", &manifest)
            .expect("complete chunk set");
        let generation = database
            .create_generation(&NewGeneration {
                collection_id: "collection".to_owned(),
                chunk_set_id: "chunk-set".to_owned(),
                embedding_profile: "embed:test".to_owned(),
                sparse_profile: "bm25-en-fr/1".to_owned(),
            })
            .expect("create generation");
        database
            .verify_generation(generation.id, 1)
            .expect("verify generation");
        database
            .publish_generation(generation.id)
            .expect("publish generation");
    }

    fn data(&self) -> PathBuf {
        self.0.join("data")
    }

    fn config(&self) -> PathBuf {
        self.0.join("config")
    }

    pub(crate) fn kernel(&self, revoke_after_refreshes: Option<usize>) -> Result<Kernel, Failure> {
        let kernel = Kernel::open_at(&self.data(), &self.config())?;
        Ok(if let Some(after) = revoke_after_refreshes {
            kernel.with_test_refresh_hook(after, revoke_access)
        } else {
            kernel
        })
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove scratch database");
    }
}

fn revoke_access(kernel: &Kernel) {
    fs::write(
        kernel.config_dir.join("config.toml"),
        "[access]\nread = []\n",
    )
    .expect("revoke local access");
}

fn chunk_request() -> GetRequest {
    GetRequest::from_cli(
        Some("chunk".to_owned()),
        None,
        Some("collection".to_owned()),
        None,
    )
    .expect("valid chunk request")
}

fn section_request() -> GetRequest {
    GetRequest::from_cli(
        None,
        Some("section".to_owned()),
        Some("collection".to_owned()),
        None,
    )
    .expect("valid section request")
}

#[test]
fn search_generation_lookup_failures_are_reported_as_kernel_unavailable() {
    let error = SearchError::Admission(RouteError::GenerationLookup(
        GenerationError::UnknownGeneration(7),
    ));
    assert_eq!(
        super::search::search_failure(&error),
        KnowledgeError::Failed {
            code: "kernel_unavailable",
            message: "the local knowledge store is unavailable",
        }
    );
}

#[test]
fn collections_refuses_access_revoked_before_delivery() {
    let scratch = Scratch::new();
    let result = collections_with(|| scratch.kernel(Some(0)));
    assert!(matches!(result, Err(ACCESS_CHANGED)));
}

#[test]
fn get_refuses_access_revoked_before_source_read() {
    let scratch = Scratch::new();
    let result = get_with(|| scratch.kernel(Some(0)), &chunk_request());
    assert!(matches!(result, Err(ACCESS_CHANGED)));
}

#[test]
fn section_get_refuses_access_revoked_before_source_read() {
    let scratch = Scratch::new();
    let result = get_with(|| scratch.kernel(Some(0)), &section_request());
    assert!(matches!(result, Err(ACCESS_CHANGED)));
}

#[test]
fn section_get_refuses_access_revoked_after_source_read_error() {
    let scratch = Scratch::new();
    let result = get_with(|| scratch.kernel(Some(1)), &section_request());
    assert!(matches!(result, Err(ACCESS_CHANGED)));
}

#[test]
fn get_refuses_access_revoked_after_source_resolution() {
    let scratch = Scratch::new();
    let result = get_with(|| scratch.kernel(Some(1)), &chunk_request());
    assert!(matches!(result, Err(ACCESS_CHANGED)));
}

#[tokio::test]
async fn search_refuses_a_temporary_grant_revoked_before_delivery() {
    let scratch = Scratch::new();
    fs::write(
        scratch.config().join("config.toml"),
        "[access]\nread = []\n",
    )
    .expect("remove the initial grant");
    let kernel = scratch
        .kernel(None)
        .expect("open without collection access")
        .with_test_refresh_hook(0, revoke_access);
    fs::write(
        scratch.config().join("config.toml"),
        "[access]\nread = ['workspace/default']\n",
    )
    .expect("grant collection access during search");
    kernel
        .database
        .apply_config(&Config::load(&scratch.config()).expect("load temporary grant"))
        .expect("apply temporary grant");
    let request = SearchRequest::parse(serde_json::json!({
        "collection": "collection",
        "query": "question",
        "deadline_ms": 1000
    }))
    .expect("valid search request");
    let model_port = RouterClient::new(Url::parse("http://127.0.0.1:8080").unwrap()).unwrap();
    let qdrant = Qdrant::new("http://127.0.0.1:1").unwrap();

    let result = Box::pin(search_with(kernel, &request, &model_port, &qdrant)).await;

    assert!(matches!(result, Err(ACCESS_CHANGED)));
}

#[test]
fn collections_opens_the_kernel_once() {
    let scratch = Scratch::new();
    let mut opens = 0;
    let result = collections_with(|| {
        opens += 1;
        scratch.kernel(None)
    });
    assert!(result.is_ok());
    assert_eq!(opens, 1);
}

#[test]
fn a_search_freezes_the_reranker_selected_for_its_collection() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).expect("open test kernel");
    let generation = kernel
        .database
        .published_generation(&kernel.scopes, "collection")
        .expect("read published generation")
        .expect("published generation");
    let selected = |kernel: &Kernel| {
        selected_reranker(&kernel.database, &kernel.scopes, "collection")
            .expect("read selected reranker")
            .map(|card| card.digest().clone())
    };

    assert_eq!(selected(&kernel), None);
    let card = select_reranker(&kernel, &generation);
    assert_eq!(selected(&kernel), Some(card.digest().clone()));
}

#[test]
fn pinned_embedder_loads_unregistered_v1_cards_and_degrades_for_missing_or_wrong_role() {
    let root = env::temp_dir().join(format!("maestro-pinned-embedder-{}", process::id()));
    let store = Store::new(&root);
    let embedder = ModelCard::record(&store, &model_card_fields(Role::Embedder))
        .expect("record legacy embedder card");
    let profile = format!("dense/1:sha256:{}", embedder.digest().as_str());

    let resolved = pinned_embedder(&store, Some(&profile));
    assert_eq!(resolved.as_ref(), Some(&embedder));
    assert!(pinned_embedder(&store, Some(&format!("dense/1:sha256:{}", "0".repeat(64)))).is_none());

    let wrong_role = ModelCard::record(&store, &model_card_fields(Role::Reranker))
        .expect("record wrong-role card");
    let wrong_profile = format!("dense/1:sha256:{}", wrong_role.digest().as_str());
    assert!(pinned_embedder(&store, Some(&wrong_profile)).is_none());

    fs::remove_dir_all(root).expect("remove artifact store");
}

fn model_card_fields(role: Role) -> CardFields {
    CardFields {
        role,
        router_entry: RouterEntry::parse("test-model").expect("router entry"),
        file_digest: Digest::of(b"model"),
        template_digest: None,
        server_build: "test".to_owned(),
        dimensions: (role == Role::Embedder)
            .then(|| NonZeroUsize::new(2).expect("nonzero dimensions")),
        limits: Limits {
            context_tokens: NonZeroU32::new(1024).expect("nonzero context"),
            output_tokens: None,
        },
        suite_results: Vec::new(),
    }
}

#[test]
fn section_reader_errors_map_to_privacy_safe_public_failures() {
    assert_eq!(
        section_read_failure(&SectionReadError::NotFound),
        KnowledgeError::Refused {
            code: "not_found",
            message: "section is unknown or not readable in the selected generation",
        }
    );
    assert_eq!(
        section_read_failure(&SectionReadError::Ambiguous),
        KnowledgeError::Refused {
            code: "ambiguous_section",
            message: "section identity is ambiguous in this chunk set",
        }
    );
    assert!(matches!(
        section_read_failure(&SectionReadError::TooLarge { bytes: 65_537 }),
        KnowledgeError::Refused {
            code: "response_too_large",
            ..
        }
    ));
    assert!(matches!(
        section_read_failure(&SectionReadError::Integrity),
        KnowledgeError::Failed {
            code: "integrity_error",
            ..
        }
    ));
    assert_eq!(
        section_read_failure(&SectionReadError::Store(Box::new(io::Error::other(
            "private backend detail",
        )))),
        KnowledgeError::Failed {
            code: "kernel_unavailable",
            message: "the local knowledge store is unavailable",
        }
    );
}
