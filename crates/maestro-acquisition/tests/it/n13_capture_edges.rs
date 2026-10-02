//! N13 prepared-capture readback and provenance guards.
use super::{
    n12_support::Fixture,
    n13_durably_enumerate_public_links_and_bounded_partitions::{
        batch, capture, item, partition, run, scope,
    },
    n13_edges::Limited,
};
use maestro_acquisition::discovery::{
    links::{LinkExtraction, LinkExtractor},
    partition::discover,
};
use maestro_kernel::{
    acquisition::{
        Captures, DispatchRequest, Enumeration, Frontier, LeaseRequest, Partitions, ReceiptError,
    },
    artifact::Digest,
};
use std::{fs, future::Future, pin::Pin, time::Duration};

#[test]
fn n13_verified_readback_refuses_substituted_dispatch_or_corrupt_body() {
    let fixture = Fixture::new();
    let handle = fixture.prepare().unwrap();
    let request = item(&fixture, "other").request;
    let other = fixture
        .db
        .enqueue(&fixture.context.writer, &request, fixture.context.now)
        .unwrap();
    let mut context = fixture.context.clone();
    context.item = fixture
        .db
        .lease(
            &context.writer,
            other.id,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: "worker",
                    now: context.now,
                    term: Duration::from_secs(30),
                },
                max_attempts: 2,
            },
        )
        .unwrap();
    assert_eq!(
        fixture.db.read_capture(&context, handle, 1000),
        Err(ReceiptError::Invalid)
    );
    let digest = fixture.envelope.artifact.as_str();
    let prefix = digest.get(..2).unwrap();
    let shard = digest.get(2..4).unwrap();
    fs::write(
        fixture
            .root
            .join("artifacts/sha256")
            .join(prefix)
            .join(shard)
            .join(digest),
        b"corrupt",
    )
    .unwrap();
    assert!(
        fixture
            .db
            .read_capture(&fixture.context, handle, 1000)
            .is_err()
    );
}

#[test]
fn n13_capture_readback_requires_original_linkage_and_positive_bound() {
    use maestro_kernel::acquisition::Receipts;
    let fixture = Fixture::new();
    let original = fixture.prepare().unwrap();
    let forged = fixture
        .db
        .retain(
            &scope(),
            &serde_json::to_vec(&fixture.envelope).unwrap(),
            &[],
        )
        .unwrap();
    assert_ne!(forged, original);
    assert_eq!(
        fixture.db.read_capture(&fixture.context, forged, 1000),
        Err(ReceiptError::Invalid)
    );
    assert_eq!(
        fixture.db.read_capture(&fixture.context, original, 0),
        Err(ReceiptError::Invalid)
    );
    assert!(
        fixture
            .db
            .read_capture(&fixture.context, original, 3)
            .unwrap()
            .1
            .is_none()
    );
    assert_eq!(
        fixture
            .db
            .read_capture(&fixture.context, original, 4)
            .unwrap()
            .1
            .unwrap(),
        b"body"
    );
}

#[test]
fn n13_prepared_parent_and_change_keys_bind_the_exact_source_context() {
    let fixture = Fixture::new();
    let capture = fixture.prepare().unwrap();
    let mut batch = batch(&fixture);
    batch.partition.kind = Enumeration::Links;
    batch.capture = Some(capture);
    batch.extractor = Some("synthetic captured extraction/1".into());
    let mut parent = batch.items.first().unwrap().keys.clone();
    parent.metadata = Some(Digest::of(
        &serde_json::to_vec(&(
            &fixture.envelope.declared_media,
            &fixture.envelope.detected_media,
        ))
        .unwrap(),
    ));
    batch.parent_keys = Some(parent);
    let child = &mut batch.items.first_mut().unwrap().keys;
    child.representation = None;
    child.validator = None;
    child.metadata = None;
    child.links = None;
    for index in 0..10 {
        let mut wrong = batch.clone();
        match index {
            0 => {
                wrong.extractor = None;
            }
            1 => {
                wrong.extractor = Some("x".repeat(513));
            }
            2 => {
                wrong.items.first_mut().unwrap().keys.representation =
                    Some(Digest::of(b"substitute"));
            }
            3 => {
                wrong.capture = Some(fixture.envelope.inputs);
            }
            4 => {
                wrong.parent_keys.as_mut().unwrap().representation =
                    Some(Digest::of(b"substitute"));
            }
            5 => {
                wrong.parent_keys.as_mut().unwrap().validator = Some(Digest::of(b"substitute"));
            }
            6 => {
                wrong.parent_keys.as_mut().unwrap().metadata = Some(Digest::of(b"substitute"));
            }
            7 => {
                wrong.parent_keys.as_mut().unwrap().permissions = Digest::of(b"substitute");
            }
            8 => {
                wrong.items.first_mut().unwrap().keys.validator = Some(Digest::of(b"substitute"));
            }
            _ => {
                wrong.items.first_mut().unwrap().keys.metadata = Some(Digest::of(b"substitute"));
            }
        }
        assert_eq!(
            fixture
                .db
                .checkpoint(&fixture.context.writer, &wrong, fixture.context.now),
            Err(ReceiptError::Invalid)
        );
    }
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
}

#[test]
fn n13_other_source_capture_cannot_back_a_checkpoint() {
    let fixture = Fixture::new();
    let mut context = fixture.context.clone();
    let lease = LeaseRequest {
        holder: "other",
        now: context.now,
        term: Duration::from_secs(30),
    };
    context.writer = fixture.db.lease_source("other", &scope(), lease).unwrap();
    let other = fixture
        .db
        .enqueue(
            &context.writer,
            &item(&fixture, "start").request,
            context.now,
        )
        .unwrap();
    context.item = fixture
        .db
        .lease(
            &context.writer,
            other.id,
            DispatchRequest {
                lease,
                max_attempts: 2,
            },
        )
        .unwrap();
    let mut envelope = fixture.envelope.clone();
    envelope.source = "other".into();
    envelope.item = other.id.to_string().parse().unwrap();
    let handle = fixture
        .db
        .prepare_capture(&context, &envelope, b"body", u64::MAX)
        .unwrap()
        .handle;
    let mut batch = batch(&fixture);
    batch.partition.kind = Enumeration::Links;
    batch.capture = Some(handle);
    batch.extractor = Some("synthetic exact source/1".into());
    assert_eq!(
        fixture
            .db
            .checkpoint(&fixture.context.writer, &batch, fixture.context.now),
        Err(ReceiptError::Invalid)
    );
}

#[test]
fn n13_unsupported_capture_media_and_status_never_enter_discovery() {
    for status in [200, 403] {
        let mut fixture = Fixture::new();
        let handle = if status == 200 {
            fixture.envelope.declared_media = Some("application/pdf".into());
            fixture
                .db
                .prepare_capture(&fixture.context, &fixture.envelope, b"body", u64::MAX)
                .unwrap()
                .handle
        } else {
            fixture.envelope.status = status;
            capture(&mut fixture, b"<html>denied</html>")
        };
        assert_eq!(
            run(discover(
                &fixture.db,
                &Limited(true),
                (&fixture.context, handle),
                &fixture.policy,
                (partition(), 0)
            )),
            Err(ReceiptError::Invalid)
        );
    }
}

#[test]
fn n13_extractor_output_must_match_its_declared_contract() {
    let mut fixture = Fixture::new();
    let handle = capture(&mut fixture, b"<html>source</html>");
    let extractor = ChangedContract(Limited(true));
    assert_eq!(
        run(discover(
            &fixture.db,
            &extractor,
            (&fixture.context, handle),
            &fixture.policy,
            (partition(), 0)
        )),
        Err(ReceiptError::Invalid)
    );
}
/// A substituted adapter cannot relabel its extraction evidence.
struct ChangedContract(Limited);
impl LinkExtractor for ChangedContract {
    fn contract(&self) -> &'static str {
        "different declared contract/1"
    }
    fn extract<'a>(
        &'a self,
        url: &'a str,
        bytes: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<LinkExtraction, ReceiptError>> + Send + 'a>> {
        self.0.extract(url, bytes)
    }
}
