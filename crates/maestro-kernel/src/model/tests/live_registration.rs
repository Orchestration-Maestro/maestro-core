//! Safe fake-backed registration checks and the explicitly ignored live card registration.

use super::support::{Scratch, collection, collection_scopes, grant, identity};
use crate::{
    artifact::{Digest, Store},
    gateway::{ModelCard, Role, card_v2::CardIdentity},
    journal::{self, Filter},
    model::{CardRecord, Error as ModelError, NewModelCard},
    paths::{self, Environment},
    scope::{Config, LOCAL, Scope, ScopeSet, collection_path},
    store::Database,
};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use std::{
    env,
    error::Error as StdError,
    fs::{self, File},
    io::{self, Read as _},
    path::{Path, PathBuf},
};

const COLLECTION: &str = "registration-test";
const TEST_WEIGHTS: &[u8] = b"synthetic model weights";

#[test]
#[ignore = "records the approved v2 card in the local kernel; run only with owner approval"]
fn register_card_live() {
    let card_path = required("MAESTRO_CARD_JSON");
    let collection_id = required("MAESTRO_CARD_COLLECTION");
    let gguf_path = required("MAESTRO_CARD_GGUF");
    let card_json = fs::read(card_path).expect("read MAESTRO_CARD_JSON");

    let environment = Environment::current();
    let data = paths::data_dir(&environment).expect("resolve kernel data home");
    let config =
        Config::load(&paths::config_dir(&environment).expect("resolve kernel config home"))
            .expect("read kernel access config");
    let database = Database::open_in(&data).expect("open default kernel");
    database
        .apply_config(&config)
        .expect("apply kernel access config");
    let scopes = database.visible(LOCAL).expect("read local kernel scopes");
    let store = Store::new(data.join("artifacts"));
    let result = RegistrationContext {
        database: &database,
        scopes: &scopes,
        collection: &collection_id,
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
}

struct RegistrationContext<'a> {
    database: &'a Database,
    scopes: &'a ScopeSet,
    collection: &'a str,
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
    scratch: Scratch,
    database: Database,
    scopes: ScopeSet,
    card_json: Vec<u8>,
    weights: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let scratch = Scratch::new();
        let database = scratch.open();
        collection(&database, COLLECTION);
        let scopes = collection_scopes(&database, COLLECTION);
        let identity = identity(&database);
        assert_eq!(identity.weights.gguf_digest, Digest::of(TEST_WEIGHTS));
        let card_json = card_json(&identity);
        let weights = scratch.0.join("weights.gguf");
        fs::write(&weights, TEST_WEIGHTS).unwrap();
        Self {
            scratch,
            database,
            scopes,
            card_json,
            weights,
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
            store: &store,
        }
        .register(&self.card_json, gguf_path)
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

fn required(variable: &str) -> String {
    env::var(variable).unwrap_or_else(|_| panic!("set {variable}"))
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
    verify_gguf(&card.identity, gguf_path)?;
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

fn verify_gguf(
    identity: &CardIdentity,
    path: &Path,
) -> Result<(), Box<dyn StdError + Send + Sync>> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != identity.weights.gguf_bytes.get() {
        return Err(invalid_data("GGUF file length differs from the card").into());
    }
    let (mut hasher, mut bytes, mut block) = (Sha256::new(), 0_u64, vec![0; 1 << 20]);
    loop {
        let read = file.read(&mut block)?;
        if read == 0 {
            break;
        }
        bytes = bytes
            .checked_add(u64::try_from(read).expect("read length fits u64"))
            .ok_or_else(|| invalid_data("GGUF file length overflows u64"))?;
        hasher.update(&block[..read]);
    }
    if bytes != identity.weights.gguf_bytes.get() {
        return Err(invalid_data("GGUF file length changed while hashing").into());
    }
    let hex = hasher
        .finalize()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            use std::fmt::Write as _;
            write!(hex, "{byte:02x}").expect("writing SHA-256 into String");
            hex
        });
    if Digest::parse(&hex)? != identity.weights.gguf_digest {
        return Err(invalid_data("GGUF file digest differs from the card").into());
    }
    Ok(())
}

fn invalid_data(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
