//! Safe fake-backed registration checks and the explicitly ignored live card registration.

use super::support::{
    Scratch, collection, collection_scopes, grant, identity, live_kernel, required,
};
use crate::{
    artifact::{Digest, Store},
    gateway::{ModelCard, Role, card_v2::CardIdentity},
    journal::{self, Filter},
    model::{CardRecord, Error as ModelError, NewModelCard},
    scope::{Scope, ScopeSet, collection_path},
    store::Database,
};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeMap,
    error::Error as StdError,
    fs::{self, File},
    io::{self, Read as _},
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
    let card_json = fs::read(card_path).expect("read MAESTRO_CARD_JSON");

    let (database, scopes, data) = live_kernel();
    let store = Store::new(data.join("artifacts"));
    let result = RegistrationContext {
        database: &database,
        scopes: &scopes,
        collection: &collection_id,
        evidence_dir: Path::new(&evidence_dir),
        store: &store,
    }
    .register(&card_json, Path::new(&gguf_path))
    .expect("register approved model card");
    println!(
        "{} {}",
        result.record.digest.as_str(),
        result.outcome.as_str()
    );
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
    fs::remove_file(fixture.evidence_dir.join("runtime-manifest.txt")).unwrap();

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

    assert_eq!(result.outcome, RegistrationOutcome::Recorded);
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

    assert_eq!(first.outcome, RegistrationOutcome::Recorded);
    assert_eq!(second.outcome, RegistrationOutcome::AlreadyPresent);
    assert_eq!(first.record.id, second.record.id);
    assert_eq!(
        fixture
            .database
            .artifact(&first.record.digest)
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

struct RegistrationContext<'a> {
    database: &'a Database,
    scopes: &'a ScopeSet,
    collection: &'a str,
    evidence_dir: &'a Path,
    store: &'a Store,
}

impl RegistrationContext<'_> {
    fn register(
        &self,
        card_json: &[u8],
        gguf_path: &Path,
    ) -> Result<Registration, Box<dyn StdError + Send + Sync>> {
        register_card(self, card_json, gguf_path)
    }
}

struct Fixture {
    database: Database,
    scopes: ScopeSet,
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
        let weights = scratch.0.join("weights.gguf");
        fs::write(&weights, TEST_WEIGHTS).unwrap();
        let evidence_dir = scratch.0.join("evidence");
        fs::create_dir(&evidence_dir).unwrap();
        for (name, bytes) in EVIDENCE_FILES {
            fs::write(evidence_dir.join(name), bytes).unwrap();
        }
        Self {
            database,
            scopes,
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
    ) -> Result<Registration, Box<dyn StdError + Send + Sync>> {
        let store = Store::new(self.scratch.0.join("artifacts"));
        RegistrationContext {
            database: &self.database,
            scopes,
            collection,
            evidence_dir: &self.evidence_dir,
            store: &store,
        }
        .register(&self.card_json, gguf_path)
    }
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CardJson {
    schema: String,
    identity: CardIdentity,
}

struct Registration {
    record: CardRecord,
    outcome: RegistrationOutcome,
}

#[derive(Debug, PartialEq, Eq)]
enum RegistrationOutcome {
    Recorded,
    AlreadyPresent,
}

impl RegistrationOutcome {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Recorded => "recorded",
            Self::AlreadyPresent => "already present",
        }
    }
}

fn card_json(identity: &CardIdentity) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schema": "maestro-model-card/2",
        "identity": identity,
    }))
    .unwrap()
}

fn register_card(
    context: &RegistrationContext<'_>,
    card_json: &[u8],
    gguf_path: &Path,
) -> Result<Registration, Box<dyn StdError + Send + Sync>> {
    let card: CardJson = serde_json::from_slice(card_json)?;
    if card.schema != "maestro-model-card/2" {
        return Err(invalid_data("expected maestro-model-card/2").into());
    }
    let collection_scope: Scope = collection_path(context.collection).parse()?;
    if !context.scopes.covers(&collection_scope) {
        return Err(Box::new(ModelError::Unauthorized));
    }
    if context
        .database
        .collection(context.scopes, context.collection)?
        .is_none()
    {
        return Err(invalid_data("model-card collection is not registered").into());
    }
    card.identity.validate()?;
    verify_gguf(&card.identity, gguf_path)?;
    let evidence = load_evidence(context.evidence_dir, &card.identity)?;
    for artifact in evidence {
        let stored = context.database.put(&artifact.bytes, artifact.media_type)?;
        if stored != artifact.digest {
            return Err(invalid_data("stored evidence digest changed").into());
        }
    }
    let model_card = ModelCard::record_v2(context.store, &card.identity)?;
    let already_present = context
        .database
        .model_cards(context.scopes, context.collection, card.identity.role)?
        .iter()
        .any(|record| record.digest == *model_card.digest());
    let record = context.database.record_model_card(
        context.scopes,
        &NewModelCard {
            collection_id: context.collection,
            card: &model_card,
        },
    )?;
    let outcome = if already_present {
        RegistrationOutcome::AlreadyPresent
    } else {
        RegistrationOutcome::Recorded
    };
    Ok(Registration { record, outcome })
}

fn load_evidence(
    evidence_dir: &Path,
    identity: &CardIdentity,
) -> Result<Vec<EvidenceArtifact>, Box<dyn StdError + Send + Sync>> {
    let required = identity.artifact_digests();
    let mut found = BTreeMap::new();
    for entry in fs::read_dir(evidence_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() {
            continue;
        }
        let actual = digest_file(&mut File::open(&path)?)?.0;
        if let Some(named) = entry
            .file_name()
            .to_str()
            .and_then(|name| Digest::parse(name).ok())
            .filter(|digest| required.contains(digest))
            && named != actual
        {
            return Err(invalid_data("evidence filename digest differs from its file").into());
        }
        if required.contains(&actual) {
            let bytes = fs::read(&path)?;
            if Digest::of(&bytes) != actual {
                return Err(invalid_data("evidence file changed while reading").into());
            }
            found.entry(actual.clone()).or_insert(EvidenceArtifact {
                digest: actual,
                bytes,
                media_type: evidence_media_type(&path),
            });
        }
    }
    let mut artifacts = Vec::with_capacity(required.len());
    for digest in required {
        let Some(artifact) = found.remove(&digest) else {
            return Err(invalid_data("evidence directory lacks a required matching digest").into());
        };
        artifacts.push(artifact);
    }
    Ok(artifacts)
}

struct EvidenceArtifact {
    digest: Digest,
    bytes: Vec<u8>,
    media_type: &'static str,
}

fn evidence_media_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => "application/json",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

fn verify_gguf(
    identity: &CardIdentity,
    path: &Path,
) -> Result<(), Box<dyn StdError + Send + Sync>> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != identity.weights.gguf_bytes.get() {
        return Err(invalid_data("GGUF file length differs from the card").into());
    }
    let (digest, bytes) = digest_file(&mut file)?;
    if bytes != identity.weights.gguf_bytes.get() {
        return Err(invalid_data("GGUF file length changed while hashing").into());
    }
    if digest != identity.weights.gguf_digest {
        return Err(invalid_data("GGUF file digest differs from the card").into());
    }
    Ok(())
}

fn digest_file(file: &mut File) -> Result<(Digest, u64), Box<dyn StdError + Send + Sync>> {
    let (mut hasher, mut bytes, mut block) = (Sha256::new(), 0_u64, vec![0; 1 << 20]);
    loop {
        let read = file.read(&mut block)?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(u64::try_from(read).expect("read length fits u64"))
            .ok_or_else(|| invalid_data("file length overflows u64"))?;
        hasher.update(&block[..read]);
    }
    let hex = hasher
        .finalize()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            use std::fmt::Write as _;
            write!(hex, "{byte:02x}").expect("writing SHA-256 into String");
            hex
        });
    Ok((Digest::parse(&hex)?, bytes))
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
