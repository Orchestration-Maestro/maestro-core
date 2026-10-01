//! The spans the kernel opens, and the names they carry, pinned in one place.
//!
//! A rename is a change to this file, which a review sees. The names follow
//! the span taxonomy of docs/architecture/05 §4 and, where it names them, the
//! semantic conventions for generative AI.

use super::stage::{COLLECTION_ID, DURATION_US, GENERATION, OUTCOME, Stage};
use tracing::field::Empty;

/// The span of one call of a registered tool.
pub const GEN_AI_EXECUTE_TOOL: &str = "gen_ai.execute_tool";

/// The attribute naming the operation a span stands for.
pub const GEN_AI_OPERATION_NAME: &str = "gen_ai.operation.name";

/// The attribute naming the tool a call runs.
pub const GEN_AI_TOOL_NAME: &str = "gen_ai.tool.name";

/// The operation of a tool call: the value of [`GEN_AI_OPERATION_NAME`] on
/// a [`GEN_AI_EXECUTE_TOOL`] span.
pub const EXECUTE_TOOL: &str = "execute_tool";

/// The span of one call of `tool`: [`GEN_AI_EXECUTE_TOOL`], carrying the
/// operation and the tool's name, then its outcome and duration once it
/// finishes. Instrument the call with it.
#[must_use]
pub fn tool_call(tool: &'static str) -> Stage {
    Stage::open(tracing::info_span!(
        GEN_AI_EXECUTE_TOOL,
        { GEN_AI_OPERATION_NAME } = EXECUTE_TOOL,
        { GEN_AI_TOOL_NAME } = tool,
        { OUTCOME } = Empty,
        { DURATION_US } = Empty,
    ))
}

/// Defines, for each stage, the function that opens its span under its
/// pinned name, with the attributes every stage may record left empty.
macro_rules! stages {
    ($($(#[doc = $doc:literal])* $function:ident => $name:literal;)*) => {$(
        $(#[doc = $doc])*
        #[must_use]
        pub fn $function() -> Stage {
            Stage::open(tracing::info_span!(
                $name,
                { OUTCOME } = Empty,
                { DURATION_US } = Empty,
                { COLLECTION_ID } = Empty,
                { GENERATION } = Empty,
                candidates = Empty,
                chunks = Empty,
                passages = Empty,
                points = Empty,
                sources = Empty,
            ))
        }
    )*};
}

stages! {
    /// `knowledge.publish`: the publication of a chunk set as a generation.
    publish => "knowledge.publish";
    /// `knowledge.publish.load_inputs`: reading the chunk set, its
    /// generation and its chunks.
    publish_load_inputs => "knowledge.publish.load_inputs";
    /// `knowledge.publish.project`: writing the generation's points.
    publish_project => "knowledge.publish.project";
    /// `knowledge.publish.embed`: the embedder representing one batch.
    publish_embed => "knowledge.publish.embed";
    /// `knowledge.publish.verify`: checking the written collection.
    publish_verify => "knowledge.publish.verify";
    /// `knowledge.publish.switch_alias`: moving the collection's alias.
    publish_switch_alias => "knowledge.publish.switch_alias";
    /// `knowledge.publish.kernel`: publishing the generation in the kernel.
    publish_kernel => "knowledge.publish.kernel";
    /// `retrieval.search`: one search, from admission to its handoff.
    search => "retrieval.search";
    /// `retrieval.route.dense`: the dense route.
    route_dense => "retrieval.route.dense";
    /// `retrieval.route.lexical`: the lexical (BM25) route.
    route_lexical => "retrieval.route.lexical";
    /// `retrieval.route.identifier`: the identifier route.
    route_identifier => "retrieval.route.identifier";
    /// `retrieval.route.structured`: the structured inventory route.
    route_structured => "retrieval.route.structured";
    /// `retrieval.fuse`: reciprocal-rank fusion of the routes' lists.
    fuse => "retrieval.fuse";
    /// `retrieval.rerank`: reranking the loaded candidates.
    rerank => "retrieval.rerank";
    /// `retrieval.assemble`: evidence assembly, from a handoff to a bundle.
    assemble => "retrieval.assemble";
    /// `retrieval.assemble.ledger`: reading the pinned duplicate ledger.
    assemble_ledger => "retrieval.assemble.ledger";
    /// `retrieval.assemble.candidates`: loading the ranked candidates.
    assemble_candidates => "retrieval.assemble.candidates";
    /// `retrieval.assemble.sources`: loading and checking their sources.
    assemble_sources => "retrieval.assemble.sources";
    /// `retrieval.assemble.sections`: expanding spans into sections.
    assemble_sections => "retrieval.assemble.sections";
    /// `retrieval.assemble.selection`: selecting the passages.
    assemble_selection => "retrieval.assemble.selection";
    /// `retrieval.assemble.output`: building the bundle.
    assemble_output => "retrieval.assemble.output";
}
