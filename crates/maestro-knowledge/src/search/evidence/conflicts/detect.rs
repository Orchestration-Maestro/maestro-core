//! Groups exact table facts into context- and manifest-authorized conflicts.

use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::evidence::Span;
use std::collections::{BTreeMap, BTreeSet};

use super::super::sections::{contains, valid_span};
use super::tables::{FactKey, TableFact, table_facts};

/// Exact product context required for two facts to correspond.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ConflictContext {
    /// Product metadata, if present on both revisions.
    pub(crate) product: Option<String>,
    /// Component metadata, if present on both revisions.
    pub(crate) component: Option<String>,
    /// Platform metadata, if present on both revisions.
    pub(crate) platform: Option<String>,
    /// Language metadata, if present on both revisions.
    pub(crate) lang: Option<String>,
}

/// Authorized candidate source supplied to structured conflict detection.
pub(crate) struct ConflictSource<'a> {
    /// Stable index in the retained candidate pool.
    pub(crate) candidate_index: usize,
    /// Canonical owning document ID.
    pub(crate) document_id: &'a str,
    /// Canonical source revision ID.
    pub(crate) revision_id: &'a str,
    /// Manifest-allowed near-duplicate group IDs.
    pub(crate) near_group_ids: &'a BTreeSet<String>,
    /// Exact metadata used to constrain family matching.
    pub(crate) context: &'a ConflictContext,
    /// Proposed full expansion containing this candidate's evidence.
    pub(crate) proposed_extent: Span,
    /// Validated canonical structure of the source revision.
    pub(crate) document: &'a CanonicalDocument,
    /// Authoritative original Markdown bytes decoded as UTF-8.
    pub(crate) markdown: &'a str,
}

/// Explicit differing table values across one family and table identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConflictFinding {
    /// Entity or parameter name preserved from the table.
    pub(crate) entity: String,
    /// Attribute name preserved from the table.
    pub(crate) attribute: String,
    /// Distinct explicit values found in the family.
    pub(crate) values: BTreeSet<String>,
    /// Candidate indices that supply conflicting rows.
    pub(crate) candidate_indices: BTreeSet<usize>,
    /// Full table spans by candidate index.
    pub(crate) table_spans: BTreeMap<usize, Vec<Span>>,
}

/// Candidate identity and correspondence metadata for one table observation.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CandidateLink {
    /// Stable index in the candidate pool.
    candidate_index: usize,
    /// Owning canonical document ID.
    document_id: String,
    /// Manifest-allowed duplicate groups.
    near_group_ids: BTreeSet<String>,
    /// Exact family context.
    context: ConflictContext,
}

/// One canonical table row observed through one or more candidates.
#[derive(Debug)]
struct Observation {
    /// Parsed row fact and its table identity.
    fact: TableFact,
    /// Candidates whose proposed extents contain the table.
    candidates: BTreeMap<usize, CandidateLink>,
}

/// Family partition for exact conflict comparison.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PartitionKey {
    /// Canonical heading path containing the table.
    section_path: Vec<String>,
    /// One-based occurrence of that path in canonical section order.
    occurrence: usize,
    /// Exact product context shared by the candidates.
    context: ConflictContext,
    /// Table entity or parameter.
    entity: String,
    /// Table attribute or the `default` field.
    attribute: String,
}

/// Candidate graph and table provenance for one conflict partition.
#[derive(Debug, Default)]
struct ConflictGraph {
    /// Undirected edges connecting candidates with differing values.
    edges: BTreeMap<usize, BTreeSet<usize>>,
    /// Differing values observed for each candidate.
    values: BTreeMap<usize, BTreeSet<String>>,
    /// Full supporting table spans for each candidate.
    table_spans: BTreeMap<usize, Vec<Span>>,
}

/// One parsed fact paired with its candidate metadata.
struct FactCandidate<'a> {
    /// Parsed table row.
    fact: &'a TableFact,
    /// Candidate that exposes the table row.
    candidate: &'a CandidateLink,
}

/// Parsed canonical tables cached for one source revision.
struct RevisionTables<'a> {
    /// Canonical owning document ID.
    document_id: &'a str,
    /// Manifest-allowed duplicate groups for this revision.
    near_group_ids: &'a BTreeSet<String>,
    /// Exact product context for this revision.
    context: &'a ConflictContext,
    /// Canonical document structure.
    document: &'a CanonicalDocument,
    /// Original Markdown source bytes.
    markdown: &'a str,
    /// Supported table facts for this revision.
    facts: Vec<TableFact>,
}

/// Finds possible conflicts in authorized, structurally matched candidates.
pub(crate) fn detect_conflicts(
    sources: &[ConflictSource<'_>],
) -> Result<Vec<ConflictFinding>, String> {
    let revisions = load_revision_tables(sources)?;
    let observations = observations(sources, &revisions)?;
    let mut partitions: BTreeMap<PartitionKey, Vec<FactCandidate<'_>>> = BTreeMap::new();
    for observation in observations.values() {
        for candidate in observation.candidates.values() {
            let key = PartitionKey {
                section_path: observation.fact.section_path.clone(),
                occurrence: observation.fact.occurrence,
                context: candidate.context.clone(),
                entity: observation.fact.entity.clone(),
                attribute: observation.fact.attribute.clone(),
            };
            partitions.entry(key).or_default().push(FactCandidate {
                fact: &observation.fact,
                candidate,
            });
        }
    }

    let mut findings = Vec::new();
    for (partition, facts) in partitions {
        let graph = conflict_graph(&facts);
        for component in components(&graph.edges) {
            let values: BTreeSet<_> = component
                .iter()
                .filter_map(|index| graph.values.get(index))
                .flatten()
                .cloned()
                .collect();
            findings.push(ConflictFinding {
                entity: partition.entity.clone(),
                attribute: partition.attribute.clone(),
                values,
                candidate_indices: component.clone(),
                table_spans: component
                    .iter()
                    .filter_map(|index| {
                        graph
                            .table_spans
                            .get(index)
                            .map(|spans| (*index, spans.clone()))
                    })
                    .collect(),
            });
        }
    }
    Ok(findings)
}

/// Connects each pair of candidates with differing corresponding values.
fn conflict_graph(facts: &[FactCandidate<'_>]) -> ConflictGraph {
    let mut graph = ConflictGraph::default();
    for (left_index, left) in facts.iter().enumerate() {
        for right in facts.iter().skip(left_index.saturating_add(1)) {
            add_conflict_edge(&mut graph, left, right);
        }
    }
    graph
}

/// Adds an undirected edge when two candidate observations conflict.
fn add_conflict_edge(
    graph: &mut ConflictGraph,
    left: &FactCandidate<'_>,
    right: &FactCandidate<'_>,
) {
    if left.fact.value == right.fact.value || !corresponds(left.candidate, right.candidate) {
        return;
    }
    graph
        .edges
        .entry(left.candidate.candidate_index)
        .or_default()
        .insert(right.candidate.candidate_index);
    graph
        .edges
        .entry(right.candidate.candidate_index)
        .or_default()
        .insert(left.candidate.candidate_index);
    add_observation(graph, left);
    add_observation(graph, right);
}

/// Validates candidate identity and caches the parsed tables per revision.
fn load_revision_tables<'a>(
    sources: &[ConflictSource<'a>],
) -> Result<BTreeMap<String, RevisionTables<'a>>, String> {
    let mut revisions: BTreeMap<String, RevisionTables<'a>> = BTreeMap::new();
    for source in sources {
        if source.document_id.trim().is_empty()
            || source.revision_id.trim().is_empty()
            || source.document.document_id != source.document_id
            || source.document.revision_id != source.revision_id
            || source.near_group_ids.iter().any(|id| id.trim().is_empty())
            || source.proposed_extent.start >= source.proposed_extent.end
            || !valid_span(source.proposed_extent, source.markdown)
        {
            return Err("conflict source identity or extent is invalid".to_owned());
        }
        if let Some(previous) = revisions.get(source.revision_id) {
            if previous.document_id != source.document_id
                || previous.near_group_ids != source.near_group_ids
                || previous.context != source.context
                || previous.document != source.document
                || previous.markdown != source.markdown
            {
                return Err("one revision has inconsistent canonical source data".to_owned());
            }
        } else {
            revisions.insert(
                source.revision_id.to_owned(),
                RevisionTables {
                    document_id: source.document_id,
                    near_group_ids: source.near_group_ids,
                    context: source.context,
                    document: source.document,
                    markdown: source.markdown,
                    facts: table_facts(source.document, source.markdown)?,
                },
            );
        }
    }
    Ok(revisions)
}

/// Deduplicates each table row across candidates covering the same table.
fn observations<'a>(
    sources: &'a [ConflictSource<'a>],
    revisions: &'a BTreeMap<String, RevisionTables<'a>>,
) -> Result<BTreeMap<FactKey, Observation>, String> {
    let mut observations = BTreeMap::new();
    for source in sources {
        let revision = revisions
            .get(source.revision_id)
            .ok_or_else(|| "conflict source revision is missing".to_owned())?;
        for fact in &revision.facts {
            if !contains(source.proposed_extent, fact.table_span) {
                continue;
            }
            let link = CandidateLink {
                candidate_index: source.candidate_index,
                document_id: source.document_id.to_owned(),
                near_group_ids: source.near_group_ids.clone(),
                context: source.context.clone(),
            };
            let observation = observations
                .entry(fact.key.clone())
                .or_insert_with(|| Observation {
                    fact: fact.clone(),
                    candidates: BTreeMap::new(),
                });
            if observation.fact != *fact {
                return Err("duplicate canonical table observation differs".to_owned());
            }
            if observation
                .candidates
                .insert(source.candidate_index, link.clone())
                .is_some_and(|previous| previous != link)
            {
                return Err("one candidate has inconsistent conflict metadata".to_owned());
            }
        }
    }
    Ok(observations)
}

/// Tests whether candidates share a document or an allowed near group.
fn corresponds(left: &CandidateLink, right: &CandidateLink) -> bool {
    left.document_id == right.document_id
        || left
            .near_group_ids
            .intersection(&right.near_group_ids)
            .next()
            .is_some()
}

/// Adds one side of a conflict edge and its table provenance.
fn add_observation(graph: &mut ConflictGraph, fact: &FactCandidate<'_>) {
    let index = fact.candidate.candidate_index;
    graph
        .values
        .entry(index)
        .or_default()
        .insert(fact.fact.value.clone());
    let spans = graph.table_spans.entry(index).or_default();
    if !spans.contains(&fact.fact.table_span) {
        spans.push(fact.fact.table_span);
        spans.sort_by_key(|span| (span.start, span.end));
    }
}

/// Returns the connected candidate sets in an undirected conflict graph.
fn components(graph: &BTreeMap<usize, BTreeSet<usize>>) -> Vec<BTreeSet<usize>> {
    let mut remaining: BTreeSet<_> = graph.keys().copied().collect();
    let mut components = Vec::new();
    while let Some(start) = remaining.iter().next().copied() {
        let mut pending = vec![start];
        let mut component = BTreeSet::new();
        while let Some(index) = pending.pop() {
            if !component.insert(index) {
                continue;
            }
            remaining.remove(&index);
            if let Some(neighbors) = graph.get(&index) {
                pending.extend(neighbors.iter().copied());
            }
        }
        components.push(component);
    }
    components
}
