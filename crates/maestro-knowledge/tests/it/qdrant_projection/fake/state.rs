//! What the fake keeps, and the refusals and hollow answers a test asked for.

use qdrant_client::qdrant::{
    CollectionParams, Filter, HnswConfigDiff, PointId, RetrievedPoint, point_id::PointIdOptions,
};
use std::{
    collections::BTreeMap,
    mem,
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::sync::Notify;
use tonic::{Code, Status};

/// A collection the fake keeps.
#[derive(Debug, Default)]
pub(super) struct Collection {
    /// Its parameters, as its creation gave them.
    pub(super) params: CollectionParams,
    /// Its HNSW configuration, as its creation gave it.
    pub(super) hnsw_config: Option<HnswConfigDiff>,
    /// Its points, by ID.
    pub(super) points: BTreeMap<String, RetrievedPoint>,
    /// Its payload indexes, by field and Qdrant field type.
    pub(super) indexes: BTreeMap<String, i32>,
}

/// What the fake keeps.
#[derive(Debug, Default)]
pub(super) struct State {
    /// The collections, by name.
    pub(super) collections: BTreeMap<String, Collection>,
    /// The aliases, each with its collection.
    pub(super) aliases: BTreeMap<String, String>,
    /// Filters passed to payload scrolls, in call order.
    pub(super) scroll_filters: Vec<Filter>,
    /// Every request admitted by a fake service, in call order.
    pub(super) calls: Vec<String>,
    /// The one payload-scroll gate a test requested.
    scroll_gate: Option<Arc<Notify>>,
    /// The calls to refuse next, each with the code to refuse it with.
    refusals: Vec<(&'static str, Code)>,
    /// Whether to refuse the next `CreateAlias` action.
    refuse_create_alias: bool,
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

    /// Whether the next `CreateAlias` action should refuse.
    pub(super) fn refuse_create_alias(&mut self) -> bool {
        mem::take(&mut self.refuse_create_alias)
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

    /// Blocks the next payload scroll until a test releases its gate.
    pub(super) fn gate_next_scroll(&self) -> Arc<Notify> {
        let gate = Arc::new(Notify::new());
        self.state().scroll_gate = Some(Arc::clone(&gate));
        gate
    }

    /// Waits for the next payload scroll's gate, if any.
    pub(super) async fn wait_scroll_gate(&self) {
        let gate = self.state().scroll_gate.take();
        if let Some(gate) = gate {
            gate.notified().await;
        }
    }

    /// Makes the next call `call` refuse with `code`.
    pub(super) fn refuse_next(&self, call: &'static str, code: Code) {
        self.state().refusals.push((call, code));
    }

    /// Makes the next `CreateAlias` action refuse.
    pub(super) fn refuse_create_alias_next(&self) {
        self.state().refuse_create_alias = true;
    }

    /// Makes the next call `call` answer without its result.
    pub(super) fn hollow_next(&self, call: &'static str) {
        self.state().hollows.push(call);
    }

    /// The call `call`, refused when a test asked for it, once.
    pub(super) fn admit(&self, call: &str) -> Result<(), Status> {
        let mut state = self.state();
        state.calls.push(call.to_owned());
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
