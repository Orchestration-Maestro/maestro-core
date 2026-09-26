//! What the fake keeps, and the refusals and hollow answers a test asked for.

use qdrant_client::qdrant::{CollectionParams, PointId, RetrievedPoint, point_id::PointIdOptions};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
};
use tonic::{Code, Status};

/// A collection the fake keeps.
#[derive(Debug, Default)]
pub(super) struct Collection {
    /// Its parameters, as its creation gave them.
    pub(super) params: CollectionParams,
    /// Its points, by ID.
    pub(super) points: BTreeMap<String, RetrievedPoint>,
}

/// What the fake keeps.
#[derive(Debug, Default)]
pub(super) struct State {
    /// The collections, by name.
    pub(super) collections: BTreeMap<String, Collection>,
    /// The aliases, each with its collection.
    pub(super) aliases: BTreeMap<String, String>,
    /// The calls to refuse next, each with the code to refuse it with.
    refusals: Vec<(&'static str, Code)>,
    /// The calls to answer next without their result.
    hollows: Vec<&'static str>,
}

impl State {
    /// The collection `name`, which must exist.
    pub(super) fn collection(&mut self, name: &str) -> Result<&mut Collection, Status> {
        self.collections.get_mut(name).ok_or_else(|| {
            Status::not_found(format!("Not found: Collection `{name}` doesn't exist!"))
        })
    }
}

/// The fake's state, shared by its services.
#[derive(Debug, Clone, Default)]
pub(super) struct Fake(Arc<Mutex<State>>);

impl Fake {
    /// What the fake keeps, for one call.
    pub(super) fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap()
    }

    /// Makes the next call `call` refuse with `code`.
    pub(super) fn refuse_next(&self, call: &'static str, code: Code) {
        self.state().refusals.push((call, code));
    }

    /// Makes the next call `call` answer without its result.
    pub(super) fn hollow_next(&self, call: &'static str) {
        self.state().hollows.push(call);
    }

    /// The call `call`, refused when a test asked for it, once.
    pub(super) fn admit(&self, call: &str) -> Result<(), Status> {
        let mut state = self.state();
        let Some(at) = state
            .refusals
            .iter()
            .position(|(refused, _)| *refused == call)
        else {
            return Ok(());
        };
        let (_, code) = state.refusals.remove(at);
        Err(Status::new(code, "the fake was told to refuse"))
    }

    /// Whether the call `call` answers without its result, as a test asked
    /// for, once.
    pub(super) fn hollow(&self, call: &str) -> bool {
        let mut state = self.state();
        let at = state.hollows.iter().position(|hollow| *hollow == call);
        at.map(|at| state.hollows.remove(at)).is_some()
    }
}

/// The key the fake keeps the point `id` under: a UUID in lower case, or a
/// number's digits.
pub(super) fn key(id: &PointId) -> String {
    match &id.point_id_options {
        Some(PointIdOptions::Uuid(uuid)) => uuid.to_lowercase(),
        Some(PointIdOptions::Num(number)) => number.to_string(),
        None => String::new(),
    }
}
