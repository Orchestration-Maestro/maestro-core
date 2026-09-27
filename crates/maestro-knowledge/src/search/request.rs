//! Retrieval results handed to T032 without assembling an evidence bundle.

use super::rerank::Ranked;
use crate::query::Understood;
use maestro_kernel::{
    evidence::{Inventory, RequestBudget, RouteStatus},
    generation::Generation,
    scope::ScopeSet,
};
use std::{collections::BTreeMap, sync::Arc};
use tokio::time::Instant;

/// Retrieval results handed to T032 without assembling an evidence bundle.
#[derive(Debug, Clone)]
pub struct EvidenceInput {
    /// The single generation pinned at admission.
    pub generation: Generation,
    /// The original question.
    pub query: String,
    /// The bounded local interpretation of that question.
    pub understood: Understood,
    /// The exact effective version filter.
    pub version: Option<String>,
    /// The trusted caller whose permissions admitted this search.
    pub principal: String,
    /// The immutable scope snapshot accepted at admission.
    pub scopes: Arc<ScopeSet>,
    /// The final reranked candidates, with fusion data preserved.
    pub ranked: Vec<Ranked>,
    /// The status of each route that participated in the search.
    pub routes: BTreeMap<String, RouteStatus>,
    /// The complete exact inventory, if structured routing succeeded.
    pub inventory: Option<Inventory>,
    /// The accepted request limits echoed to evidence.
    pub budget: RequestBudget,
    /// The absolute deadline shared with T032.
    pub deadline: Instant,
    /// Gaps known before evidence expansion.
    pub known_gaps: Vec<String>,
}
