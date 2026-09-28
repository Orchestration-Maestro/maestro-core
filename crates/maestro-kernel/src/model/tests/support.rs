//! Scratch databases and v2 card fixtures for model persistence tests.

use crate::{
    artifact::{Digest, Store},
    document::Collection,
    gateway::{
        Limits, ModelCard, Role, RouterEntry,
        card_v2::{
            Backend, CachePolicy, Capability, CardFormats, CardIdentity, Dimensions,
            EmbeddingFormat, FlagValue, HardwareIdentity, KvCache, KvCacheType, MemoryEstimate,
            Normalization, Observation, OffloadMode, OffloadPolicy, Pooling, Provenance,
            QualificationMethod, QualifiedLimits, Quantization, Resources, RuntimeLimits, Sampling,
            SamplingParameters, Template, TextFormat, TokenizerDerivation, WeightIdentity,
        },
    },
    model::{
        CardRecord, EvaluationDisposition, EvaluationMode, EvaluationRecord, NewModelCard,
        NewModelEvaluation, NewModelSelection, SelectionRecord,
    },
    paths::{self, Environment},
    scope::{Config, LOCAL, Right, Scope, ScopeSet},
    store::{Database, Error as StoreError},
};
use rusqlite::Connection;
use std::{
    collections::BTreeMap,
    env, fs,
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

pub(super) struct Scratch(pub(super) PathBuf);

impl Scratch {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "maestro-model-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(super) fn open(&self) -> Database {
        Database::open(&self.0.join("kernel.sqlite3"), &self.0.join("artifacts")).unwrap()
    }

    pub(super) fn store(&self) -> Store {
        Store::new(self.0.join("cards"))
    }

    pub(super) fn outside(&self) -> Connection {
        Connection::open(self.0.join("kernel.sqlite3")).unwrap()
    }
}

pub(super) fn corrupt_artifact(scratch: &Scratch, digest: &Digest) {
    let hex = digest.as_str();
    let path = scratch
        .0
        .join("artifacts/sha256")
        .join(hex.get(..2).unwrap())
        .join(hex.get(2..4).unwrap())
        .join(hex);
    fs::write(path, b"corrupted artifact").unwrap();
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

pub(super) fn collection(database: &Database, id: &str) {
    database
        .record_collection(&Collection {
            id: id.to_owned(),
            title: id.to_owned(),
            visibility: "private".to_owned(),
            profiles: BTreeMap::default(),
        })
        .unwrap();
}

pub(super) fn grant(database: &Database, principal: &str, scope: &str) -> ScopeSet {
    database
        .grant(
            principal,
            &scope.parse::<Scope>().unwrap(),
            Right::Read,
            "test",
        )
        .unwrap();
    database.visible(principal).unwrap()
}

#[expect(
    clippy::too_many_lines,
    reason = "the fixture spells out every required immutable identity field"
)]
pub(super) fn identity(database: &Database) -> CardIdentity {
    let weight_bytes = b"synthetic model weights";
    let gguf_digest = database
        .put(weight_bytes, "application/octet-stream")
        .unwrap();
    let runtime_digest = database
        .put(b"runtime binary", "application/octet-stream")
        .unwrap();
    let qualification_digest = database
        .put(b"native tokenizer qualification", "application/json")
        .unwrap();
    CardIdentity {
        role: Role::Embedder,
        router_entry: RouterEntry::parse("embed").unwrap(),
        weights: WeightIdentity {
            upstream_model_id: "test/model".to_owned(),
            upstream_revision: "0123456789abcdef".to_owned(),
            source_url: "https://example.invalid/model".to_owned(),
            licence_id: "apache-2.0".to_owned(),
            licence_terms_source: "https://example.invalid/license".to_owned(),
            gguf_digest: gguf_digest.clone(),
            gguf_bytes: NonZeroU64::new(weight_bytes.len() as u64).unwrap(),
            quantization: Quantization::Q8_0,
            adapters: BTreeMap::default(),
            drafts: BTreeMap::default(),
            projectors: BTreeMap::default(),
        },
        formats: CardFormats {
            tokenizer_digest: gguf_digest.clone(),
            tokenizer_derivation: TokenizerDerivation::GgufEmbedded,
            qualification_digest: qualification_digest.clone(),
            template: Template::Absent,
            system_format: String::new(),
            tool_format: Capability::Unsupported,
            document: Capability::Supported(TextFormat {
                prefix: String::new(),
                suffix: String::new(),
            }),
            query: Capability::Supported(TextFormat {
                prefix: String::new(),
                suffix: String::new(),
            }),
            embedding: EmbeddingFormat::Supported {
                pooling: Pooling::LastToken,
                normalization: Normalization::L2,
            },
        },
        invocation: RuntimeLimits {
            limits: Limits {
                context_tokens: NonZeroU32::new(4096).unwrap(),
                output_tokens: None,
            },
            dimensions: Dimensions::Measured(NonZeroUsize::new(1024).unwrap()),
            sampling: Sampling::NotApplicable,
            reasoning: Capability::NotApplicable,
            llama_cpp_build: "build-123".to_owned(),
            runtime_binary_digest: runtime_digest,
            backend: Backend::Cuda,
            server_flags: [(
                "--model".to_owned(),
                FlagValue::Asset {
                    name: "weights".to_owned(),
                    digest: gguf_digest,
                },
            )]
            .into(),
        },
        resources: Resources {
            offload: OffloadPolicy {
                mode: OffloadMode::ExplicitDevices,
                devices: vec!["gpu0".to_owned()],
            },
            memory_estimate: MemoryEstimate {
                bytes: NonZeroU64::new(1_000_000).unwrap(),
                source: "synthetic estimate".to_owned(),
            },
            slots: NonZeroU32::new(1).unwrap(),
            kv_cache: KvCache {
                key: KvCacheType::Q8_0,
                value: KvCacheType::Q8_0,
            },
            cache_policy: CachePolicy::KeepLoaded,
            batch_size: NonZeroU32::new(128).unwrap(),
            micro_batch_size: NonZeroU32::new(32).unwrap(),
            hardware: HardwareIdentity {
                device: "synthetic GPU".to_owned(),
                driver_version: "synthetic".to_owned(),
                host_cpu: "synthetic CPU".to_owned(),
                host_memory_bytes: Observation::Unavailable {
                    reason: "not measured".to_owned(),
                },
            },
            qualified_limits: QualifiedLimits {
                context_tokens: Observation::Measured {
                    value: NonZeroU32::new(4096).unwrap(),
                    provenance: "synthetic qualification".to_owned(),
                },
                output_tokens: Observation::Unavailable {
                    reason: "not applicable".to_owned(),
                },
                concurrency: Observation::Measured {
                    value: NonZeroU32::new(1).unwrap(),
                    provenance: "synthetic qualification".to_owned(),
                },
                peak_memory_bytes: Observation::Unavailable {
                    reason: "not measured".to_owned(),
                },
            },
        },
        provenance: Provenance {
            identity_created: "2026-09-27".to_owned(),
            qualification_created: "2026-09-27".to_owned(),
            qualification_method: QualificationMethod::NativeTokenizer,
            tool_versions: [("tokenizer".to_owned(), "1".to_owned())].into(),
            artifacts: [("qualification".to_owned(), qualification_digest)].into(),
        },
    }
}

pub(super) fn card(database: &Database, scratch: &Scratch) -> ModelCard {
    ModelCard::record_v2(&scratch.store(), &identity(database)).unwrap()
}

pub(super) fn card_for_role(database: &Database, scratch: &Scratch, role: Role) -> ModelCard {
    let mut identity = identity(database);
    identity.role = role;
    if role != Role::Embedder {
        identity.invocation.dimensions = Dimensions::NotApplicable;
        identity.formats.embedding = EmbeddingFormat::NotApplicable;
        identity.formats.document = Capability::NotApplicable;
        identity.formats.query = Capability::NotApplicable;
    }
    if role == Role::Answerer {
        identity.invocation.sampling = Sampling::Configured(SamplingParameters {
            temperature: 0.1,
            top_p: 0.9,
            top_k: 40,
            min_p: 0.0,
            typical_p: 1.0,
            repeat_penalty: 1.0,
            frequency_penalty: 0.0,
            presence_penalty: 0.0,
            seed: Some(1),
        });
        identity.invocation.reasoning = Capability::Unsupported;
        identity.invocation.limits.output_tokens = NonZeroU32::new(128);
    }
    ModelCard::record_v2(&scratch.store(), &identity).unwrap()
}

pub(super) fn collection_scopes(database: &Database, id: &str) -> ScopeSet {
    grant(
        database,
        &format!("reader-{id}"),
        &format!("workspace/default/collection/{id}"),
    )
}

pub(super) fn empty_scopes(database: &Database) -> ScopeSet {
    database.visible("nobody").unwrap()
}

#[expect(
    clippy::too_many_arguments,
    reason = "test calls name each required evaluation fact directly"
)]
pub(super) fn new_eval<'a>(
    card_id: ulid::Ulid,
    collection_id: &'a str,
    run_id: &'a str,
    mode: EvaluationMode,
    disposition: EvaluationDisposition,
    report: &'a [u8],
    manifest: &'a [u8],
) -> NewModelEvaluation<'a> {
    NewModelEvaluation {
        run_id,
        collection_id,
        card_id,
        role: Role::Embedder,
        mode,
        generation_id: None,
        disposition,
        manifest,
        report,
    }
}

pub(super) fn populated_model_records(
    scratch: &Scratch,
) -> (CardRecord, EvaluationRecord, SelectionRecord) {
    let database = scratch.open();
    collection(&database, "one");
    let scopes = collection_scopes(&database, "one");
    let card = card(&database, scratch);
    let registration = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "one",
                card: &card,
            },
        )
        .unwrap();
    let evaluation = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registration.id,
                "one",
                "run",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                b"report",
                b"manifest",
            ),
        )
        .unwrap();
    let selection = database
        .record_model_selection(
            &scopes,
            &NewModelSelection {
                collection_id: "one",
                role: registration.role,
                card_id: registration.id,
                evaluation_id: evaluation.id,
                selected_by: "owner",
                reason: "approved",
            },
        )
        .unwrap();
    (registration, evaluation, selection)
}

/// A building generation of `collection`, over a complete chunk set.
pub(super) fn generation_in(database: &Database, collection: &str) -> i64 {
    database
        .write(|transaction| {
            transaction.execute(
                "INSERT INTO chunk_sets \
                     (id,collection_id,chunk_profile,counter_contract_id,state) \
                     VALUES (?1 || '-set',?1,'structural-500-700/1','native','building')",
                [collection],
            )?;
            transaction.execute(
                "UPDATE chunk_sets SET state='complete',manifest_digest='manifest' \
                     WHERE id=?1 || '-set'",
                [collection],
            )?;
            let id = transaction.query_row(
                "INSERT INTO generations \
                     (collection_id,chunk_set_id,embedding_profile,sparse_profile) \
                     VALUES (?1,?1 || '-set','embed:test','bm25-en-fr/1') RETURNING id",
                [collection],
                |row| row.get(0),
            )?;
            Ok::<_, StoreError>(id)
        })
        .unwrap()
}

/// The local kernel, the local principal's scopes and the kernel's data
/// directory, as a live test opens them.
pub(super) fn live_kernel() -> (Database, ScopeSet, PathBuf) {
    let environment = Environment::current();
    let data = paths::data_dir(&environment).expect("resolve kernel data home");
    let config =
        Config::load(&paths::config_dir(&environment).expect("resolve kernel config home"))
            .expect("read kernel access config");
    let database = Database::open_in(&data).expect("open default kernel");
    database
        .apply_config(&config)
        .expect("apply kernel access config");
    let scopes = database.visible(LOCAL).expect("read local kernel scopes");
    (database, scopes, data)
}

/// The value of the environment variable `variable`, which a live test
/// requires.
pub(super) fn required(variable: &str) -> String {
    env::var(variable).unwrap_or_else(|_| panic!("set {variable}"))
}
