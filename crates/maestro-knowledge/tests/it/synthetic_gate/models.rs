//! Synthetic-only tokenizer canaries plus deterministic fake inference.

use maestro_kernel::{
    artifact::{Digest, Store},
    gateway::{
        CardError, CardFields, Error, FakeModels, Limits, Message, ModelCard, ModelPort, Role,
        Room, RouterEntry,
    },
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    iter,
    num::{NonZeroU32, NonZeroUsize},
    sync::{Arc, Mutex},
};

const FAKE_MODEL: &str = "FakeModels synthetic protocol simulation/1";
const PARITY: &str = include_str!("../../../src/prepare/native-parity.json");
const NATIVE_CONTRACT: &str =
    include_str!("../../../../maestro-canonicalization/tokenizer-contract.json");

#[derive(Debug, Clone)]
pub(super) struct SyntheticModels {
    goldens: Arc<HashMap<String, Vec<u32>>>,
    first_canary: String,
    embedded: Arc<Mutex<Vec<Vec<String>>>>,
}

impl SyntheticModels {
    pub(super) fn new() -> Result<Self, String> {
        let file: Parity = serde_json::from_str(PARITY).map_err(|error| error.to_string())?;
        let native: serde_json::Value =
            serde_json::from_str(NATIVE_CONTRACT).map_err(|error| error.to_string())?;
        let contract = native
            .get("contract_id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "native tokenizer contract has no ID".to_owned())?;
        if file.profile != contract {
            return Err("native tokenizer parity profile differs from its contract".to_owned());
        }
        let mut first_canary = None;
        let mut goldens = HashMap::new();
        for fixture in file.fixtures {
            if fixture.name.is_empty() {
                return Err("native parity fixture has an empty name".to_owned());
            }
            let input = expand(fixture.input).into_iter().collect::<String>();
            if fixture.canary && first_canary.is_none() {
                first_canary = Some(input.clone());
            }
            goldens.insert(input, expand(fixture.ids));
        }
        let first_canary =
            first_canary.ok_or_else(|| "native parity file has no canary".to_owned())?;
        Ok(Self {
            goldens: Arc::new(goldens),
            first_canary,
            embedded: Arc::default(),
        })
    }

    pub(super) fn changed_canary(&self) -> Self {
        let mut goldens = (*self.goldens).clone();
        goldens.insert(self.first_canary.clone(), vec![0, 99, 2]);
        Self {
            goldens: Arc::new(goldens),
            first_canary: self.first_canary.clone(),
            embedded: Arc::clone(&self.embedded),
        }
    }

    pub(super) fn card(store: &Store) -> Result<ModelCard, CardError> {
        let fields = CardFields {
            role: Role::Embedder,
            router_entry: RouterEntry::parse("synthetic")?,
            file_digest: Digest::of(FAKE_MODEL.as_bytes()),
            template_digest: None,
            server_build: "fake-models/1".to_owned(),
            dimensions: NonZeroUsize::new(32),
            limits: Limits {
                context_tokens: NonZeroU32::new(8192).ok_or_else(|| {
                    CardError::Invalid("context tokens must be nonzero".to_owned())
                })?,
                output_tokens: None,
            },
            suite_results: Vec::new(),
        };
        ModelCard::record(store, &fields)
    }

    pub(super) fn embedded(&self) -> Result<Vec<Vec<String>>, Error> {
        self.embedded
            .lock()
            .map(|calls| calls.clone())
            .map_err(|error| Error::InvalidAnswer {
                reason: format!("synthetic inference recorder is poisoned: {error}"),
            })
    }

    pub(super) fn identity() -> &'static str {
        "fake-models/1 (synthetic protocol simulation)"
    }

    pub(super) fn qualification() -> String {
        format!(
            "synthetic-protocol-simulation/1:native-parity:{}",
            Digest::of(PARITY.as_bytes()).as_str()
        )
    }
}

impl ModelPort for SyntheticModels {
    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        self.embedded
            .lock()
            .map_err(|error| Error::InvalidAnswer {
                reason: format!("synthetic inference recorder is poisoned: {error}"),
            })?
            .push(inputs.to_vec());
        FakeModels.embed(card, room, inputs).await
    }

    async fn rerank(
        &self,
        card: &ModelCard,
        room: Room,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<f64>, Error> {
        FakeModels.rerank(card, room, query, documents).await
    }

    async fn tokenize(&self, card: &ModelCard, room: Room, text: &str) -> Result<Vec<u32>, Error> {
        match self.goldens.get(text) {
            Some(ids) => Ok(ids.clone()),
            None => FakeModels.tokenize(card, room, text).await,
        }
    }

    async fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        messages: &[Message],
    ) -> Result<String, Error> {
        FakeModels.chat(card, room, messages).await
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parity {
    profile: String,
    fixtures: Vec<Fixture>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    name: String,
    #[serde(default)]
    canary: bool,
    input: Vec<Run<String>>,
    ids: Vec<Run<u32>>,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum Run<T> {
    Once(T),
    Repeated { repeat: T, times: usize },
}

fn expand<T: Clone>(runs: Vec<Run<T>>) -> Vec<T> {
    let mut expanded = Vec::new();
    for run in runs {
        match run {
            Run::Once(value) => expanded.push(value),
            Run::Repeated { repeat, times } => expanded.extend(iter::repeat_n(repeat, times)),
        }
    }
    expanded
}
