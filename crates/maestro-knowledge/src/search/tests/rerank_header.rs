//! Header formatting and token budgets affect model input, not candidate text.

use super::rerank::{FakePort, candidate, card};
use crate::search::{Reranker, rerank, rerank_header::heading_path};
use maestro_kernel::{evidence::RouteStatus, gateway::Role};
use std::{num::NonZeroUsize, time::Duration};

#[test]
fn header_normalizes_parts_without_losing_french_text() {
    let path = vec![
        "  ".to_owned(),
        " L’été\n  d'André ".to_owned(),
        "Détail".to_owned(),
    ];
    assert_eq!(
        heading_path(Some(" Page\t titre "), &path).as_deref(),
        Some("Page titre > L’été d'André > Détail\n\n")
    );
    assert_eq!(
        heading_path(None, &path).as_deref(),
        Some("L’été d'André > Détail\n\n")
    );
    assert_eq!(heading_path(Some("Page"), &[]).as_deref(), Some("Page\n\n"));
    assert_eq!(heading_path(None, &[]), None);
    assert_eq!(heading_path(Some(" \n"), &["\t".to_owned()]), None);
}

#[test]
fn capped_header_keeps_title_and_deepest_headings_at_utf8_boundaries() {
    let path = vec![
        "ancestor".repeat(80),
        "étape".repeat(100),
        "deepest".to_owned(),
    ];
    let header = heading_path(Some("Titre"), &path).unwrap();
    assert!(header.len() <= 512);
    assert!(header.starts_with("Titre > "));
    assert!(header.ends_with(" > deepest\n\n"));
    assert!(!header.contains("ancestor"));
    assert!(!header.contains(" >  > "));
    let header = heading_path(Some(&"é".repeat(800)), &["fin".to_owned()]).unwrap();
    assert!(header.len() <= 512);
    assert!(header.starts_with('é'));
    assert!(header.ends_with(" > fin\n\n"));
}

#[tokio::test]
async fn header_is_on_every_window_and_counts_against_context() {
    for (header, expected) in [
        (None, vec!["ab cd ef"]),
        (
            Some("H\n\n".to_owned()),
            vec!["H\n\nab ", "H\n\ncd ", "H\n\nef"],
        ),
    ] {
        let port = FakePort::scores(vec![0.5; expected.len()]);
        let card = card(Role::Reranker, 25);
        let mut input = candidate("a", 1.0, "ab cd ef");
        input.header = header.clone();
        let result = rerank(
            "q",
            vec![input.clone()],
            &Reranker {
                port: &port,
                card: &card,
            },
            NonZeroUsize::new(1).unwrap(),
            Duration::from_secs(1),
        )
        .await;
        assert_eq!(result.status, RouteStatus::Ok);
        assert_eq!(result.ranked[0].candidate, input);
        assert_eq!(port.calls.lock().unwrap()[0].documents, expected);
        if let Some(header) = header {
            assert_eq!(
                port.tokenized
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|text| *text == &header)
                    .count(),
                1
            );
        } else {
            assert!(port.tokenized.lock().unwrap().is_empty());
        }
    }
}

#[tokio::test]
async fn header_that_leaves_no_text_budget_falls_back_without_scoring() {
    let port = FakePort::scores(vec![1.0]);
    let card = card(Role::Reranker, 19);
    let mut input = candidate("a", 1.0, "x");
    input.header = Some("H\n\n".to_owned());
    let result = rerank(
        "q",
        vec![input.clone()],
        &Reranker {
            port: &port,
            card: &card,
        },
        NonZeroUsize::new(1).unwrap(),
        Duration::from_secs(1),
    )
    .await;
    assert_eq!(
        result.status,
        RouteStatus::Unavailable("context_limit".to_owned())
    );
    assert_eq!(result.ranked[0].candidate, input);
    assert!(port.calls.lock().unwrap().is_empty());
}
