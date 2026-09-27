//! Search this machine's published collection against its live Qdrant and model
//! router, writing per-question evidence to a JSONL file without printing text.
//!
//! Point `XDG_DATA_HOME` at a copy of the kernel to leave the real one alone.
//! Set `MAESTRO_SEARCH_COLLECTION`, `MAESTRO_SEARCH_QUESTIONS`,
//! `MAESTRO_SEARCH_OUTPUT`, `MAESTRO_QDRANT_URL`, and `MAESTRO_ROUTER_URL`.
//! Each question line needs string `id` and `question` fields; other fields are
//! ignored. An optional `MAESTRO_RERANK_CARD` enables reranking when its digest
//! names a card in this kernel's artifact store.
//!
//! Run explicitly with:
//! `cargo test -p maestro-knowledge --test it search_live -- --ignored --nocapture`

#![cfg(test)]

use super::{
    live_router::required,
    publish_live::{card_of, kernel},
};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::{
    artifact::{Digest, Store},
    chunk_set::Chunk,
    evidence::{RequestBudget, RouteStatus},
    gateway::{ModelCard, RouterClient, Url},
    scope::{LOCAL, ScopeSet},
    store::Database,
};
use maestro_knowledge::{
    index::Qdrant,
    search::{
        EvidenceInput, Ranked, Reranker, SearchContext, SearchRequest, pin,
        routes::dense::Embedder, search,
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    env,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
    num::NonZeroUsize,
    path::Path,
    sync::Arc,
    time::Instant,
};

#[derive(Deserialize)]
struct Question {
    id: String,
    question: String,
}

#[derive(Default, serde::Serialize)]
struct Availability {
    ok: usize,
    unavailable: usize,
}

/// Loads a question's optional reranker card only when its artifact is present.
fn reranker_card(data: &Path) -> Option<ModelCard> {
    let digest = env::var("MAESTRO_RERANK_CARD").ok()?;
    let digest = Digest::parse(&digest).ok()?;
    ModelCard::load(&Store::new(data.join("artifacts")), &digest).ok()
}

/// Reads only the required string fields from each JSONL object.
fn read_questions(path: &str) -> Vec<Question> {
    BufReader::new(File::open(path).unwrap())
        .lines()
        .map(|line| serde_json::from_str::<Question>(&line.unwrap()).unwrap())
        .collect()
}

/// Records route status and availability without nesting reporting logic in the search loop.
fn route_record(
    name: &str,
    status: &RouteStatus,
    availability: &mut BTreeMap<String, Availability>,
) -> (String, Value) {
    let availability = availability.entry(name.to_owned()).or_default();
    let (status, reason) = match status {
        RouteStatus::Ok => {
            availability.ok += 1;
            ("ok", None)
        }
        RouteStatus::Unavailable(reason) => {
            availability.unavailable += 1;
            ("unavailable", Some(reason.as_str()))
        }
    };
    (name.to_owned(), json!({"status": status, "reason": reason}))
}

/// The read-only metadata and availability accumulated for the JSONL report.
struct ReportContext<'a> {
    database: &'a Database,
    scopes: &'a ScopeSet,
    chunks: &'a HashMap<String, Chunk>,
    availability: BTreeMap<String, Availability>,
}

impl ReportContext<'_> {
    /// Builds one JSONL result and records the routes that participated.
    fn result(&mut self, question: &Question, elapsed_ms: u128, evidence: &EvidenceInput) -> Value {
        let routes = evidence
            .routes
            .iter()
            .map(|(name, status)| route_record(name, status, &mut self.availability))
            .collect::<BTreeMap<_, _>>();
        let candidates = evidence
            .ranked
            .iter()
            .take(10)
            .map(|ranked| candidate_json(self.database, self.scopes, self.chunks, ranked))
            .collect::<Vec<_>>();
        json!({
            "id": question.id,
            "elapsed_ms": elapsed_ms,
            "routes": routes,
            "candidates": candidates,
        })
    }
}

/// Returns one candidate's route ranks and document provenance without exposing it on stdout.
fn candidate_json(
    database: &Database,
    scopes: &ScopeSet,
    chunks: &HashMap<String, Chunk>,
    ranked: &Ranked,
) -> Value {
    let fused = &ranked.candidate.fused;
    let chunk = chunks.get(&fused.chunk_id).unwrap();
    let revision = database
        .revision(scopes, &chunk.revision_id)
        .unwrap()
        .unwrap();
    let document = database
        .document(scopes, &revision.document_id)
        .unwrap()
        .unwrap();
    let canonical: CanonicalDocument =
        serde_json::from_slice(&database.get(&revision.canonical_digest).unwrap()).unwrap();
    let heading_path = chunk
        .section_id
        .as_deref()
        .and_then(|section_id| {
            canonical
                .sections
                .iter()
                .find(|section| section.section_id == section_id)
        })
        .map(|section| section.heading_path.as_slice())
        .unwrap_or_default();
    let ranks = fused
        .ranks
        .iter()
        .map(|(route, rank)| (route.name(), rank.get()))
        .collect::<BTreeMap<_, _>>();
    let text = ranked.candidate.text.chars().take(200).collect::<String>();
    json!({
        "chunk_id": fused.chunk_id,
        "source_ref": document.source_ref,
        "title": canonical.source_metadata.title,
        "heading_path": heading_path,
        "route_ranks": ranks,
        "fusion_score": fused.score,
        "rerank_score": ranked.score,
        "text": text,
    })
}

/// Returns the nearest-rank latency percentile from sorted samples.
fn percentile(sorted: &[u128], percent: usize) -> Option<u128> {
    if sorted.is_empty() {
        return None;
    }
    let rank = sorted.len().saturating_mul(percent).div_ceil(100).max(1);
    sorted.get(rank - 1).copied()
}

#[cfg(test)]
mod tests {
    use super::{Question, percentile};

    #[test]
    fn latency_percentiles_use_nearest_rank() {
        assert_eq!(percentile(&[10, 20, 30, 40], 50), Some(20));
        assert_eq!(percentile(&[10, 20, 30, 40], 95), Some(40));
        assert_eq!(percentile(&[], 95), None);
    }

    #[test]
    fn question_input_ignores_unrequested_fields() {
        let question: Question =
            serde_json::from_str(r#"{"id":"q1","question":"Where?","ignored":true}"#).unwrap();
        assert_eq!(question.id, "q1");
        assert_eq!(question.question, "Where?");
    }
}

#[tokio::test]
#[ignore = "searches this machine's live kernel, Qdrant and model router; run explicitly"]
async fn questions_run_against_the_published_generation() {
    let collection = required("MAESTRO_SEARCH_COLLECTION");
    let questions_path = required("MAESTRO_SEARCH_QUESTIONS");
    let output_path = required("MAESTRO_SEARCH_OUTPUT");
    let qdrant = Qdrant::new(&required("MAESTRO_QDRANT_URL")).unwrap();
    let router = Url::parse(&required("MAESTRO_ROUTER_URL")).unwrap();
    let (database, scopes, data) = kernel();
    let generation = pin(&database, &scopes, &collection).unwrap();
    let set = database
        .chunk_set(&scopes, &generation.chunk_set_id)
        .unwrap()
        .unwrap();
    let embedder_card = card_of(&set, &data);
    let reranker_card = reranker_card(&data);
    let port = RouterClient::new(router).unwrap();
    let database = Arc::new(database);
    let chunks = database
        .chunks(&scopes, &generation.chunk_set_id)
        .unwrap()
        .into_iter()
        .map(|chunk| (chunk.id.clone(), chunk))
        .collect::<HashMap<_, _>>();
    let questions = read_questions(&questions_path);
    let mut output = BufWriter::new(File::create(output_path).unwrap());
    let mut latencies = Vec::with_capacity(questions.len());
    let mut report = ReportContext {
        database: &database,
        scopes: &scopes,
        chunks: &chunks,
        availability: BTreeMap::new(),
    };
    let context = SearchContext {
        database: Arc::clone(&database),
        principal: LOCAL,
        qdrant: &qdrant,
        embedder: Some(Embedder {
            port: &port,
            card: &embedder_card,
        }),
        reranker: reranker_card
            .as_ref()
            .map(|card| Reranker { port: &port, card }),
    };

    for question in &questions {
        let request = SearchRequest {
            collection: &collection,
            text: &question.question,
            version: None,
            budget: RequestBudget::default(),
            rerank_depth: NonZeroUsize::new(50).unwrap(),
        };
        let started = Instant::now();
        let evidence = Box::pin(search(&context, &request)).await.unwrap();
        let elapsed_ms = started.elapsed().as_millis();
        latencies.push(elapsed_ms);
        let line = report.result(question, elapsed_ms, &evidence);
        serde_json::to_writer(&mut output, &line).unwrap();
        output.write_all(b"\n").unwrap();
    }
    output.flush().unwrap();

    latencies.sort_unstable();
    println!(
        "{}",
        serde_json::to_string(&json!({
            "questions_run": questions.len(),
            "route_availability": report.availability,
            "p50_ms": percentile(&latencies, 50),
            "p95_ms": percentile(&latencies, 95),
        }))
        .unwrap()
    );
}
