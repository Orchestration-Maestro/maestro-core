use super::super::{Reranker, rerank};
use super::rerank::{FakePort, candidate, card};
use maestro_kernel::gateway::Role;
use std::{num::NonZeroUsize, time::Duration};

#[tokio::test]
async fn an_over_budget_whitespace_window_uses_the_last_fitting_boundary() {
    let port = FakePort::scores(vec![0.8, 0.1]).with_token_overrides(&[("ab ", 5)]);
    let card = card(Role::Reranker, 21);
    let result = rerank(
        "q",
        vec![candidate("doc", 1.0, "ab cdef")],
        &Reranker {
            port: &port,
            card: &card,
        },
        NonZeroUsize::new(1).unwrap(),
        Duration::from_secs(1),
    )
    .await;

    let calls = port.calls.lock().unwrap();
    assert_eq!(calls[0].documents, ["ab c", "def"]);
    assert_eq!(result.ranked[0].score, Some(0.8));
}

#[tokio::test]
async fn a_fallback_window_below_budget_is_still_accepted() {
    let port = FakePort::scores(vec![0.8, 0.1]).with_token_overrides(&[("ab ", 5), ("ab c", 3)]);
    let card = card(Role::Reranker, 21);
    let result = rerank(
        "q",
        vec![candidate("doc", 1.0, "ab cdef")],
        &Reranker {
            port: &port,
            card: &card,
        },
        NonZeroUsize::new(1).unwrap(),
        Duration::from_secs(1),
    )
    .await;

    let calls = port.calls.lock().unwrap();
    assert_eq!(calls[0].documents, ["ab c", "def"]);
    assert_eq!(result.ranked[0].score, Some(0.8));
}

#[tokio::test]
async fn a_whitespace_window_at_the_full_budget_keeps_its_boundary() {
    let port = FakePort::scores(vec![0.8, 0.1]).with_token_overrides(&[
        ("ab cd !e", 5),
        ("ab cd", 4),
        ("ab cd !", 4),
        ("ab cd ", 4),
    ]);
    let card = card(Role::Reranker, 21);
    let result = rerank(
        "q",
        vec![candidate("doc", 1.0, "ab cd !e")],
        &Reranker {
            port: &port,
            card: &card,
        },
        NonZeroUsize::new(1).unwrap(),
        Duration::from_secs(1),
    )
    .await;

    let calls = port.calls.lock().unwrap();
    assert_eq!(calls[0].documents, ["ab cd ", "!e"]);
    assert_eq!(result.ranked[0].score, Some(0.8));
}
