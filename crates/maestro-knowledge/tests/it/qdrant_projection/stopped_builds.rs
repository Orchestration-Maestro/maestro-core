//! What a publication refuses before any work, what the kernel holds that a
//! point cannot be read from, and a Qdrant that refuses or cannot be
//! reached: each stops the build, and a rerun resumes the generation left
//! building. Only the fake can refuse on demand, so these run against it.

use super::{
    backends::{Backend, fake},
    kernel::Kernel,
    models::{Embedder, card, embedder},
    support::{collection_of, projection, publish, state_of},
};
use maestro_kernel::{
    chunk_set::ChunkSetState, gateway::Role, generation::GenerationState, scope::Right,
};
use maestro_knowledge::index::{Error, Projection, QdrantError};
use qdrant_client::QdrantError as ClientError;
use std::net::TcpListener;
use tonic::Code;

/// Whether `kernel` records no generation.
fn no_generation(kernel: &Kernel) -> bool {
    let generations = kernel
        .database
        .generations(&kernel.scopes, &kernel.collection);
    generations.unwrap().is_empty()
}

#[tokio::test]
async fn a_card_that_is_not_an_embedders_is_refused_before_any_work() {
    let (kernel, backend) = (Kernel::with_guides(1), fake());
    let reranker = card(Role::Reranker, 0);
    let error = publish(&kernel, &backend, &Embedder::default(), &reranker)
        .await
        .unwrap_err();
    assert!(
        matches!(
            &error,
            Error::NotAnEmbedder { card, role: Role::Reranker } if card == reranker.digest()
        ),
        "{error}"
    );
    assert!(no_generation(&kernel));
}

#[tokio::test]
async fn a_chunk_set_the_caller_cannot_read_or_that_is_not_complete_is_refused() {
    let (kernel, backend) = (Kernel::with_guides(1), fake());
    let (qdrant, card, port) = (backend.client(), embedder(8), Embedder::default());
    let publication = projection(&kernel, &qdrant, &port, &card);
    let error = publication.publish("chunk-set-unknown").await.unwrap_err();
    assert!(
        matches!(&error, Error::UnknownChunkSet(id) if id == "chunk-set-unknown"),
        "{error}"
    );
    kernel.begin_another("chunk-set-building", 1);
    let error = publication.publish("chunk-set-building").await.unwrap_err();
    assert!(
        matches!(
            &error,
            Error::Incomplete { chunk_set, state: ChunkSetState::Building }
                if chunk_set == "chunk-set-building"
        ),
        "{error}"
    );
    let elsewhere = "workspace/default/collection/elsewhere".parse().unwrap();
    kernel
        .database
        .grant("stranger", &elsewhere, Right::Read, "test")
        .unwrap();
    let strangers = kernel.database.visible("stranger").unwrap();
    let stranger = Projection {
        scopes: &strangers,
        ..publication
    };
    let error = stranger.publish(&kernel.chunk_set).await.unwrap_err();
    assert!(
        matches!(&error, Error::UnknownChunkSet(id) if *id == kernel.chunk_set),
        "{error}"
    );
    assert!(no_generation(&kernel));
    assert!(port.calls().is_empty());
}

#[tokio::test]
async fn a_qdrant_that_refuses_a_batch_leaves_the_generation_building() {
    let (kernel, backend) = (Kernel::with_guides(30), fake());
    let fake = backend.fake.as_ref().unwrap();
    // The client tries an unavailable server a second time before it fails.
    fake.refuse_next("upsert", Code::Unavailable);
    fake.refuse_next("upsert", Code::Unavailable);
    let (card, port) = (embedder(8), Embedder::default());
    let error = publish(&kernel, &backend, &port, &card).await.unwrap_err();
    assert!(
        matches!(
            &error,
            Error::Qdrant(QdrantError::Client(ClientError::ResponseError { status }))
                if status.code() == Code::Unavailable
                    && status.message() == "the fake was told to refuse"
        ),
        "{error}"
    );
    assert_eq!(state_of(&kernel, 1), GenerationState::Building);
    assert_eq!(backend.count(&collection_of(&kernel, 1)).await, 0);
    let report = publish(&kernel, &backend, &port, &card).await.unwrap();
    assert_eq!((report.generation, report.points), (1, 91));
}

#[tokio::test]
async fn a_qdrant_answer_without_its_result_leaves_the_generation_building() {
    for (call, missing) in [
        ("count", "no count of"),
        ("collection_info", "no parameters of"),
    ] {
        let (kernel, backend) = (Kernel::with_guides(1), fake());
        backend.fake.as_ref().unwrap().hollow_next(call);
        let (card, port) = (embedder(8), Embedder::default());
        let error = publish(&kernel, &backend, &port, &card).await.unwrap_err();
        let expected = format!("{missing} {}", collection_of(&kernel, 1));
        assert!(
            matches!(
                &error,
                Error::Qdrant(QdrantError::InvalidAnswer(reason)) if *reason == expected
            ),
            "{call}: {error}"
        );
        assert_eq!(state_of(&kernel, 1), GenerationState::Building, "{call}");
        let report = publish(&kernel, &backend, &port, &card).await.unwrap();
        assert_eq!((report.generation, report.points), (1, 4), "{call}");
    }
}

#[tokio::test]
async fn a_qdrant_out_of_reach_leaves_the_generation_building() {
    let kernel = Kernel::with_guides(1);
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    };
    let unreachable = Backend {
        name: "closed",
        url: closed,
        fake: None,
    };
    let (card, port) = (embedder(8), Embedder::default());
    let error = publish(&kernel, &unreachable, &port, &card)
        .await
        .unwrap_err();
    assert!(
        matches!(&error, Error::Qdrant(QdrantError::Client(_))),
        "{error}"
    );
    assert_eq!(state_of(&kernel, 1), GenerationState::Building);
    let report = publish(&kernel, &fake(), &port, &card).await.unwrap();
    assert_eq!((report.generation, report.points), (1, 4));
}

#[tokio::test]
async fn a_chunk_whose_section_or_input_cannot_be_read_stops_the_build() {
    let unknown_section = Kernel::with_changed_guides(2, &|_, guide, mut chunks| {
        if guide == 1 {
            chunks[1].section_id = Some("section-of-another-revision".to_owned());
        }
        chunks
    });
    let not_text = Kernel::with_changed_guides(2, &|kernel, guide, mut chunks| {
        if guide == 1 {
            chunks[2].digest = kernel.put(&[0xff, 0xfe, 0x00]);
        }
        chunks
    });
    for (kernel, chunk, reason) in [
        (
            &unknown_section,
            "chunk-1-1",
            "its section section-of-another-revision is not one of its revision ",
        ),
        (&not_text, "chunk-1-2", "its prepared input sha256:"),
    ] {
        let error = publish(kernel, &fake(), &Embedder::default(), &embedder(8))
            .await
            .unwrap_err();
        assert!(
            matches!(
                &error,
                Error::Unreadable { chunk: found, reason: why }
                    if found == chunk && why.starts_with(reason)
            ),
            "{error}"
        );
        assert_eq!(state_of(kernel, 1), GenerationState::Building);
    }
}
