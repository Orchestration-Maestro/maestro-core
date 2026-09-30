//! Publication's identifier payload and readiness marker.

use super::super::stopped_clock::on_stopped_clock;
use super::super::support::{point_id, projection};
use super::{
    backends::{Backend, backends},
    kernel::Kernel,
    models,
    support::cleanup,
};
use maestro_kernel::{
    chunk_set::Chunk,
    document::{Disposition, Outcome},
    generation::{Generation, GenerationState},
    retrieval::{IDENTIFIER_PROFILE, ReadControl, SearchRead},
};
use maestro_knowledge::search::RuntimeClock;
use qdrant_client::qdrant::value::Kind;
use std::{
    future,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

const INPUT: &str = "Run `ctm repair --force` at ERR-042 on /etc/app.conf port:8443 v2.4";

/// Changes only the first guide's lead input before its chunk-set write.
fn replace_input(kernel: &Kernel, guide: usize, mut chunks: Vec<Chunk>) -> Vec<Chunk> {
    if guide == 0 {
        chunks[0].digest = kernel.put(INPUT.as_bytes());
    }
    chunks
}

/// Confirms the published point carries every exact identifier family value.
async fn assert_published_identifiers(backend: &Backend, collection: &str, chunk: &Chunk) {
    let point = backend.point(collection, point_id(&chunk.id)).await;
    let profile = point
        .payload
        .get("identifier_profile")
        .and_then(|value| value.kind.as_ref());
    assert!(matches!(profile, Some(Kind::StringValue(value)) if value == IDENTIFIER_PROFILE));
    let identifiers = point
        .payload
        .get("identifiers")
        .and_then(|value| value.kind.as_ref());
    let Some(Kind::ListValue(identifiers)) = identifiers else {
        panic!(
            "published point lacks its identifier array: {:?}",
            point.payload
        );
    };
    let identifiers: Vec<_> = identifiers
        .values
        .iter()
        .filter_map(|value| match value.kind.as_ref() {
            Some(Kind::StringValue(value)) => Some(value.as_str()),
            _ => None,
        })
        .collect();
    for expected in [
        "--force",
        "/etc/app.conf",
        "8443",
        "ERR-042",
        "app.conf",
        "ctm repair --force",
        "repair --force",
        "v2.4",
    ] {
        assert!(
            identifiers.contains(&expected),
            "{}: {expected:?} in {identifiers:?}",
            backend.name
        );
    }
}

/// Confirms the same projection is indexed and readable by the kernel.
fn assert_kernel_identifiers(kernel: &Kernel, generation: &Generation, chunk: &Chunk) {
    assert_eq!(
        kernel
            .database
            .generation_search(&kernel.scopes, generation.id)
            .unwrap()
            .unwrap()
            .identifier_profile,
        IDENTIFIER_PROFILE
    );
    let connection = rusqlite::Connection::open(kernel.database_path()).unwrap();
    let indexed_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM chunk_search_identifiers
             WHERE chunk_set_id = ?1 AND identifier = 'ERR-042' AND chunk_id = ?2",
            rusqlite::params![kernel.chunk_set, chunk.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(indexed_count, 1);
    let control = ReadControl {
        deadline: Instant::now() + Duration::from_secs(5),
        clock: Arc::new(RuntimeClock::current()),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let read = SearchRead {
        generation,
        scopes: &kernel.scopes,
        version: None,
        control: &control,
    };
    let indexed = kernel
        .database
        .identifier_hits(&read, &["ERR-042".to_owned()], 10)
        .unwrap();
    assert!(indexed.too_common.is_empty());
    assert!(indexed.hits.iter().any(|hit| hit.chunk_id == chunk.id));
    assert!(
        kernel
            .database
            .generation_search(&kernel.scopes, generation.id)
            .unwrap()
            .unwrap()
            .ready
    );
}

#[tokio::test]
async fn publication_indexes_exact_identifiers_and_marks_search_ready() {
    for backend in backends("publication_indexes_exact_identifiers_and_marks_search_ready") {
        let kernel = Kernel::with_changed_guides(3, &replace_input);
        let lead = kernel
            .chunks()
            .into_iter()
            .find(|chunk| chunk.id == "chunk-0-lead")
            .unwrap();
        kernel
            .database
            .record_disposition(&Disposition {
                revision_id: lead.revision_id,
                outcome: Outcome::Accepted,
                reasons: Vec::new(),
                rule_ids: Vec::new(),
                decided_by: "test".to_owned(),
            })
            .unwrap();
        let card = models::embedder(3);
        let port = models::Embedder::default();
        let qdrant = backend.client();
        on_stopped_clock(future::pending(), || async {
            let report = projection(&kernel, &qdrant, &port, &card)
                .publish(&kernel.chunk_set)
                .await
                .unwrap();
            let generation = kernel
                .database
                .generation(&kernel.scopes, report.generation)
                .unwrap()
                .unwrap();
            let chunk = kernel
                .chunks()
                .into_iter()
                .find(|chunk| chunk.id == "chunk-0-lead")
                .unwrap();
            assert_published_identifiers(&backend, &report.qdrant_collection, &chunk).await;
            assert_kernel_identifiers(&kernel, &generation, &chunk);
            assert_eq!(generation.state, GenerationState::Published);
            cleanup(&backend, &[&generation]).await;
        })
        .await;
    }
}
