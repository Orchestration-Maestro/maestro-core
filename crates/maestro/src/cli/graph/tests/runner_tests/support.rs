use crate::kernel::Kernel;
use maestro_canonicalization::{CanonicalizeInput, canonicalize};
use maestro_kernel::{
    artifact::{Digest, Store},
    document::{
        Collection, Disposition, Document, Outcome, Revision, RevisionStatus,
        Source as KernelSource,
    },
    facts::{Budget, BuildPlan},
    gateway::{
        Candidate, CardFields, ChatRequest, Error, ExtractRequest, FakeModels, Limits, ModelCard,
        ModelPort, Role, Room, RouterEntry,
    },
    scope::Right,
    store::Database,
};
use maestro_knowledge::graph::{
    build,
    extract::{ModelExtractor, WindowPolicy},
    rules::Extractor,
    verify::Source as GraphSource,
};
use serde_json::{Map, json};
use std::{
    collections::BTreeMap,
    fs,
    future::{Future, ready},
    num::NonZeroU32,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Debug)]
pub(super) struct CountingModels {
    extracts: Arc<AtomicUsize>,
    tokenizes: Arc<AtomicUsize>,
}

impl ModelPort for CountingModels {
    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        FakeModels.embed(card, room, inputs).await
    }

    async fn rerank(
        &self,
        card: &ModelCard,
        room: Room,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<f64>, Error> {
        FakeModels.rerank(card, room, query, documents).await
    }

    fn tokenize(
        &self,
        _card: &ModelCard,
        _room: Room,
        _text: &str,
    ) -> impl Future<Output = Result<Vec<u32>, Error>> + Send {
        self.tokenizes.fetch_add(1, Ordering::Relaxed);
        ready(Ok(vec![1]))
    }

    async fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        request: &ChatRequest,
    ) -> Result<String, Error> {
        FakeModels.chat(card, room, request).await
    }

    fn extract(
        &self,
        _card: &ModelCard,
        _request: &ExtractRequest,
    ) -> impl Future<Output = Result<Vec<Candidate>, Error>> + Send {
        self.extracts.fetch_add(1, Ordering::Relaxed);
        ready(Ok(Vec::new()))
    }
}

pub(super) struct Fixture {
    pub(super) root: PathBuf,
    pub(super) kernel: Kernel,
    pub(super) revisions: Vec<Revision>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("remove graph runner fixture");
    }
}

pub(super) fn fixture(markdowns: &[&str]) -> Fixture {
    let root = maestro_test_scratch::scratch_directory().unwrap();
    let database = Database::open_in(&root).unwrap();
    database
        .grant(
            "graph-test",
            &"workspace/default".parse().unwrap(),
            Right::Read,
            "test",
        )
        .unwrap();
    let scopes = database.visible("graph-test").unwrap();
    database
        .record_collection(&Collection {
            id: "graph-test".into(),
            title: "Graph test".into(),
            visibility: "public".into(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    database
        .record_source(&KernelSource {
            collection_id: "graph-test".into(),
            id: "fixture".into(),
            kind: "import".into(),
            transport: None,
            reference: "synthetic".into(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    let mut revisions = Vec::new();
    for (index, markdown) in markdowns.iter().enumerate() {
        let id = format!("doc-{index}");
        database
            .record_document(&Document {
                id: id.clone(),
                collection_id: "graph-test".into(),
                source_id: "fixture".into(),
                source_ref: format!("corpus-path:{index}.md"),
            })
            .unwrap();
        let canonical = canonicalize(CanonicalizeInput::new(markdown, "synthetic.md")).unwrap();
        let original_digest = database.put(markdown.as_bytes(), "text/markdown").unwrap();
        let canonical_digest = database
            .put(&serde_json::to_vec(&canonical).unwrap(), "application/json")
            .unwrap();
        let revision = Revision {
            id: canonical.revision_id.clone(),
            document_id: id,
            original_digest,
            canonical_digest,
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: Map::new(),
        };
        database.record_revision(&revision).unwrap();
        database
            .record_disposition(&Disposition {
                revision_id: revision.id.clone(),
                outcome: Outcome::Accepted,
                reasons: Vec::new(),
                rule_ids: Vec::new(),
                decided_by: "test".into(),
            })
            .unwrap();
        revisions.push(revision);
    }
    Fixture {
        root: root.clone(),
        kernel: Kernel {
            database: Arc::new(database),
            artifacts: Store::new(root.join("artifacts")),
            scopes,
            config_dir: PathBuf::new(),
            test_refresh_hook: None,
        },
        revisions,
    }
}

pub(super) fn extractor(
    fixture: &Fixture,
    budget: usize,
) -> (
    ModelExtractor<CountingModels>,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    PathBuf,
) {
    let root = fixture.root.join("card");
    let fields = CardFields {
        role: Role::Answerer,
        router_entry: RouterEntry::parse("synthetic").unwrap(),
        file_digest: Digest::of(b"synthetic model"),
        template_digest: None,
        server_build: "synthetic".into(),
        dimensions: None,
        limits: Limits {
            context_tokens: NonZeroU32::new(4096).unwrap(),
            output_tokens: NonZeroU32::new(128),
        },
        suite_results: Vec::new(),
    };
    let card = ModelCard::record(&Store::new(&root), &fields).unwrap();
    let extracts = Arc::new(AtomicUsize::new(0));
    let tokenizes = Arc::new(AtomicUsize::new(0));
    let extractor = ModelExtractor::new(
        CountingModels {
            extracts: Arc::clone(&extracts),
            tokenizes: Arc::clone(&tokenizes),
        },
        card,
        WindowPolicy {
            schema: "maestro-graph-window-policy/1".into(),
            max_window_bytes: 4096,
            overlap_bytes: 0,
            max_windows: 8,
        },
        Digest::of(b"policy-bytes"),
        budget,
    )
    .unwrap();
    (extractor, extracts, tokenizes, root)
}

pub(super) fn plan(revisions: &[Revision], extractor: &dyn Extractor) -> BuildPlan {
    BuildPlan {
        collection_id: "graph-test".into(),
        provenance: extractor.provenance(),
        sources: revisions
            .iter()
            .map(|revision| revision.id.clone())
            .collect(),
        budget: Budget {
            max_claims: 100,
            max_rejections: 100,
        },
    }
}

pub(super) fn frozen_inputs(
    plan: &BuildPlan,
    revisions: &[Revision],
    extractor: &dyn Extractor,
    database: &Database,
) -> serde_json::Value {
    let estimate = revisions
        .iter()
        .map(|revision| {
            let source = GraphSource::read(database, revision).unwrap();
            extractor.estimated_tokens(&source).unwrap()
        })
        .sum::<usize>();
    let mut extra = extractor.job_inputs().unwrap();
    extra["estimated_tokens"] = json!(estimate);
    build::inputs(plan, Some(extra))
}
