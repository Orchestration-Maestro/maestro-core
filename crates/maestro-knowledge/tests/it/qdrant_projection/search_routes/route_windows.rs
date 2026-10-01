//! A route's window comes from the request's budget, less the time the later
//! stages keep: a route slower than a fixed window still contributes, and
//! one that outlasts the budget is dropped with the search still ending
//! within its deadline and evidence assembly keeping its time. Each search
//! runs on a stopped clock, which only the doubles' slow answers and the
//! cutoffs move, however loaded the host.

use super::super::{
    fake::SlowQuery,
    stopped_clock::{StageEnd, on_stopped_clock},
};
use super::configured_search::{Published, clean, context, published};
use super::models::{self, Answers, SlowReranker};
use maestro_kernel::{
    evidence::{RequestBudget, RouteStatus},
    gateway::Role,
};
use maestro_knowledge::search::{
    DEADLINE_EXCEEDED, Route, SearchConfiguration, SearchRequest, StageWindow,
    evidence::{EvidenceCounter, EvidenceSettings, assemble_evidence},
    search,
};
use std::{collections::BTreeMap, future, time::Duration};
use tokio::time::Instant;
use tonic::Code;

/// The search deadline of these tests. Evidence assembly keeps 650 ms and
/// fusion and the rerank keep 300 ms, so a route may run until 550 ms.
const DEADLINE: Duration = Duration::from_millis(1500);
/// When the routes of a search under [`DEADLINE`] must end.
const ROUTES_END: Duration = Duration::from_millis(550);
/// Longer than the fixed 300 ms route window the routes used to have, and
/// shorter than [`ROUTES_END`].
const SLOW: Duration = Duration::from_millis(400);
/// The fixed route window of the experiments' knob.
const FIXED: Duration = Duration::from_millis(300);

/// When a search's stopped clock may move on to its next cutoff.
#[derive(Clone, Copy)]
enum Clock {
    /// Never: only the doubles' slow answers move it.
    Held,
    /// Once the stage span of this name records its outcome, when the other
    /// routes wait on nothing but a cutoff.
    HeldUntil(&'static str),
}

/// Until the lexical route ended, for a search whose dense route hangs.
const UNTIL_LEXICAL: Clock = Clock::HeldUntil("retrieval.route.lexical");

/// How one search ended.
struct Searched {
    /// Each route's and the rerank's status.
    routes: BTreeMap<String, RouteStatus>,
    /// The candidates each route contributed.
    route_hits: BTreeMap<Route, usize>,
    /// How long the search took, up to its handover to evidence assembly.
    elapsed: Duration,
    /// The time left before the deadline at that handover.
    left: Duration,
    /// The passages evidence assembly then delivered.
    passages: usize,
}

/// Searches `fixture` for "scheduler" within `deadline` on `clock`,
/// reranking with a reranker when `reranks`, then assembles its evidence.
async fn search_within(
    fixture: &Published,
    deadline: Duration,
    reranks: bool,
    clock: Clock,
) -> Searched {
    search_with_window(fixture, deadline, reranks, StageWindow::Derived, clock).await
}

/// Searches as [`search_within`] does, with the route window `stage_window`.
async fn search_with_window(
    fixture: &Published,
    deadline: Duration,
    reranks: bool,
    stage_window: StageWindow,
    clock: Clock,
) -> Searched {
    let reranker_card = models::card(Role::Reranker, 3);
    let search_context = context(fixture, reranks.then_some(&reranker_card));
    let request = SearchRequest {
        evidence: EvidenceSettings::default(),
        collection: &fixture.kernel.collection,
        text: "scheduler",
        version: None,
        budget: RequestBudget {
            deadline_ms: u32::try_from(deadline.as_millis()).unwrap(),
            ..RequestBudget::default()
        },
        configuration: SearchConfiguration {
            stage_window,
            ..SearchConfiguration::default()
        },
    };
    let searched = || async {
        let started = Instant::now();
        let input = Box::pin(search(&search_context, &request)).await.unwrap();
        let elapsed = started.elapsed();
        let left = input.deadline.saturating_duration_since(Instant::now());
        let routes = input.routes.clone();
        let route_hits = input
            .observations
            .route_ranks
            .iter()
            .map(|(route, ranks)| (*route, ranks.len()))
            .collect();
        let bundle = assemble_evidence(
            fixture.kernel.database.clone(),
            input,
            EvidenceCounter::Utf8Bytes,
        )
        .await
        .unwrap();
        assert!(started.elapsed() < deadline, "{:?}", started.elapsed());
        Searched {
            routes,
            route_hits,
            elapsed,
            left,
            passages: bundle.passages.len(),
        }
    };
    match clock {
        Clock::Held => on_stopped_clock(future::pending(), searched).await,
        Clock::HeldUntil(stage) => {
            let end = StageEnd::watch(stage);
            on_stopped_clock(end.ended(), searched).await
        }
    }
}

/// Makes the next query of the fake Qdrant's vector `using` wait `delay`,
/// or never answer when `None`, then answer, or refuse with `refusal`.
fn slow_query(
    fixture: &Published,
    using: &'static str,
    delay: Option<Duration>,
    refusal: Option<Code>,
) {
    fixture
        .backend
        .fake
        .as_ref()
        .unwrap()
        .slow_next_query(SlowQuery {
            using,
            delay,
            refusal,
        });
}

/// Asserts that `route` ran to its end with candidates, and that the search
/// delivered passages.
fn assert_contributed(searched: &Searched, route: Route) {
    assert_eq!(searched.routes[route.name()], RouteStatus::Ok);
    assert!(searched.route_hits[&route] > 0);
    assert!(searched.passages > 0);
}

/// Asserts that `route` was dropped only once the routes' share of the
/// budget was spent, and that another route still delivered passages.
fn assert_dropped_at_the_routes_end(searched: &Searched, route: Route) {
    assert_eq!(
        searched.routes[route.name()],
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
    assert!(searched.elapsed >= ROUTES_END, "{:?}", searched.elapsed);
    assert!(searched.passages > 0);
}

/// Asserts that the search handed over with at least `reserve` left.
fn assert_left(searched: &Searched, reserve: Duration) {
    assert!(searched.left >= reserve, "{:?}", searched.left);
}

#[tokio::test]
async fn a_dense_query_slower_than_a_fixed_window_still_contributes() {
    let fixture = published().await;
    fixture.port.delay_embeddings(Answers::After(SLOW));

    let searched = search_within(&fixture, DEADLINE, false, Clock::Held).await;

    assert_contributed(&searched, Route::Dense);
    assert!(searched.elapsed >= SLOW, "{:?}", searched.elapsed);
    clean(&fixture).await;
}

#[tokio::test]
async fn a_lexical_query_slower_than_a_fixed_window_still_contributes() {
    let fixture = published().await;
    slow_query(&fixture, "bm25", Some(SLOW), None);

    let searched = search_within(&fixture, DEADLINE, false, Clock::Held).await;

    assert_contributed(&searched, Route::Lexical);
    assert!(searched.elapsed >= SLOW, "{:?}", searched.elapsed);
    clean(&fixture).await;
}

#[tokio::test]
async fn a_dense_query_outlasting_the_budget_is_dropped_within_the_deadline() {
    let fixture = published().await;
    fixture.port.delay_embeddings(Answers::Never);

    let searched = search_within(&fixture, DEADLINE, false, UNTIL_LEXICAL).await;

    assert_dropped_at_the_routes_end(&searched, Route::Dense);
    assert_eq!(searched.routes["lexical"], RouteStatus::Ok);
    assert_left(&searched, Duration::from_millis(650));
    clean(&fixture).await;
}

#[tokio::test]
async fn a_lexical_query_outlasting_the_budget_is_dropped_within_the_deadline() {
    let fixture = published().await;
    slow_query(&fixture, "bm25", None, None);

    let clock = Clock::HeldUntil("retrieval.route.dense");
    let searched = search_within(&fixture, DEADLINE, false, clock).await;

    assert_dropped_at_the_routes_end(&searched, Route::Lexical);
    assert_eq!(searched.routes["dense"], RouteStatus::Ok);
    assert_left(&searched, Duration::from_millis(650));
    clean(&fixture).await;
}

#[tokio::test]
async fn evidence_assembly_keeps_its_reserve_behind_a_hanging_route_and_rerank() {
    let mut fixture = published().await;
    fixture.port = models::Embedder::with_slow_reranker(SlowReranker::Hanging);
    fixture.port.delay_embeddings(Answers::Never);

    let searched = search_within(&fixture, DEADLINE, true, UNTIL_LEXICAL).await;

    assert_dropped_at_the_routes_end(&searched, Route::Dense);
    assert_eq!(
        searched.routes["rerank"],
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
    // The rerank ends at 850 ms; the 650 ms after it are assembly's, less
    // the millisecond the timer's deadline rounds up to.
    assert_left(&searched, Duration::from_millis(649));
    clean(&fixture).await;
}

// On Windows, a connection Qdrant refuses takes about 2 s to fail, and the
// client tries a refused connection twice: the route reports its own
// failure instead of a passed deadline. Scaled down, the two refusals take
// longer than the old fixed window together, and end within the budget.
#[tokio::test]
async fn a_slow_refused_connection_ends_its_route_with_its_own_reason() {
    let fixture = published().await;
    let refused_after = Duration::from_millis(200);
    for _ in 0..2 {
        slow_query(
            &fixture,
            "dense",
            Some(refused_after),
            Some(Code::Unavailable),
        );
    }

    let searched = search_within(&fixture, DEADLINE, false, Clock::Held).await;

    assert_eq!(
        searched.routes["dense"],
        RouteStatus::Unavailable("Qdrant search failed".to_owned())
    );
    assert!(
        searched.elapsed >= refused_after * 2,
        "{:?}",
        searched.elapsed
    );
    assert_contributed(&searched, Route::Lexical);
    clean(&fixture).await;
}

#[tokio::test]
async fn a_fixed_window_for_experiments_drops_a_route_slower_than_it() {
    let fixture = published().await;
    fixture.port.delay_embeddings(Answers::Never);

    let searched = search_with_window(
        &fixture,
        DEADLINE,
        false,
        StageWindow::Fixed(FIXED),
        UNTIL_LEXICAL,
    )
    .await;

    assert_eq!(
        searched.routes["dense"],
        RouteStatus::Unavailable(DEADLINE_EXCEEDED.to_owned())
    );
    // The window, not the routes' end, dropped it.
    assert!(
        FIXED <= searched.elapsed && searched.elapsed < SLOW,
        "{:?}",
        searched.elapsed
    );
    clean(&fixture).await;
}
