//! Model cards: what was evaluated of a model filling a role (D8), kept as
//! strict JSON artifacts whose digest is the card's identity.
//!
//! A legacy card is written as `maestro-model-card/1`:
//!
//! ```json
//! {
//!   "schema": "maestro-model-card/1",
//!   "role": "embedder",
//!   "router_entry": "embed",
//!   "file_digest": "<SHA-256 of the model file, recorded when the card is made>",
//!   "template_digest": null,
//!   "server_build": "b6500-3f2c9a1b",
//!   "dimensions": 1024,
//!   "limits": {"context_tokens": 8192, "output_tokens": null},
//!   "suite_results": [{"suite": "synthetic-retrieval", "report": "<SHA-256>"}]
//! }
//! ```
//!
//! An unknown or repeated key is refused, and so is a digest that is not 64
//! lowercase hexadecimal characters. `server_build` is llama.cpp's
//! `build_info` and `template_digest` the SHA-256 of the chat template, both
//! as the model's server reports them through `/props`; an embedder alone
//! records `dimensions`. `output_tokens` is null for a model that generates
//! nothing, and each suite result names the digest of its report. New
//! candidates use v2 immutable identities; evaluations and selections are
//! separate records and never change a card digest.

pub use super::card_types::{CardError, CardFields, Limits, Role, RouterEntry, SuiteResult};
use super::card_v2::{CARD_SCHEMA_V2, CardIdentity, CardV2Json};
use crate::artifact::{Digest, Store};
use serde::{Deserialize, Serialize};
use std::{fmt, num::NonZeroUsize};

/// Legacy card schema retained for exact backwards-compatible reads.
const CARD_SCHEMA: &str = "maestro-model-card/1";

/// A model card: what it records, identified by the digest of its JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelCard {
    /// The digest of the card's JSON, its identity.
    digest: Digest,
    /// Common fields kept for existing model ports.
    fields: CardFields,
    /// Full immutable metadata for v2; v1 is never inferred or promoted.
    identity: Option<CardIdentity>,
}

impl ModelCard {
    /// Records `fields` in `store` as a new card and returns it: its JSON is
    /// checked as [`ModelCard::load`] checks it, then stored as an artifact.
    ///
    /// # Errors
    ///
    /// [`CardError::Invalid`] when the fields break a rule of the card, such
    /// as dimensions on a card that is not an embedder's, and
    /// [`CardError::Store`] when the store cannot keep the card.
    pub fn record(store: &Store, fields: &CardFields) -> Result<Self, CardError> {
        let json = serde_json::to_vec(&CardJson::of(fields)).map_err(invalid)?;
        let card = Self::from_json(&json)?;
        store.put(&json).map_err(CardError::Store)?;
        Ok(card)
    }

    /// Records the full immutable model/runtime identity as v2.
    ///
    /// # Errors
    ///
    /// [`CardError::Invalid`] when identity fields conflict or contain invalid data, and
    /// [`CardError::Store`] when the artifact cannot be stored.
    pub fn record_v2(store: &Store, identity: &CardIdentity) -> Result<Self, CardError> {
        identity.validate()?;
        let json = serde_json::to_vec(&CardV2Json {
            schema: CARD_SCHEMA_V2.to_owned(),
            identity: identity.clone(),
        })
        .map_err(invalid)?;
        let card = Self::from_json(&json)?;
        store.put(&json).map_err(CardError::Store)?;
        Ok(card)
    }

    /// The card stored under `digest`.
    ///
    /// # Errors
    ///
    /// [`CardError::Store`] when the store holds no intact artifact under
    /// `digest`, and [`CardError::Invalid`] when the artifact is not a valid
    /// card.
    pub fn load(store: &Store, digest: &Digest) -> Result<Self, CardError> {
        let json = store.get(digest).map_err(CardError::Store)?;
        Self::from_json(&json)
    }

    /// The card's identity: the digest of its JSON.
    #[must_use]
    pub fn digest(&self) -> &Digest {
        &self.digest
    }

    /// What the card records.
    #[must_use]
    pub fn fields(&self) -> &CardFields {
        &self.fields
    }

    /// Full immutable identity for v2 cards; v1 has no fabricated identity.
    #[must_use]
    pub fn identity(&self) -> Option<&CardIdentity> {
        self.identity.as_ref()
    }

    /// The deterministic v2 bytes for registry persistence.
    pub(crate) fn card_json(&self) -> Result<Vec<u8>, CardError> {
        let identity = self
            .identity
            .as_ref()
            .ok_or_else(|| invalid("v1 cards cannot be newly registered"))?;
        serde_json::to_vec(&CardV2Json {
            schema: CARD_SCHEMA_V2.to_owned(),
            identity: identity.clone(),
        })
        .map_err(invalid)
    }

    /// The card whose JSON is `json`.
    pub(crate) fn from_json_bytes(json: &[u8]) -> Result<Self, CardError> {
        Self::from_json(json)
    }

    /// Parses a strict v1 or v2 card while preserving the exact input digest.
    fn from_json(json: &[u8]) -> Result<Self, CardError> {
        let legacy_error = match serde_json::from_slice::<CardJson>(json) {
            Ok(written) => match written.into_fields() {
                Ok(fields) => {
                    return Ok(Self {
                        digest: Digest::of(json),
                        fields,
                        identity: None,
                    });
                }
                Err(error) => error.to_string(),
            },
            Err(error) => error.to_string(),
        };
        match serde_json::from_slice::<CardV2Json>(json) {
            Ok(written) if written.schema == CARD_SCHEMA_V2 => {
                let canonical = serde_json::to_vec(&written).map_err(invalid)?;
                if canonical != json {
                    return Err(invalid("v2 card JSON is not canonical"));
                }
                written.identity.validate()?;
                Ok(Self {
                    digest: Digest::of(json),
                    fields: written.identity.legacy_fields(),
                    identity: Some(written.identity),
                })
            }
            Ok(written) => Err(invalid(format_args!(
                "schema {:?} is not {CARD_SCHEMA_V2}",
                written.schema
            ))),
            Err(modern_error) => Err(invalid(format_args!(
                "v1: {legacy_error}; v2: {modern_error}"
            ))),
        }
    }
}

/// A [`CardError::Invalid`] saying why.
fn invalid(reason: impl fmt::Display) -> CardError {
    CardError::Invalid(reason.to_string())
}

/// The digest `text` names, for the card's field `field`.
fn parse_digest(field: &str, text: &str) -> Result<Digest, CardError> {
    Digest::parse(text).map_err(|refused| invalid(format_args!("{field}: {refused}")))
}

/// A legacy v1 card as its JSON is written.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CardJson {
    /// Must equal [`CARD_SCHEMA`].
    schema: String,
    /// See [`CardFields::role`].
    role: Role,
    /// See [`CardFields::router_entry`].
    router_entry: String,
    /// See [`CardFields::file_digest`].
    file_digest: String,
    /// See [`CardFields::template_digest`].
    template_digest: Option<String>,
    /// See [`CardFields::server_build`].
    server_build: String,
    /// See [`CardFields::dimensions`].
    dimensions: Option<NonZeroUsize>,
    /// See [`CardFields::limits`].
    limits: Limits,
    /// See [`CardFields::suite_results`].
    suite_results: Vec<SuiteJson>,
}

impl CardJson {
    /// The JSON of `fields`.
    fn of(fields: &CardFields) -> Self {
        Self {
            schema: CARD_SCHEMA.to_owned(),
            role: fields.role,
            router_entry: fields.router_entry.as_str().to_owned(),
            file_digest: fields.file_digest.as_str().to_owned(),
            template_digest: fields
                .template_digest
                .as_ref()
                .map(|digest| digest.as_str().to_owned()),
            server_build: fields.server_build.clone(),
            dimensions: fields.dimensions,
            limits: fields.limits,
            suite_results: fields
                .suite_results
                .iter()
                .map(|result| SuiteJson {
                    suite: result.suite.clone(),
                    report: result.report.as_str().to_owned(),
                })
                .collect(),
        }
    }

    /// What the JSON records, once each of the card's rules holds.
    fn into_fields(self) -> Result<CardFields, CardError> {
        if self.schema != CARD_SCHEMA {
            return Err(invalid(format_args!(
                "schema {:?} is not {CARD_SCHEMA}",
                self.schema
            )));
        }
        match (self.role, self.dimensions) {
            (Role::Embedder, None) => return Err(invalid("an embedder's card records dimensions")),
            (Role::Reranker | Role::Answerer, Some(_)) => {
                return Err(invalid(format_args!(
                    "a {}'s card records no dimensions",
                    self.role
                )));
            }
            _ => {}
        }
        let suite_results = self
            .suite_results
            .into_iter()
            .map(|result| {
                Ok(SuiteResult {
                    report: parse_digest("report", &result.report)?,
                    suite: result.suite,
                })
            })
            .collect::<Result<_, CardError>>()?;
        Ok(CardFields {
            role: self.role,
            router_entry: RouterEntry::parse(&self.router_entry)?,
            file_digest: parse_digest("file_digest", &self.file_digest)?,
            template_digest: self
                .template_digest
                .map(|text| parse_digest("template_digest", &text))
                .transpose()?,
            server_build: self.server_build,
            dimensions: self.dimensions,
            limits: self.limits,
            suite_results,
        })
    }
}

/// A suite result as its JSON is written.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuiteJson {
    /// See [`SuiteResult::suite`].
    suite: String,
    /// See [`SuiteResult::report`].
    report: String,
}
