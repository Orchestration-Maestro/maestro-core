//! Request-boundary checks before generation admission.

use crate::search::{
    admission::{ensure_permissions, validate},
    request::{SearchConfiguration, SearchError, SearchRequest},
    rerank::DEFAULT_DEPTH,
};
use maestro_kernel::{evidence::RequestBudget, store::Database};
use std::{
    env, fs,
    num::NonZeroUsize,
    process,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::time::Instant;

fn request<'a>(
    text: &'a str,
    budget: RequestBudget,
    depth: usize,
    version: Option<&'a str>,
) -> SearchRequest<'a> {
    SearchRequest {
        collection: "collection",
        text,
        version,
        budget,
        configuration: SearchConfiguration {
            rerank_depth: NonZeroUsize::new(depth).unwrap(),
            ..SearchConfiguration::default()
        },
    }
}

#[test]
fn search_request_constructor_uses_the_measured_default_depth() {
    let request = SearchRequest::new("docs", "question", None, RequestBudget::default());
    assert_eq!(request.configuration.rerank_depth, DEFAULT_DEPTH);
    assert_eq!(DEFAULT_DEPTH.get(), 30);
}

fn rejected(request: &SearchRequest<'_>, expected: &str) {
    assert!(matches!(
        validate(request),
        Err(SearchError::InvalidRequest { reason }) if reason == expected
    ));
}

#[tokio::test]
async fn an_expired_permission_recheck_reports_its_deadline() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = env::temp_dir().join(format!(
        "maestro-search-permission-deadline-{}-{}",
        process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    let database = Arc::new(Database::open_in(&path).unwrap());
    let scopes = database.visible("reader").unwrap();
    assert!(matches!(
        ensure_permissions(database.clone(), "reader", &scopes, Instant::now()).await,
        Err(SearchError::PermissionCheckTimedOut)
    ));
    drop(database);
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn request_bounds_include_both_endpoints_and_reject_the_next_value() {
    let valid = RequestBudget {
        k: 50,
        max_tokens: 12_000,
        deadline_ms: 10_000,
    };
    assert!(validate(&request("query", valid, 120, Some(&"v".repeat(256)))).is_ok());
    let valid_low = RequestBudget {
        k: 1,
        max_tokens: 1,
        deadline_ms: 1,
    };
    assert!(validate(&request("query", valid_low, 1, None)).is_ok());

    for (budget, expected) in [
        (
            RequestBudget { k: 0, ..valid_low },
            "k must be between 1 and 50",
        ),
        (
            RequestBudget { k: 51, ..valid },
            "k must be between 1 and 50",
        ),
        (
            RequestBudget {
                max_tokens: 0,
                ..valid_low
            },
            "max_tokens must be between 1 and 12000",
        ),
        (
            RequestBudget {
                max_tokens: 12_001,
                ..valid
            },
            "max_tokens must be between 1 and 12000",
        ),
        (
            RequestBudget {
                deadline_ms: 0,
                ..valid_low
            },
            "deadline_ms must be between 1 and 10000",
        ),
        (
            RequestBudget {
                deadline_ms: 10_001,
                ..valid
            },
            "deadline_ms must be between 1 and 10000",
        ),
    ] {
        rejected(&request("query", budget, 1, None), expected);
    }
    rejected(
        &request("query", valid_low, 121, None),
        "rerank depth must be between 1 and 120",
    );
    rejected(
        &request("query", valid_low, 1, Some("")),
        "version must contain 1 to 256 UTF-8 bytes",
    );
    rejected(
        &request("query", valid_low, 1, Some(&"v".repeat(257))),
        "version must contain 1 to 256 UTF-8 bytes",
    );
}

#[test]
fn text_and_distinct_identifier_bounds_are_enforced() {
    let budget = RequestBudget::default();
    assert!(validate(&request(&"x".repeat(8192), budget, 1, None)).is_ok());
    rejected(
        &request(&"x".repeat(8193), budget, 1, None),
        "query exceeds 8192 UTF-8 bytes",
    );
    rejected(
        &request(" \n\t ", budget, 1, None),
        "query must not be blank",
    );

    let within = (0..64)
        .map(|index| format!("ERR-{index:03}"))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(validate(&request(&within, budget, 1, None)).is_ok());
    let over = (0..65)
        .map(|index| format!("ERR-{index:03}"))
        .collect::<Vec<_>>()
        .join(" ");
    rejected(
        &request(&over, budget, 1, None),
        "query has more than 64 distinct identifiers",
    );
}

/// A request with every route weight at the default and one set by `set`.
fn weighted(set: impl Fn(&mut SearchConfiguration)) -> SearchRequest<'static> {
    let mut request = request("query", RequestBudget::default(), 1, None);
    set(&mut request.configuration);
    request
}

#[test]
fn each_route_weight_must_be_finite_and_nonnegative() {
    let setters: [fn(&mut SearchConfiguration, f64); 4] = [
        |configuration, weight| configuration.dense_weight = weight,
        |configuration, weight| configuration.lexical_weight = weight,
        |configuration, weight| configuration.identifier_weight = weight,
        |configuration, weight| configuration.structured_weight = weight,
    ];
    for set in setters {
        for weight in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            -f64::MIN_POSITIVE,
        ] {
            rejected(
                &weighted(|configuration| set(configuration, weight)),
                "route weights must be finite and nonnegative",
            );
        }
        for weight in [0.0, 0.5, 2.0] {
            assert!(validate(&weighted(|configuration| set(configuration, weight))).is_ok());
        }
    }
}
