//! Builds, prepares, publishes and evaluates the public synthetic collection.

use super::super::{
    failure::Failure,
    models::SyntheticModels,
    search::{self, QuestionRanking, SearchContext},
};
use super::{
    contract::{COLLECTION, Counts, Output, Provenance, SEED},
    fixture::{self, FixtureInputs, Scratch},
    readiness,
    report::metrics_complete,
};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::{
    artifact::Store,
    gateway::ModelCard,
    generation::{Generation, GenerationState},
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use maestro_knowledge::{
    eval::{self, Header, Report},
    import,
    index::{self, Projection, Qdrant},
    prepare::{self, RouterTokenizer},
    quality::{self, Ledger},
    suite::{Question, Suite},
};
use std::{collections::BTreeMap, env, path::Path};
use tokio::runtime::Runtime;

pub(super) struct BuiltGeneration {
    pub(super) database: Database,
    scopes: ScopeSet,
    import_report: import::Report,
    quality_report: quality::Report,
    prepared: prepare::Report,
    publication: index::Report,
    generation: Generation,
    pub(super) card: ModelCard,
    pub(super) models: SyntheticModels,
    pub(super) scratch: Scratch,
}

struct GenerationContext<'a> {
    runtime: &'a Runtime,
    qdrant: &'a Qdrant,
    database: &'a Database,
    scopes: &'a ScopeSet,
    data: &'a Path,
}

struct PublishedGeneration {
    prepared: prepare::Report,
    publication: index::Report,
    generation: Generation,
    card: ModelCard,
    models: SyntheticModels,
}

struct Evaluation {
    report: Report,
    rankings: Vec<QuestionRanking>,
    completed: usize,
}

pub(super) fn execute(
    runtime: &Runtime,
    qdrant: &Qdrant,
    backend: &str,
    generations: &mut Vec<i64>,
) -> Result<Output, Failure> {
    execute_internal(runtime, (qdrant, qdrant), backend, None, generations)
}

pub(super) fn execute_real(
    runtime: &Runtime,
    qdrant: &Qdrant,
    backend: &str,
    url: &str,
    generations: &mut Vec<i64>,
) -> Result<Output, Failure> {
    execute_internal(runtime, (qdrant, qdrant), backend, Some(url), generations)
}

pub(super) fn execute_with_search_qdrant(
    runtime: &Runtime,
    publish_qdrant: &Qdrant,
    search_qdrant: &Qdrant,
    backend: &str,
    generations: &mut Vec<i64>,
) -> Result<Output, Failure> {
    execute_internal(
        runtime,
        (publish_qdrant, search_qdrant),
        backend,
        None,
        generations,
    )
}

fn execute_internal(
    runtime: &Runtime,
    qdrants: (&Qdrant, &Qdrant),
    backend: &str,
    ready_url: Option<&str>,
    generations: &mut Vec<i64>,
) -> Result<Output, Failure> {
    let (publish_qdrant, search_qdrant) = qdrants;
    let fixture = fixture::load()?;
    let built = build_generation(runtime, publish_qdrant, &fixture, generations)?;
    if let Some(url) = ready_url {
        readiness::wait_for_green(runtime, &built.generation, built.publication.points, url)
            .map_err(|error| Failure::from_error("qdrant-ready", error))?;
    }
    evaluate_generation(runtime, search_qdrant, backend, &fixture, &built)
}

pub(super) fn build_generation(
    runtime: &Runtime,
    qdrant: &Qdrant,
    fixture: &FixtureInputs,
    generations: &mut Vec<i64>,
) -> Result<BuiltGeneration, Failure> {
    let scratch = Scratch::new()?;
    let data = scratch.path.join("kernel");
    let database =
        Database::open_in(&data).map_err(|error| Failure::from_error("kernel-open", error))?;
    let collection_scope: Scope = format!("workspace/default/collection/{COLLECTION}")
        .parse()
        .map_err(|error| Failure::from_error("scope-setup", error))?;
    database
        .grant(
            "synthetic-gate",
            &collection_scope,
            Right::Read,
            "synthetic gate",
        )
        .map_err(|error| Failure::from_error("scope-setup", error))?;
    let scopes = database
        .visible("synthetic-gate")
        .map_err(|error| Failure::from_error("scope-setup", error))?;
    let import_report = import_fixture(&database, &scopes, fixture)?;
    let quality_report = gate_quality(&database, &scopes, fixture)?;
    let context = GenerationContext {
        runtime,
        qdrant,
        database: &database,
        scopes: &scopes,
        data: &data,
    };
    let published = prepare_publish(&context, generations)?;
    Ok(BuiltGeneration {
        database,
        scopes,
        import_report,
        quality_report,
        prepared: published.prepared,
        publication: published.publication,
        generation: published.generation,
        card: published.card,
        models: published.models,
        scratch,
    })
}

fn import_fixture(
    database: &Database,
    scopes: &ScopeSet,
    fixture: &FixtureInputs,
) -> Result<import::Report, Failure> {
    let report = import::import(database, scopes, &fixture.declaration, &fixture.bindings)
        .map_err(|error| Failure::from_error("import", error))?;
    if report.imported + report.unchanged != 28 || report.held != 0 || report.refused != 0 {
        return Err(Failure::new("import-accounting"));
    }
    Ok(report)
}

fn gate_quality(
    database: &Database,
    scopes: &ScopeSet,
    fixture: &FixtureInputs,
) -> Result<quality::Report, Failure> {
    let ledger_path = fixture.declaration.quality.ledger.under(&fixture.root);
    if ledger_path.exists() {
        return Err(Failure::new("synthetic-ledger-must-be-absent"));
    }
    let ledger =
        Ledger::load(&ledger_path).map_err(|error| Failure::from_error("quality-ledger", error))?;
    if !ledger.rules().is_empty() {
        return Err(Failure::new("quality-ledger-accounting"));
    }
    let report = quality::gate(database, scopes, COLLECTION, &ledger)
        .map_err(|error| Failure::from_error("quality", error))?;
    let eligible = quality::eligible(database, scopes, COLLECTION)
        .map_err(|error| Failure::from_error("quality-eligibility", error))?;
    if report.revisions != 28
        || report.decided != 28
        || report.outcomes.accepted + report.outcomes.accepted_with_warnings != 28
        || !report.held.is_empty()
        || eligible.len() != 28
    {
        return Err(Failure::new("quality-accounting"));
    }
    Ok(report)
}

fn prepare_publish(
    context: &GenerationContext<'_>,
    generations: &mut Vec<i64>,
) -> Result<PublishedGeneration, Failure> {
    let card_store = Store::new(context.data.join("artifacts"));
    let card = SyntheticModels::card(&card_store)
        .map_err(|error| Failure::from_error("synthetic-model-card", error))?;
    let models = SyntheticModels::new()
        .map_err(|error| Failure::from_error("synthetic-model-fixtures", error))?;
    let tokenizer = RouterTokenizer::qualify(models.clone(), card.clone())
        .map_err(|error| Failure::from_error("synthetic-tokenizer-qualification", error))?;
    let prepared = prepare::prepare(context.database, context.scopes, COLLECTION, &tokenizer)
        .map_err(|error| Failure::from_error("prepare", error))?;
    if prepared.eligible != 28
        || prepared.prepared != 27
        || prepared.duplicates != 1
        || prepared.refused != 0
        || prepared.left_out != 0
        || prepared.chunks == 0
    {
        return Err(Failure::new("prepare-accounting"));
    }
    let projection = Projection {
        database: context.database,
        scopes: context.scopes,
        qdrant: context.qdrant,
        port: &models,
        card: &card,
    };
    let publication_result = context
        .runtime
        .block_on(projection.publish(&prepared.chunk_set));
    let recorded_generations = context
        .database
        .generations(context.scopes, COLLECTION)
        .map_err(|error| Failure::from_error("generation-readback", error))?;
    generations.extend(recorded_generations.iter().map(|generation| generation.id));
    let publication = publication_result.map_err(|error| Failure::from_error("publish", error))?;
    let generation = context
        .database
        .generation(context.scopes, publication.generation)
        .map_err(|error| Failure::from_error("generation-readback", error))?
        .ok_or_else(|| Failure::new("generation-readback"))?;
    if generation.state != GenerationState::Published
        || publication.points != prepared.chunks
        || publication.sparse_profile != "bm25-en-fr/1"
    {
        return Err(Failure::new("publish-accounting"));
    }
    Ok(PublishedGeneration {
        prepared,
        publication,
        generation,
        card,
        models,
    })
}

fn evaluate_generation(
    runtime: &Runtime,
    qdrant: &Qdrant,
    backend: &str,
    fixture: &FixtureInputs,
    built: &BuiltGeneration,
) -> Result<Output, Failure> {
    let corpus =
        search::generation_corpus(&built.database, &built.scopes, &built.prepared.chunk_set)?;
    if fixture
        .suite
        .questions
        .iter()
        .flat_map(|question| &question.expected)
        .any(|expected| !corpus.documents.contains_key(&expected.source_ref))
    {
        return Err(Failure::new("expected-document-membership"));
    }
    let profiles = profiles(
        &built.prepared,
        &built.publication,
        &built.card,
        backend,
        fixture,
    );
    let header = Header {
        suite: "synthetic".to_owned(),
        collection: COLLECTION.to_owned(),
        generation: built.generation.id,
        profiles: profiles.clone(),
        seed: SEED,
    };
    let context = SearchContext {
        runtime,
        database: &built.database,
        scopes: &built.scopes,
        generation: &built.generation,
        qdrant,
        card: &built.card,
        models: &built.models,
        corpus: &corpus,
    };
    let evaluation = evaluate_suite(&context, &fixture.suite, header)?;
    let embedded = built
        .models
        .embedded()
        .map_err(|error| Failure::from_error("synthetic-model-call-readback", error))?;
    let dense_queries = embedded.iter().filter(|call| call.len() == 1).count();
    if dense_queries < 56 {
        return Err(Failure::new("synthetic-model-call-accounting"));
    }
    let provenance = Provenance {
        schema: "maestro-synthetic-run/1",
        source_commit: env::var("GITHUB_SHA").ok(),
        suite: "synthetic",
        suite_digest: fixture.suite.digest.as_str().to_owned(),
        declaration_digest: fixture.declaration_digest.clone(),
        manifest_digest: fixture.manifest_digest.clone(),
        corpus_entries: fixture.corpus_digests.clone(),
        backend: backend.to_owned(),
        model: SyntheticModels::identity(),
        model_card_digest: built.card.digest().as_str().to_owned(),
        tokenizer_qualification: SyntheticModels::qualification(),
        profiles,
        answerable_questions: 48,
        unanswerable_questions: 8,
        baseline_digest: None,
    };
    Ok(Output {
        counts: Counts {
            imported: built.import_report.imported + built.import_report.unchanged,
            quality_decided: built.quality_report.decided,
            accepted: built.quality_report.outcomes.accepted
                + built.quality_report.outcomes.accepted_with_warnings,
            eligible: built.prepared.eligible,
            prepared: built.prepared.prepared,
            duplicates: built.prepared.duplicates,
            chunks: built.prepared.chunks,
            published_points: built.publication.points,
            dense_queries: u64::try_from(dense_queries).unwrap_or(u64::MAX),
            completed_questions: u64::try_from(evaluation.completed).unwrap_or(u64::MAX),
        },
        report: evaluation.report,
        rankings: evaluation.rankings,
        provenance,
        baseline_digest: None,
        model_identity: SyntheticModels::identity(),
    })
}

fn evaluate_suite(
    context: &SearchContext<'_>,
    suite: &Suite,
    header: Header,
) -> Result<Evaluation, Failure> {
    let mut rankings = Vec::with_capacity(suite.questions.len());
    let mut completed = 0_usize;
    let report = eval::run(
        header,
        suite,
        |source_ref| {
            Ok::<Option<CanonicalDocument>, Failure>(
                context.corpus.documents.get(source_ref).cloned(),
            )
        },
        |question: &Question| {
            let (bundle, ranking) = search::retrieve(context, question, completed)?;
            rankings.push(ranking);
            completed += 1;
            Ok(bundle)
        },
    )
    .map_err(|error| evaluation_failure(error, completed))?;
    if completed != 56
        || rankings.len() != 56
        || report.degraded_searches != 0
        || !metrics_complete(&report)
    {
        return Err(Failure::new("evaluation-accounting"));
    }
    Ok(Evaluation {
        report,
        rankings,
        completed,
    })
}

fn evaluation_failure(error: eval::RunError<Failure>, completed: usize) -> Failure {
    let cause = error.to_string();
    match error {
        eval::RunError::Documents { source_ref, .. } => {
            Failure::question_from_error("evaluation-document-read", &source_ref, completed, &cause)
        }
        eval::RunError::NoDocument { question, .. } => Failure::question_from_error(
            "evaluation-generation-membership",
            &question,
            completed,
            &cause,
        ),
        eval::RunError::Unresolved { question, .. }
        | eval::RunError::SectionExtent { question, .. } => Failure::question_from_error(
            "evaluation-expected-section",
            &question,
            completed,
            &cause,
        ),
        eval::RunError::SameSection { question, .. } => Failure::question_from_error(
            "evaluation-duplicate-section",
            &question,
            completed,
            &cause,
        ),
        eval::RunError::SameDocument { question, .. } => Failure::question_from_error(
            "evaluation-duplicate-document",
            &question,
            completed,
            &cause,
        ),
        eval::RunError::Retrieval { error, .. } => error,
        eval::RunError::Recording { .. } => Failure::new("evaluation-recording"),
        eval::RunError::InvalidV2Header { .. } => Failure::new("evaluation-v2-header"),
        eval::RunError::InvalidV2Report { .. } => Failure::new("evaluation-v2-report"),
        eval::RunError::OtherGeneration { question, .. } => Failure::question_from_error(
            "evaluation-generation-mismatch",
            &question,
            completed,
            &cause,
        ),
    }
}

fn profiles(
    prepared: &prepare::Report,
    publication: &index::Report,
    card: &ModelCard,
    backend: &str,
    fixture: &FixtureInputs,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "adapter".to_owned(),
            "test-adapter/dense-lexical-fusion+T029c-reader/no-T032/1".to_owned(),
        ),
        (
            "abstention".to_owned(),
            "test-adapter/no-T032-policy/1".to_owned(),
        ),
        ("backend".to_owned(), backend.to_owned()),
        (
            "collection_declaration".to_owned(),
            fixture.declaration_digest.clone(),
        ),
        (
            "corpus_manifest".to_owned(),
            fixture.manifest_digest.clone(),
        ),
        ("chunking".to_owned(), prepared.chunk_profile.clone()),
        ("counter".to_owned(), prepared.counter.clone()),
        ("dense".to_owned(), publication.embedding_profile.clone()),
        (
            "evidence_count".to_owned(),
            "utf8-bytes/estimated-test-adapter".to_owned(),
        ),
        ("k".to_owned(), "10".to_owned()),
        ("max_tokens".to_owned(), "6000".to_owned()),
        ("deadline_ms".to_owned(), "10000".to_owned()),
        ("model_card".to_owned(), card.digest().as_str().to_owned()),
        ("model_inference".to_owned(), "fake-models/1".to_owned()),
        ("qualification".to_owned(), SyntheticModels::qualification()),
        (
            "qdrant_search".to_owned(),
            concat!(
                "dense_exact=false;sparse_exact=false;hnsw_ef=server-default/1;",
                "route_window=2x;tie_tolerance=1e-5*max(1,abs(leader));",
                "tie_order=chunk_id"
            )
            .to_owned(),
        ),
        ("rerank_depth".to_owned(), "80-not-run".to_owned()),
        (
            "reranker".to_owned(),
            "not-run/approved-dense-lexical-fusion-adapter/1".to_owned(),
        ),
        ("sparse".to_owned(), publication.sparse_profile.clone()),
        ("suite".to_owned(), "synthetic".to_owned()),
    ])
}
