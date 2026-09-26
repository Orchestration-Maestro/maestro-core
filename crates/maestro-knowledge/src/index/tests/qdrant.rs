//! A Qdrant out of reach: the client's error is the cause of the refusal,
//! and of the publication's stop; and what a client shows of itself.

use super::super::{Error, Qdrant, QdrantError};
use std::{error::Error as _, net::TcpListener};

#[tokio::test]
async fn a_qdrant_out_of_reach_keeps_the_clients_error_as_its_cause() {
    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    };
    let error = Qdrant::new(&closed)
        .unwrap()
        .count("collection")
        .await
        .unwrap_err();
    let cause = error.source().map(ToString::to_string).unwrap();
    assert!(matches!(error, QdrantError::Client(_)), "{error}");
    assert_eq!(error.to_string(), format!("Qdrant refused: {cause}"));
    let stop = Error::Qdrant(error);
    assert!(
        stop.to_string()
            .starts_with("Qdrant failed: Qdrant refused: "),
        "{stop}"
    );
    assert!(stop.source().is_some_and(|cause| cause.source().is_some()));
}

#[test]
fn a_url_that_is_no_uri_is_refused_before_any_request() {
    let error = Qdrant::new("http://[::1").unwrap_err();
    assert!(matches!(error, QdrantError::Client(_)), "{error}");
}

#[test]
fn a_client_shows_where_it_connects_and_nothing_else() {
    let qdrant = Qdrant::new("http://127.0.0.1:6334").unwrap();
    assert_eq!(
        format!("{qdrant:?}"),
        r#"Qdrant { url: "http://127.0.0.1:6334", .. }"#
    );
}
