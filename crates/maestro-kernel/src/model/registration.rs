//! Importing and registering a strict v2 model card with its pinned evidence.

use super::{
    error::Error as ModelError,
    records::{CardRecord, NewModelCard},
};
use crate::{
    artifact::{Digest, Store},
    document::Error as DocumentError,
    gateway::{CardIdentity, ModelCard},
    scope::{Scope, ScopeSet, collection_path},
    store::{Database, Error as StoreError},
};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeMap,
    error, fmt,
    fs::{self, File},
    io::{self, Read as _},
    path::Path,
};

/// Maximum bytes accepted from a model-card JSON file.
const MAX_CARD_BYTES: usize = 1 << 20;
/// Maximum bytes accepted from one evidence file.
const MAX_EVIDENCE_FILE_BYTES: usize = 16 << 20;
/// Maximum aggregate bytes read while scanning an evidence directory.
const MAX_EVIDENCE_DIRECTORY_BYTES: usize = 64 << 20;

/// Result of registering a card and its evidence.
///
/// Errors are separated so a caller can preserve the CLI's refusal-versus-failure contract.
#[derive(Debug)]
pub enum ModelCardRegistrationError {
    /// The local principal's scope does not authorize this collection.
    Unauthorized,
    /// The card, evidence or provided paths do not meet the registration contract.
    Invalid(String),
    /// The existing model-registry API refused or failed to record the card.
    Model(ModelError),
    /// A stored artifact or database record failed its integrity check.
    Integrity(String),
    /// The database or artifact store failed.
    Store(StoreError),
    /// A card, evidence or GGUF file could not be read.
    Io(io::Error),
}

impl fmt::Display for ModelCardRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unauthorized => {
                formatter.write_str("the collection is not writable in this scope")
            }
            Self::Invalid(message) => formatter.write_str(message),
            Self::Model(error) => fmt::Display::fmt(error, formatter),
            Self::Integrity(message) => write!(
                formatter,
                "model-card registration integrity check failed: {message}"
            ),
            Self::Store(error) => fmt::Display::fmt(error, formatter),
            Self::Io(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl error::Error for ModelCardRegistrationError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Unauthorized | Self::Invalid(_) | Self::Integrity(_) => None,
        }
    }
}

/// Whether registration inserted a new card or found its digest already recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelCardRegistrationOutcome {
    /// The card was recorded now.
    Recorded,
    /// The same card digest was already recorded in the collection.
    AlreadyPresent,
}

impl fmt::Display for ModelCardRegistrationOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Recorded => "recorded",
            Self::AlreadyPresent => "already present",
        })
    }
}

/// Validates all files before importing evidence and registering the v2 card.
///
/// Card files may be pretty-printed: the parsed identity is validated and `record_v2`
/// serializes its canonical representation before it becomes a registry identity.
/// Evidence is content-addressed, size bounded, and imported only after the card,
/// collection, GGUF and every required evidence digest have passed validation.
///
/// # Errors
///
/// Returns [`ModelCardRegistrationError::Unauthorized`] for an uncovered collection,
/// [`ModelCardRegistrationError::Invalid`] or `Model(Invalid)` for invalid inputs,
/// and store, integrity or file errors when an operation fails.
#[expect(
    clippy::too_many_arguments,
    reason = "the registration boundary keeps dependencies and input paths explicit"
)]
pub fn register_model_card(
    database: &Database,
    scopes: &ScopeSet,
    collection_id: &str,
    store: &Store,
    card_path: &Path,
    evidence_dir: &Path,
    gguf_path: Option<&Path>,
) -> Result<(CardRecord, ModelCardRegistrationOutcome), ModelCardRegistrationError> {
    let card_bytes = read_bounded(card_path, MAX_CARD_BYTES, "model-card file")?;
    let written: CardJson = serde_json::from_slice(&card_bytes)
        .map_err(|error| invalid(format!("invalid v2 model card: {error}")))?;
    if written.schema != "maestro-model-card/2" {
        return Err(invalid("expected maestro-model-card/2"));
    }
    let collection_scope: Scope = collection_path(collection_id)
        .parse::<Scope>()
        .map_err(|error| invalid(error.to_string()))?;
    if !scopes.covers(&collection_scope) {
        return Err(ModelCardRegistrationError::Unauthorized);
    }
    let collection = database
        .collection(scopes, collection_id)
        .map_err(document_error)?;
    if collection.is_none() {
        return Err(invalid("model-card collection is not registered"));
    }
    written
        .identity
        .validate()
        .map_err(|error| ModelCardRegistrationError::Model(error.into()))?;
    let gguf_path = gguf_path.ok_or_else(|| invalid("card pins GGUF weights; provide --gguf"))?;
    verify_gguf(&written.identity, gguf_path)?;
    let evidence = load_evidence(evidence_dir, &written.identity)?;

    let card = ModelCard::record_v2(store, &written.identity)
        .map_err(|error| ModelCardRegistrationError::Model(error.into()))?;
    if let Some(record) = database
        .model_cards(scopes, collection_id, written.identity.role)
        .map_err(ModelCardRegistrationError::Model)?
        .into_iter()
        .find(|record| record.digest == *card.digest())
    {
        return Ok((record, ModelCardRegistrationOutcome::AlreadyPresent));
    }
    for artifact in evidence {
        let stored = database
            .put(&artifact.bytes, artifact.media_type)
            .map_err(ModelCardRegistrationError::Store)?;
        if stored != artifact.digest {
            return Err(ModelCardRegistrationError::Integrity(
                "stored evidence digest changed".to_owned(),
            ));
        }
    }
    let record = database
        .record_model_card(
            scopes,
            &NewModelCard {
                collection_id,
                card: &card,
            },
        )
        .map_err(ModelCardRegistrationError::Model)?;
    Ok((record, ModelCardRegistrationOutcome::Recorded))
}

/// Strict input shape for a v2 card file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CardJson {
    /// The only candidate card schema accepted by this registration path.
    schema: String,
    /// The immutable candidate identity to validate and canonicalize.
    identity: CardIdentity,
}

/// A selected evidence file read after its size is bounded.
struct EvidenceArtifact {
    /// Content digest checked against the card's pinned identity.
    digest: Digest,
    /// Bounded bytes that will be imported after all checks pass.
    bytes: Vec<u8>,
    /// Media type inferred from the evidence filename extension.
    media_type: &'static str,
}

/// Checks every required evidence digest before the caller stores any evidence.
fn load_evidence(
    evidence_dir: &Path,
    identity: &CardIdentity,
) -> Result<Vec<EvidenceArtifact>, ModelCardRegistrationError> {
    let required = identity.artifact_digests();
    let mut found = BTreeMap::new();
    let mut total_bytes = 0_usize;
    for entry in fs::read_dir(evidence_dir).map_err(ModelCardRegistrationError::Io)? {
        let entry = entry.map_err(ModelCardRegistrationError::Io)?;
        let path = entry.path();
        if !entry
            .file_type()
            .map_err(ModelCardRegistrationError::Io)?
            .is_file()
        {
            continue;
        }
        let bytes = read_bounded(&path, MAX_EVIDENCE_FILE_BYTES, "evidence file")?;
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| invalid("evidence directory exceeds the 64 MiB limit"))?;
        if total_bytes > MAX_EVIDENCE_DIRECTORY_BYTES {
            return Err(invalid("evidence directory exceeds the 64 MiB limit"));
        }
        let actual = Digest::of(&bytes);
        if let Some(named) = entry
            .file_name()
            .to_str()
            .and_then(|name| Digest::parse(name).ok())
            .filter(|digest| required.contains(digest))
            && named != actual
        {
            return Err(invalid("evidence filename digest differs from its file"));
        }
        if required.contains(&actual) {
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
            return Err(invalid(
                "evidence directory lacks a required matching digest",
            ));
        };
        artifacts.push(artifact);
    }
    Ok(artifacts)
}

/// Reads `path` only through `limit + 1` bytes to detect an over-limit file.
fn read_bounded(
    path: &Path,
    limit: usize,
    description: &str,
) -> Result<Vec<u8>, ModelCardRegistrationError> {
    let file = File::open(path).map_err(ModelCardRegistrationError::Io)?;
    if !file
        .metadata()
        .map_err(ModelCardRegistrationError::Io)?
        .is_file()
    {
        return Err(invalid(format!("{description} is not a regular file")));
    }
    let limit = u64::try_from(limit)
        .map_err(|_| invalid("configured input limit exceeds the supported file size"))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(ModelCardRegistrationError::Io)?;
    if u64::try_from(bytes.len()).map_err(|_| {
        invalid(format!(
            "{description} length exceeds the supported file size"
        ))
    })? > limit
    {
        return Err(invalid(format!(
            "{description} exceeds the {limit} byte limit"
        )));
    }
    Ok(bytes)
}

/// The media type used for one imported evidence artifact.
fn evidence_media_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => "application/json",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

/// Verifies the GGUF's length and SHA-256 without holding the weights in memory.
fn verify_gguf(identity: &CardIdentity, path: &Path) -> Result<(), ModelCardRegistrationError> {
    let mut file = File::open(path).map_err(ModelCardRegistrationError::Io)?;
    let metadata = file.metadata().map_err(ModelCardRegistrationError::Io)?;
    if !metadata.is_file() || metadata.len() != identity.weights.gguf_bytes.get() {
        return Err(invalid("GGUF file length differs from the card"));
    }
    let (digest, bytes) = digest_file(&mut file)?;
    if bytes != identity.weights.gguf_bytes.get() {
        return Err(invalid("GGUF file length changed while hashing"));
    }
    if digest != identity.weights.gguf_digest {
        return Err(invalid("GGUF file digest differs from the card"));
    }
    Ok(())
}

/// Streams `file` through SHA-256 and returns its digest and observed byte count.
fn digest_file(file: &mut File) -> Result<(Digest, u64), ModelCardRegistrationError> {
    let (mut hasher, mut bytes, mut block) = (Sha256::new(), 0_u64, vec![0; 1 << 20]);
    loop {
        let read = file
            .read(&mut block)
            .map_err(ModelCardRegistrationError::Io)?;
        if read == 0 {
            break;
        }
        let read = u64::try_from(read).map_err(|_| {
            ModelCardRegistrationError::Integrity("read length exceeds u64".to_owned())
        })?;
        bytes = bytes.checked_add(read).ok_or_else(|| {
            ModelCardRegistrationError::Integrity("file length overflows u64".to_owned())
        })?;
        let read = usize::try_from(read).map_err(|_| {
            ModelCardRegistrationError::Integrity("read length exceeds usize".to_owned())
        })?;
        hasher.update(block.get(..read).ok_or_else(|| {
            ModelCardRegistrationError::Integrity("read length exceeds hash buffer".to_owned())
        })?);
    }
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        use fmt::Write as _;
        write!(hex, "{byte:02x}").map_err(|_| {
            ModelCardRegistrationError::Integrity("could not format SHA-256 digest".to_owned())
        })?;
    }
    let digest = Digest::parse(&hex).map_err(|error| invalid(error.to_string()))?;
    Ok((digest, bytes))
}

/// Converts document-collection reads to the registration error boundary.
fn document_error(error: DocumentError) -> ModelCardRegistrationError {
    match error {
        DocumentError::Store(error) => ModelCardRegistrationError::Store(error),
        other => invalid(other.to_string()),
    }
}

/// A validated user input that cannot be registered.
fn invalid(message: impl fmt::Display) -> ModelCardRegistrationError {
    ModelCardRegistrationError::Invalid(message.to_string())
}
