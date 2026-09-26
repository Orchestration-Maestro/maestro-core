//! What each refusal of a publication says, and the cause each keeps.

use super::super::{Error, Failure, QdrantError, Refusal, Unverified};
use maestro_kernel::{
    artifact::Digest,
    chunk_set::{self, ChunkSetState},
    document, gateway,
    gateway::Role,
    generation, store,
};
use qdrant_client::QdrantError as ClientError;
use std::{error::Error as _, time::Duration};
use tonic::Status;

#[test]
fn a_refusal_before_any_work_or_at_a_check_has_no_cause() {
    let stops = [
        (
            Error::NotAnEmbedder {
                card: Digest::of(b"card"),
                role: Role::Reranker,
            },
            format!(
                "the model card sha256:{} is a reranker's: a generation's dense vectors need an \
                 embedder's",
                Digest::of(b"card").as_str()
            ),
        ),
        (
            Error::UnknownChunkSet("chunk-set-1".to_owned()),
            "no chunk set chunk-set-1 is recorded that the caller can read".to_owned(),
        ),
        (
            Error::Incomplete {
                chunk_set: "chunk-set-1".to_owned(),
                state: ChunkSetState::Building,
            },
            "the chunk set chunk-set-1 is building: only a complete chunk set is published"
                .to_owned(),
        ),
        (
            Error::Unreadable {
                chunk: "chunk-1".to_owned(),
                reason: "its prepared input is not UTF-8".to_owned(),
            },
            "the chunk chunk-1 cannot be read as its point needs: its prepared input is not UTF-8"
                .to_owned(),
        ),
        (
            Error::Unverified {
                generation: 3,
                reason: Unverified::Missing {
                    chunk: "chunk-1".to_owned(),
                },
            },
            "generation 3 failed its check, and the alias stays where it was: its collection \
             holds no point of the chunk chunk-1"
                .to_owned(),
        ),
        (
            Error::Stopped,
            "the publication was stopped by its caller: its generation stays building, and a \
             rerun resumes it"
                .to_owned(),
        ),
    ];
    for (stop, says) in stops {
        assert_eq!(stop.to_string(), says);
        assert!(stop.source().is_none(), "{stop}");
    }
}

#[test]
fn a_stop_part_way_says_what_failed_and_keeps_its_cause() {
    let sqlite = || store::Error::Sqlite(rusqlite::Error::InvalidQuery);
    let stops = [
        (
            Error::Embedding {
                at: 64,
                failure: Failure::TimedOut(Duration::from_millis(1500)),
            },
            "the batch from chunk 64 has no vectors, and the generation stays building: the \
             embedder gave no answer within 1.5 s",
        ),
        (
            Error::Qdrant(QdrantError::InvalidAnswer("no count of c".to_owned())),
            "Qdrant failed: Qdrant's answer is not what was asked for: no count of c",
        ),
        (
            Error::Generation(generation::Error::UnknownGeneration(3)),
            "the generation failed: no generation 3 is recorded",
        ),
        (
            Error::ChunkSet(chunk_set::Error::UnknownChunkSet("chunk-set-1".to_owned())),
            "the chunk set failed: no chunk set chunk-set-1 is recorded",
        ),
        (
            Error::Records(document::Error::Store(sqlite())),
            "the kernel's records failed: the kernel database refused the operation",
        ),
        (
            Error::Artifacts(sqlite()),
            "the artifact store failed: the kernel database refused the operation",
        ),
    ];
    for (stop, says) in stops {
        assert_eq!(stop.to_string(), says);
        let cause = stop.source().map(ToString::to_string).unwrap();
        assert!(says.ends_with(&cause), "{says} ends with {cause}");
    }
}

#[test]
fn each_check_a_collection_fails_says_what_it_found() {
    let checks = [
        (
            Unverified::Vectors {
                dimensions: 8,
                found: "no dense vector `dense` and no sparse vector `bm25`".to_owned(),
            },
            "its collection has no dense vector `dense` and no sparse vector `bm25`, where the \
             generation needs a dense vector `dense` of 8 dimensions compared by cosine and a \
             sparse vector `bm25` weighted by IDF",
        ),
        (
            Unverified::Count {
                expected: 10,
                found: 11,
            },
            "its collection holds 11 points, where its chunk set has 10 chunks",
        ),
    ];
    for (check, says) in checks {
        assert_eq!(check.to_string(), says);
    }
}

#[test]
fn each_refusal_of_a_batch_names_the_input_it_found_at_fault() {
    let refusals = [
        (
            Refusal::Count {
                inputs: 3,
                vectors: 2,
            },
            "2 vectors came back for 3 inputs",
        ),
        (
            Refusal::Dimensions {
                input: 1,
                expected: 8,
                found: 7,
            },
            "the vector of input 1 has 7 dimensions, where the card records 8",
        ),
        (
            Refusal::NonFinite { input: 2 },
            "the vector of input 2 holds a value that is not finite",
        ),
        (
            Refusal::Zero { input: 0 },
            "the vector of input 0 is zero, which has no direction",
        ),
    ];
    for (refusal, says) in refusals {
        let failure = Failure::Refused(refusal);
        assert_eq!(failure.to_string(), format!("the batch is refused: {says}"));
        assert!(failure.source().is_none());
    }
}

#[test]
fn a_port_that_refused_is_the_cause_of_its_failure() {
    let failure = Failure::Port(gateway::Error::Unavailable {
        reason: "no free room".to_owned(),
    });
    assert_eq!(
        failure.to_string(),
        "the embedder refused: the model is unavailable: no free room"
    );
    let cause = failure.source().map(ToString::to_string);
    assert_eq!(
        cause.as_deref(),
        Some("the model is unavailable: no free room")
    );
    assert!(Failure::TimedOut(Duration::from_secs(1)).source().is_none());
}

#[test]
fn qdrant_s_refusals_say_what_they_hold() {
    let status = Status::not_found("Not found: Collection `c` doesn't exist!");
    let refused = QdrantError::Client(ClientError::from(status));
    let cause = refused.source().map(ToString::to_string).unwrap();
    assert!(
        cause.contains("Not found: Collection `c` doesn't exist!"),
        "{cause}"
    );
    assert_eq!(refused.to_string(), format!("Qdrant refused: {cause}"));
    assert!(QdrantError::InvalidAnswer(String::new()).source().is_none());
}
