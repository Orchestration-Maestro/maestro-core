//! Each blocking search stage reads its cutoff on its request's clock.
//!
//! Just before the cutoff a stage completes; at the cutoff it times out.

use super::{
    clock::{ManualClock, control_at, just_before},
    support::CandidateDb,
};
use crate::{
    index::Qdrant,
    search::{
        DEADLINE_EXCEEDED, Query, SearchError,
        admission::{Admission, admit},
        candidates::{self, Failure, check_control},
        routes::{identifier::kernel_leg, structured::search_structured},
    },
};
use maestro_kernel::{
    evidence::RouteStatus,
    retrieval::{self, Clock, InventoryRequest},
};
use maestro_test_clock::on_stopped_clock;
use std::future;
use std::{
    sync::Arc,
    time::{Duration, Instant as StdInstant},
};
use tokio::time::Instant;

/// A fixture whose one chunk carries the identifier `ERR-042`.
fn searchable() -> CandidateDb {
    let mut fixture = CandidateDb::new(b"Run ERR-042 before the upgrade.", "docs");
    fixture.publish_searchable();
    fixture
}

/// A cutoff an hour away: only the stage's clock can reach it.
fn cutoff() -> Instant {
    Instant::now() + Duration::from_secs(3600)
}

/// Blocking clocks reading just before, then exactly at, `cutoff`.
fn clocks(cutoff: Instant) -> [Arc<dyn Clock>; 2] {
    let cutoff = cutoff.into_std();
    [
        ManualClock::at(just_before(cutoff)),
        ManualClock::at(cutoff),
    ]
}

#[tokio::test]
async fn admission_reads_its_cutoff_on_the_admitted_clock() {
    let fixture = searchable();
    let deadline = cutoff();
    let [before, at] = clocks(deadline);
    let admit_on = |clock| {
        on_stopped_clock(
            future::pending(),
            admit(
                fixture.database.clone(),
                Admission {
                    principal: "reader".to_owned(),
                    collection: fixture.generation.collection_id.clone(),
                    version: Some("1.0".to_owned()),
                    deadline,
                    clock,
                },
            ),
        )
    };

    assert!(admit_on(before).await.is_ok());
    assert!(matches!(
        admit_on(at).await,
        Err(SearchError::Kernel(retrieval::Error::TimedOut))
    ));
}

#[tokio::test]
async fn candidate_loading_reads_its_cutoff_on_the_request_clock() {
    let fixture = searchable();
    let deadline = cutoff();
    let load_on = |clock| {
        let mut request = fixture.request(&fixture.chunk_id, vec![fixture.revision_id.clone()]);
        request.deadline = deadline;
        request.context_deadline = deadline;
        request.clock = clock;
        on_stopped_clock(
            future::pending(),
            candidates::load(fixture.database.clone(), request),
        )
    };
    let [before, at] = clocks(deadline);

    assert_eq!(load_on(before).await.unwrap().candidates.len(), 1);
    assert!(matches!(load_on(at).await, Err(Failure::TimedOut)));
}

#[tokio::test]
async fn the_identifier_kernel_leg_reads_its_cutoff_on_the_query_clock() {
    let fixture = searchable();
    let qdrant = Qdrant::new("http://127.0.0.1:1").unwrap();
    let deadline = cutoff();
    let [before, at] = clocks(deadline);
    let identifiers = ["ERR-042".to_owned()];
    let query = |clock| Query {
        generation: &fixture.generation,
        scopes: &fixture.scopes,
        text: "ERR-042",
        limit: 10,
        identifier_limit: 20,
        version: None,
        projection: &qdrant,
        clock,
    };

    let found = on_stopped_clock(
        future::pending(),
        kernel_leg(
            &query(&before),
            fixture.database.clone(),
            &identifiers,
            10,
            deadline,
        ),
    )
    .await
    .unwrap();
    assert_eq!(found.hits.len(), 1);
    assert_eq!(
        on_stopped_clock(
            future::pending(),
            kernel_leg(
                &query(&at),
                fixture.database.clone(),
                &identifiers,
                10,
                deadline
            )
        )
        .await
        .err()
        .as_deref(),
        Some(DEADLINE_EXCEEDED)
    );
}

#[tokio::test]
async fn the_structured_route_reads_its_cutoff_on_the_query_clock() {
    let fixture = searchable();
    let qdrant = Qdrant::new("http://127.0.0.1:1").unwrap();
    let deadline = cutoff();
    let [before, at] = clocks(deadline);
    let request = InventoryRequest::DocumentsBySet { set: None };
    let query = |clock| Query {
        generation: &fixture.generation,
        scopes: &fixture.scopes,
        text: "how many documents",
        limit: 10,
        identifier_limit: 20,
        version: None,
        projection: &qdrant,
        clock,
    };

    let counted = on_stopped_clock(
        future::pending(),
        search_structured(
            &query(&before),
            fixture.database.clone(),
            &request,
            deadline,
        ),
    )
    .await;
    assert_eq!(counted.route.status, RouteStatus::Ok);
    let late = on_stopped_clock(
        future::pending(),
        search_structured(&query(&at), fixture.database.clone(), &request, deadline),
    )
    .await;
    assert_eq!(
        late.route.status,
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
}

#[test]
fn candidate_control_times_out_exactly_at_the_deadline() {
    let deadline = StdInstant::now() + Duration::from_secs(3600);
    assert!(check_control(&control_at(deadline, just_before(deadline))).is_ok());
    assert!(matches!(
        check_control(&control_at(deadline, deadline)),
        Err(Failure::TimedOut)
    ));
}
