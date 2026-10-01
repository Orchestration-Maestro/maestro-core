//! Normalized topology inputs. Session bindings, start and terminal facts are
//! resolved compile inputs, not additions to the authored workflow format.

use crate::source::ResourceId;
use std::collections::BTreeMap;

/// Admission of completed protected review for an exact resource identity.
/// Source labels and area ownership do not implement this port. There is no
/// production adapter yet: C22b must supply admitted evidence. S4 execution
/// additionally requires qualification, which topology compilation never grants.
pub(crate) trait ReviewEvidence {
    /// Whether this exact resource has admitted reviewed evidence.
    fn reviewed(&self, id: &ResourceId) -> bool;
}

/// The resolved session, model profile and provider for an agent node.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Bindings {
    /// Distinct session identity, not a role label.
    pub(crate) session: Option<String>,
    /// Resolved model profile identity.
    pub(crate) profile: Option<String>,
    /// Resolved provider identity.
    pub(crate) provider: Option<String>,
}

/// The seven supported architecture 03 node constructs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NodeKind {
    /// Agent and optional skill spellings retained for precise refusal diagnostics.
    Agent {
        /// An exact qualified agent reference, never a path or basename.
        agent: String,
        /// An optional exact qualified skill reference.
        skill: Option<String>,
    },
    /// Deterministic step; sandbox validation belongs to C22b.
    Step,
    /// Human or automated gate.
    Gate,
    /// Typed edge choice; exact choices belong to C22b.
    Router,
    /// Bounded fan-out.
    Map {
        /// Positive resolved fan-out ceiling.
        max_items: Option<usize>,
    },
    /// Fan-in; join policy validation belongs to C22b.
    Join,
    /// Call to a declared workflow.
    Subgraph {
        /// Exact qualified workflow reference, retaining the original spelling.
        workflow: String,
        /// Positive resolved depth ceiling; aggregate nesting belongs to C22b.
        max_depth: Option<usize>,
    },
    /// An unsupported source construct, which must refuse rather than disappear.
    Unknown(String),
}

/// One normalized node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Node {
    /// Its supported construct and exact reference spellings.
    pub(crate) kind: NodeKind,
    /// Whether this node is an explicitly resolved terminal.
    pub(crate) terminal: bool,
    /// Resolved agent-session identities, never qualification evidence.
    pub(crate) bindings: Bindings,
    /// Nodes from which this reviewer must be independent.
    pub(crate) independent_of: Vec<String>,
}

/// One normalized edge; conditions are checked by C22b.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Edge {
    /// Source node name.
    pub(crate) from: String,
    /// Target node name.
    pub(crate) to: String,
    /// A positive loop ceiling; zero does not bound a cycle.
    pub(crate) max_iterations: Option<usize>,
}

/// The topology of a checked workflow and its resolved compile facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Workflow {
    /// Owner-relative workflow source path.
    pub(crate) source: String,
    /// Exact typed workflow identity.
    pub(crate) id: ResourceId,
    /// The exact declared requirements, matching the checked catalog resource.
    pub(crate) requires: Vec<ResourceId>,
    /// The explicitly resolved start node.
    pub(crate) start: String,
    /// Nodes keyed by graph-local name.
    pub(crate) nodes: BTreeMap<String, Node>,
    /// Declared graph-local edges.
    pub(crate) edges: Vec<Edge>,
}
