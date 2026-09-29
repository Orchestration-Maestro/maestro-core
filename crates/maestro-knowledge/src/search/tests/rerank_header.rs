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

#[test]
fn capped_header_accepts_exact_fit_and_truncates_one_byte_over() {
    let exact = heading_path(Some("T"), &["x".repeat(506)]).unwrap();
    assert_eq!(exact, format!("T > {}\n\n", "x".repeat(506)));
    assert_eq!(exact.len(), 512);

    let long_title = heading_path(Some(&"t".repeat(300)), &["p".repeat(207)]).unwrap();
    assert_eq!(
        long_title,
        format!("{} > {}\n\n", "t".repeat(300), "p".repeat(207))
    );

    let over_title = format!("T > {}x", "x".repeat(506));
    let over = heading_path(Some(&over_title), &[]).unwrap();
    assert_eq!(over.len(), 512);
    assert_eq!(over, format!("{}\n\n", &over_title[..510]));

    let title_only = heading_path(Some(&"x".repeat(511)), &[]).unwrap();
    assert_eq!(title_only, format!("{}\n\n", "x".repeat(510)));
    let path_only = heading_path(None, &["x".repeat(511)]).unwrap();
    assert_eq!(path_only.len(), 512);
    assert_eq!(path_only, format!("{}\n\n", "x".repeat(510)));
}

#[test]
fn capped_header_removes_a_trailing_space_at_the_truncation_boundary() {
    let title = format!("{} y", "x".repeat(509));
    let header = heading_path(Some(&title), &[]).unwrap();
    assert_eq!(header, format!("{}\n\n", "x".repeat(509)));
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
async fn headed_tokenized_document_fits_without_splitting() {
    let port = FakePort::scores(vec![1.0]);
    let card = card(Role::Reranker, 24);
    let mut input = candidate("a", 1.0, "éééé");
    input.header = Some("H\n\n".to_owned());
    let result = rerank(
        "q",
        vec![input],
        &Reranker {
            port: &port,
            card: &card,
        },
        NonZeroUsize::new(1).unwrap(),
        Duration::from_secs(1),
    )
    .await;
    assert_eq!(result.status, RouteStatus::Ok);
    assert_eq!(port.calls.lock().unwrap()[0].documents, ["H\n\néééé"]);
    assert_eq!(
        port.tokenized.lock().unwrap().as_slice(),
        ["q", "H\n\n", "éééé"]
    );
}

#[tokio::test]
async fn header_that_leaves_no_text_budget_falls_back_without_scoring() {
    for (context, tokenizes_text) in [(19, false), (20, true)] {
        let port = FakePort::scores(vec![1.0]);
        let card = card(Role::Reranker, context);
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
            RouteStatus::Unavailable("context_limit".to_owned()),
            "context={context}"
        );
        assert_eq!(result.ranked[0].candidate, input);
        assert!(port.calls.lock().unwrap().is_empty());
        assert_eq!(
            port.tokenized
                .lock()
                .unwrap()
                .iter()
                .any(|text| text == "x"),
            tokenizes_text,
            "context={context}"
        );
    }
}
