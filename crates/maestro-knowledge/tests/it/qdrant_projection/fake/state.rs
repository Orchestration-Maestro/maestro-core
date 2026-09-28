//! What the fake keeps, and the refusals and hollow answers a test asked for.

use qdrant_client::qdrant::{
    CollectionParams, Filter, HnswConfigDiff, PointId, RetrievedPoint, point_id::PointIdOptions,
};
use std::{
    collections::BTreeMap,
    mem,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
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
    /// The collection the next `CreateAlias` action points its alias at
    /// instead of its own, as a concurrent alias change would.
    redirect_create_alias: Option<String>,
    /// The calls to answer next without their result.
    hollows: Vec<&'static str>,
    /// The queries to answer late next, as a loaded host or a slow refused
    /// connection would.
    slow_queries: Vec<SlowQuery>,
}

/// A query a test made slow: the named vector it searches, how long it
/// waits, and the code it is then refused with, when it is.
#[derive(Debug, Clone, Copy)]
pub(in super::super) struct SlowQuery {
    /// The named vector of the query: `dense` or `bm25`.
    pub(in super::super) using: &'static str,
    /// How long the query waits before it answers; `None` never answers.
    pub(in super::super) delay: Option<Duration>,
    /// The code it is refused with once it waited, or `None` to answer.
    pub(in super::super) refusal: Option<Code>,
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

    /// The collection the next `CreateAlias` action points at instead of its
    /// own, if a test asked for one.
    pub(super) fn redirect_create_alias(&mut self) -> Option<String> {
        self.redirect_create_alias.take()
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

    /// Points the next `CreateAlias` action's alias at `collection` instead.
    pub(super) fn redirect_create_alias_next(&self, collection: &str) {
        self.state().redirect_create_alias = Some(collection.to_owned());
    }

    /// Makes the next query of `slow.using` wait, then answer or refuse.
    pub(super) fn slow_next_query(&self, slow: SlowQuery) {
        self.state().slow_queries.push(slow);
    }

    /// The slowness a test asked for the next query of `using`, once.
    pub(super) fn slow_query(&self, using: &str) -> Option<SlowQuery> {
        let mut state = self.state();
        let at = state
            .slow_queries
            .iter()
            .position(|slow| slow.using == using)?;
        Some(state.slow_queries.remove(at))
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
