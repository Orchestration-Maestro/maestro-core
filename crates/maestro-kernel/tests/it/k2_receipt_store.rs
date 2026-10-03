//! Receipt boundary, error rendering and durable work-order regressions.
use super::{
    n04_frontier_support::{Scratch, item, now, request},
    n06_store_scoped_receipts_and_content_free_progress_events::{grant, receipt, scope},
    n37_capture_support::Fixture,
};
use maestro_kernel::{
    acquisition::{
        Captures, Error, Frontier, Progress, ReceiptError, Receipts, SafeHeader, UnfinalizedPage,
    },
    job,
    store::{self, Database},
};
use std::{error::Error as _, time::Duration};

#[test]
fn k2_headers_validate_each_policy_boundary() {
    let fixture = Fixture::new();
    for (name, value, allowed) in [
        ("content-type", "text/html", true),
        ("content-encoding", "gzip, deflate", true),
        ("content-length", "4", true),
        ("authorization", "synthetic", false),
        ("content-type", "text/html; secret=synthetic", false),
        ("content-length", "004", false),
        ("content-encoding", "unknown", false),
    ] {
        let mut envelope = fixture.envelope.clone();
        envelope.headers.insert(
            name.into(),
            SafeHeader::Value {
                value: value.into(),
            },
        );
        assert_eq!(
            fixture
                .db
                .check_capture(&fixture.context, &envelope, b"body")
                .is_ok(),
            allowed,
            "{name}: {value}"
        );
    }
}

#[test]
fn k2_receipt_unfinalized_and_progress_boundaries() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let run = receipt(&db);
    db.begin(&scope("docs"), &run).unwrap();
    let page = UnfinalizedPage {
        scope: &scope("docs"),
        after: None,
        now: now() + Duration::from_secs(60),
        limit: 1000,
    };
    assert_eq!(db.unfinalized("reader", &page), Ok(vec![run.clone()]));
    assert!(db.unfinalized("denied", &page).unwrap().is_empty());
    assert_eq!(
        db.unfinalized("reader", &UnfinalizedPage { limit: 0, ..page }),
        Err(ReceiptError::Invalid)
    );
    assert_eq!(
        db.unfinalized(
            "reader",
            &UnfinalizedPage {
                limit: 1001,
                ..page
            }
        ),
        Err(ReceiptError::Invalid)
    );
    assert_eq!(
        Receipts::page(&db, "reader", run.run, None, 1000),
        Ok(vec![Progress {
            receipt: run.attempt,
            status: run.status,
            reason: run.reason
        }])
    );
    assert_eq!(
        Receipts::page(&db, "reader", run.run, None, 1001),
        Err(ReceiptError::Invalid)
    );
    assert_eq!(
        format!("{:?}", db.read("reader", run.inputs).unwrap().unwrap()),
        format!(
            "ProtectedArtifact {{ bytes: {} }}",
            db.read("reader", run.inputs)
                .unwrap()
                .unwrap()
                .bytes()
                .len()
        )
    );
}

#[test]
fn k2_frontier_identity_limit_and_pending_work_order() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let writer = db
        .lease_source("docs", &scope("docs"), request(now()))
        .unwrap();
    let mut request = item();
    request.fetch_identity = "x".repeat(8192);
    let admitted = db.enqueue(&writer, &request, now());
    assert!(admitted.is_ok(), "8192-byte identity must be admitted");
    let row = admitted.unwrap();
    request.fetch_identity.push('x');
    assert!(matches!(
        db.enqueue(&writer, &request, now()),
        Err(Error::Invalid)
    ));
    let page = db
        .work_page(&db.visible("reader").unwrap(), "docs", None, 1000)
        .unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].item, row);
    assert!(!page[0].cursor.verified);
}

#[test]
fn k2_errors_pin_messages_and_causes() {
    for (error, message) in [
        (ReceiptError::Invalid, "invalid acquisition receipt"),
        (
            ReceiptError::Conflict,
            "acquisition receipt conflicts with its attempt",
        ),
        (ReceiptError::Storage, "acquisition receipt storage failed"),
    ] {
        assert_eq!(error.to_string(), message);
    }
    assert_eq!(
        Error::Invalid.to_string(),
        "invalid frontier identity or bound"
    );
    let cause = job::Error::Time;
    let message = cause.to_string();
    let error = Error::Job(cause);
    assert_eq!(error.to_string(), message);
    assert_eq!(error.source().map(ToString::to_string), Some(message));
    let cause = store::Error::Sqlite(rusqlite::Error::InvalidQuery);
    let message = cause.to_string();
    let error = Error::Store(cause);
    assert_eq!(error.source().map(ToString::to_string), Some(message));
}
