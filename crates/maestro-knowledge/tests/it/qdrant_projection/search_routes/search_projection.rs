//! Publication's identifier payload and readiness marker.

use super::super::support::{point_id, projection};
use super::{backends::backends, kernel::Kernel, models, support::cleanup};
use maestro_kernel::{chunk_set::Chunk, generation::GenerationState};
use qdrant_client::qdrant::value::Kind;

const INPUT: &str = "Run `ctm repair --force` at ERR-042 on /etc/app.conf port:8443 v2.4";

/// Changes only the first guide's lead input before its chunk-set write.
fn replace_input(kernel: &Kernel, guide: usize, mut chunks: Vec<Chunk>) -> Vec<Chunk> {
    if guide == 0 {
        chunks[0].digest = kernel.put(INPUT.as_bytes());
    }
    chunks
}

#[tokio::test]
async fn publication_indexes_exact_identifiers_and_marks_search_ready() {
    for backend in backends("publication_indexes_exact_identifiers_and_marks_search_ready") {
        let kernel = Kernel::with_changed_guides(1, &replace_input);
        let card = models::embedder(3);
        let port = models::Embedder::default();
        let qdrant = backend.client();
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
        let point = backend
            .point(&report.qdrant_collection, point_id(&chunk.id))
            .await;
        let profile = point
            .payload
            .get("identifier_profile")
            .and_then(|value| value.kind.as_ref());
        assert!(matches!(profile, Some(Kind::StringValue(value)) if value == "identifiers/1"));
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
        assert_eq!(
            kernel
                .database
                .generation_search(&kernel.scopes, generation.id)
                .unwrap()
                .unwrap()
                .identifier_profile,
            "identifiers/1"
        );
        assert!(
            kernel
                .database
                .generation_search(&kernel.scopes, generation.id)
                .unwrap()
                .unwrap()
                .ready
        );
        assert_eq!(generation.state, GenerationState::Published);
        cleanup(&backend, &[&generation]).await;
    }
}
