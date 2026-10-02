//! Safe fake-backed registration checks and the explicitly ignored live card registration.

use super::support::{
    Scratch, collection, collection_scopes, grant, identity, live_kernel, required,
};
use crate::{
    artifact::{Digest, Store},
    gateway::{Role, card_v2::CardIdentity},
    journal::{self, Filter},
    model::{
        CardRecord, ModelCardRegistrationError, ModelCardRegistrationOutcome, register_model_card,
    },
    scope::ScopeSet,
    store::Database,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

const COLLECTION: &str = "registration-test";
const TEST_WEIGHTS: &[u8] = b"synthetic model weights";
const EVIDENCE_FILES: [(&str, &[u8]); 4] = [
    (
        "output-limit-observation.json",
        b"synthetic output-limit observation",
    ),
    ("runtime-manifest.txt", b"synthetic runtime manifest"),
    (
        "tokenizer-observation.json",
        b"synthetic tokenizer observation",
    ),
    ("qualification.json", b"synthetic qualification artifact"),
];

#[test]
#[ignore = "records the approved v2 card in the local kernel; run only with owner approval"]
fn register_card_live() {
    let card_path = required("MAESTRO_CARD_JSON");
    let collection_id = required("MAESTRO_CARD_COLLECTION");
    let gguf_path = required("MAESTRO_CARD_GGUF");
    let evidence_dir = required("MAESTRO_CARD_EVIDENCE");
    let (database, scopes, data) = live_kernel();
    let store = Store::new(data.join("artifacts"));
    let (record, outcome) = register_model_card(
        &database,
        &scopes,
        &collection_id,
        &store,
        Path::new(&card_path),
        Path::new(&evidence_dir),
        Some(Path::new(&gguf_path)),
    )
    .expect("register approved model card");
    println!("{} {outcome}", record.digest.as_str());
}

#[test]
fn register_card_refuses_a_gguf_digest_mismatch() {
    let fixture = Fixture::new();
    let wrong_weights = fixture.scratch.0.join("wrong.gguf");
    let wrong_bytes = b"synthetic model weightx";
    assert_eq!(wrong_bytes.len(), TEST_WEIGHTS.len());
    fs::write(&wrong_weights, wrong_bytes).unwrap();

    assert!(
        fixture
            .register(&wrong_weights, COLLECTION, &fixture.scopes)
            .is_err()
    );
    assert!(
        fixture
            .database
            .model_cards(&fixture.scopes, COLLECTION, Role::Embedder)
            .unwrap()
            .is_empty()
    );
    assert_no_evidence_import(&fixture);
}

#[test]
fn register_card_refuses_missing_evidence_before_writing() {
    let fixture = Fixture::new();
    fs::remove_file(
        fixture
            .evidence_dir
            .join(fixture.evidence_digests[1].as_str()),
    )
    .unwrap();

    let Err(error) = fixture.register(&fixture.weights, COLLECTION, &fixture.scopes) else {
        panic!("missing evidence was accepted");
    };
    assert_eq!(
        error.to_string(),
        "evidence directory lacks a required matching digest"
    );
    assert_no_evidence_import(&fixture);
}

#[test]
fn register_card_refuses_evidence_digest_mismatch_before_writing() {
    let fixture = Fixture::new();
    let wrong = fixture.evidence_dir.join("mismatched.json");
    fs::write(&wrong, b"not the qualified evidence").unwrap();
    let mislabeled = fixture
        .evidence_dir
        .join(fixture.evidence_digests[0].as_str());
    fs::rename(wrong, mislabeled).unwrap();

    let Err(error) = fixture.register(&fixture.weights, COLLECTION, &fixture.scopes) else {
        panic!("mislabeled evidence digest was accepted");
    };
    assert_eq!(
        error.to_string(),
        "evidence filename digest differs from its file"
    );
    assert_no_evidence_import(&fixture);
}

#[test]
fn register_card_imports_all_evidence_before_recording() {
    let fixture = Fixture::new();

    let result = fixture
        .register(&fixture.weights, COLLECTION, &fixture.scopes)
        .unwrap();

    assert_eq!(result.1, ModelCardRegistrationOutcome::Recorded);
    for digest in &fixture.evidence_digests {
        assert_eq!(
            fixture.database.artifact(digest).unwrap().unwrap().pins,
            1,
            "{}",
            digest.as_str()
        );
    }
    assert_eq!(
        fixture
            .database
            .model_cards(&fixture.scopes, COLLECTION, Role::Embedder)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn register_card_duplicate_is_a_noop() {
    let fixture = Fixture::new();
    let first = fixture
        .register(&fixture.weights, COLLECTION, &fixture.scopes)
        .unwrap();
    let second = fixture
        .register(&fixture.weights, COLLECTION, &fixture.scopes)
        .unwrap();

    assert_eq!(first.1, ModelCardRegistrationOutcome::Recorded);
    assert_eq!(second.1, ModelCardRegistrationOutcome::AlreadyPresent);
    assert_eq!(first.0.id, second.0.id);
    assert_eq!(
        fixture
            .database
            .artifact(&first.0.digest)
            .unwrap()
            .unwrap()
            .pins,
        1
    );
    let stream = journal::stream(COLLECTION);
    let events = fixture
        .database
        .events(
            &fixture.scopes,
            &Filter {
                stream: &stream,
                after: 0,
                r#type: Some("maestro.model.card.recorded.v1"),
            },
        )
        .unwrap();
    assert_eq!(events.len(), 1);
    for digest in &fixture.evidence_digests {
        assert_eq!(
            fixture.database.artifact(digest).unwrap().unwrap().pins,
            1,
            "{}",
            digest.as_str()
        );
    }
}

#[test]
fn register_card_accepts_pretty_json_with_a_trailing_newline_and_canonicalizes_it() {
    let fixture = Fixture::new();
    let first_json: serde_json::Value =
        serde_json::from_slice(&fs::read(&fixture.card_path).unwrap()).unwrap();
    fs::write(
        &fixture.card_path,
        format!("{}\n", serde_json::to_string_pretty(&first_json).unwrap()),
    )
    .unwrap();
    let first = fixture
        .register(&fixture.weights, COLLECTION, &fixture.scopes)
        .unwrap();

    fs::write(&fixture.card_path, &fixture.card_json).unwrap();
    let second = fixture
        .register(&fixture.weights, COLLECTION, &fixture.scopes)
        .unwrap();

    assert_eq!(first.0.digest, second.0.digest);
    assert_eq!(first.1, ModelCardRegistrationOutcome::Recorded);
    assert_eq!(second.1, ModelCardRegistrationOutcome::AlreadyPresent);
}

#[test]
fn register_card_refuses_an_oversized_card_without_writing() {
    let fixture = Fixture::new();
    fs::write(&fixture.card_path, vec![b' '; (1 << 20) + 1]).unwrap();

    assert_registration_refused_without_writes(&fixture);
}

#[test]
fn register_card_refuses_oversized_evidence_without_writing() {
    let fixture = Fixture::new();
    fs::write(
        fixture.evidence_dir.join("too-large"),
        vec![b'x'; (16 << 20) + 1],
    )
    .unwrap();

    assert_registration_refused_without_writes(&fixture);
}

#[test]
fn register_card_refuses_an_oversized_evidence_directory_without_writing() {
    let fixture = Fixture::new();
    for index in 0..5 {
        let path = fixture.evidence_dir.join(format!("padding-{index}.bin"));
        fs::File::create(path).unwrap().set_len(13 << 20).unwrap();
    }

    assert_registration_refused_without_writes(&fixture);
}

#[test]
fn register_card_requires_the_pinned_gguf() {
    let fixture = Fixture::new();
    let store = Store::new(fixture.scratch.0.join("artifacts"));
    let error = register_model_card(
        &fixture.database,
        &fixture.scopes,
        COLLECTION,
        &store,
        &fixture.card_path,
        &fixture.evidence_dir,
        None,
    )
    .unwrap_err();
    assert!(error.to_string().contains("provide --gguf"));
    assert_no_evidence_import(&fixture);
}

#[test]
fn register_card_refuses_an_unknown_collection() {
    let fixture = Fixture::new();
    let missing_scopes = grant(
        &fixture.database,
        "missing-collection-reader",
        "workspace/default/collection/missing",
    );

    assert!(
        fixture
            .register(&fixture.weights, "missing", &missing_scopes)
            .is_err()
    );
    assert!(
        fixture
            .database
            .model_cards(&missing_scopes, "missing", Role::Embedder)
            .unwrap()
            .is_empty()
    );
    assert_no_evidence_import(&fixture);
}

#[test]
fn register_card_accepts_a_card_of_exactly_the_size_limit() {
    let fixture = Fixture::new();
    let mut padded = fixture.card_json.clone();
    padded.resize(1 << 20, b' ');
    fs::write(&fixture.card_path, padded).unwrap();

    let result = fixture.register(&fixture.weights, COLLECTION, &fixture.scopes);

    assert_eq!(result.unwrap().1, ModelCardRegistrationOutcome::Recorded);
}

#[test]
fn register_card_accepts_an_evidence_directory_of_exactly_the_size_limit() {
    let fixture = Fixture::new();
    let evidence_bytes: u64 = EVIDENCE_FILES
        .iter()
        .map(|(_, bytes)| u64::try_from(bytes.len()).unwrap())
        .sum();
    let file_limit = 16 << 20;
    for (index, length) in [
        file_limit,
        file_limit,
        file_limit,
        file_limit - evidence_bytes,
    ]
    .into_iter()
    .enumerate()
    {
        let path = fixture.evidence_dir.join(format!("padding-{index}.bin"));
        fs::File::create(path).unwrap().set_len(length).unwrap();
    }

    let result = fixture.register(&fixture.weights, COLLECTION, &fixture.scopes);

    assert_eq!(result.unwrap().1, ModelCardRegistrationOutcome::Recorded);
}

#[test]
fn register_card_stores_evidence_with_the_media_type_of_its_extension() {
    let fixture = Fixture::new();
    for (name, bytes) in &EVIDENCE_FILES[..3] {
        let digest_named = fixture.evidence_dir.join(Digest::of(bytes).as_str());
        fs::rename(digest_named, fixture.evidence_dir.join(name)).unwrap();
    }

    fixture
        .register(&fixture.weights, COLLECTION, &fixture.scopes)
        .unwrap();

    let expected = [
        "application/json",
        "text/plain",
        "application/json",
        "application/octet-stream",
    ];
    for ((name, bytes), media) in EVIDENCE_FILES.iter().zip(expected) {
        let stored = fixture.database.artifact(&Digest::of(bytes)).unwrap();
        assert_eq!(stored.unwrap().media, media, "{name}");
    }
}

#[test]
fn register_card_refuses_a_gguf_of_another_length_before_hashing_it() {
    let fixture = Fixture::new();
    let longer = fixture.scratch.0.join("longer.gguf");
    fs::write(&longer, [TEST_WEIGHTS, b"!"].concat()).unwrap();

    let Err(error) = fixture.register(&longer, COLLECTION, &fixture.scopes) else {
        panic!("a GGUF of another length was accepted");
    };

    assert_eq!(error.to_string(), "GGUF file length differs from the card");
    assert_no_evidence_import(&fixture);
}

struct Fixture {
    database: Database,
    scopes: ScopeSet,
    card_path: PathBuf,
    card_json: Vec<u8>,
    weights: PathBuf,
    evidence_dir: PathBuf,
    evidence_digests: Vec<Digest>,
    /// Last, as fields drop in order: Windows refuses to remove a database
    /// still open.
    scratch: Scratch,
}

impl Fixture {
    fn new() -> Self {
        let scratch = Scratch::new();
        let database = scratch.open();
        collection(&database, COLLECTION);
        let scopes = collection_scopes(&database, COLLECTION);
        let mut identity = identity(&database);
        assert_eq!(identity.weights.gguf_digest, Digest::of(TEST_WEIGHTS));
        identity.provenance.artifacts = [
            (
                "output-limit-observation".to_owned(),
                Digest::of(EVIDENCE_FILES[0].1),
            ),
            (
                "runtime-manifest".to_owned(),
                Digest::of(EVIDENCE_FILES[1].1),
            ),
            (
                "tokenizer-observation".to_owned(),
                Digest::of(EVIDENCE_FILES[2].1),
            ),
        ]
        .into();
        identity.formats.qualification_digest = Digest::of(EVIDENCE_FILES[3].1);
        let evidence_digests = identity.artifact_digests().into_iter().collect();
        let card_json = card_json(&identity);
        let card_path = scratch.0.join("card.json");
        fs::write(&card_path, &card_json).unwrap();
        let weights = scratch.0.join("weights.gguf");
        fs::write(&weights, TEST_WEIGHTS).unwrap();
        let evidence_dir = scratch.0.join("evidence");
        fs::create_dir(&evidence_dir).unwrap();
        for (_, bytes) in EVIDENCE_FILES {
            fs::write(evidence_dir.join(Digest::of(bytes).as_str()), bytes).unwrap();
        }
        Self {
            database,
            scopes,
            card_path,
            card_json,
            weights,
            evidence_dir,
            evidence_digests,
            scratch,
        }
    }

    fn register(
        &self,
        gguf_path: &Path,
        collection: &str,
        scopes: &ScopeSet,
    ) -> Result<(CardRecord, ModelCardRegistrationOutcome), ModelCardRegistrationError> {
        let store = Store::new(self.scratch.0.join("artifacts"));
        register_model_card(
            &self.database,
            scopes,
            collection,
            &store,
            &self.card_path,
            &self.evidence_dir,
            Some(gguf_path),
        )
    }
}

fn assert_registration_refused_without_writes(fixture: &Fixture) {
    assert!(
        fixture
            .register(&fixture.weights, COLLECTION, &fixture.scopes)
            .is_err()
    );
    assert_no_evidence_import(fixture);
}

fn assert_no_evidence_import(fixture: &Fixture) {
    assert!(
        fixture
            .database
            .model_cards(&fixture.scopes, COLLECTION, Role::Embedder)
            .unwrap()
            .is_empty()
    );
    for digest in &fixture.evidence_digests {
        assert!(
            fixture.database.artifact(digest).unwrap().is_none(),
            "{} was written before evidence preflight completed",
            digest.as_str()
        );
    }
}

fn card_json(identity: &CardIdentity) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schema": "maestro-model-card/2",
        "identity": identity,
    }))
    .unwrap()
}
