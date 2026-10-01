//! Shared response bounds and semantic search truncation for CLI and MCP.

use maestro_kernel::evidence::{Bundle, Inventory, TRUNCATED_INVENTORY_GAP_PREFIX};
use rmcp::{
    ErrorData as McpError,
    model::{
        CallToolResponse, CallToolResult, ContentBlock, MetaObject, ProtocolVersion, RequestId,
        ServerResult,
    },
    service::{RoleServer, TxJsonRpcMessage},
};
use serde::Serialize;
use serde_json::json;

/// Maximum serialized UTF-8 bytes in a complete response line.
pub(crate) const RESPONSE_LIMIT_BYTES: usize = 65_536;

/// How many complete semantic items the bounded search response dropped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub(crate) struct SearchTruncation {
    /// Lowest-ranked passages omitted from the bundle.
    pub(crate) passages: usize,
    /// Trailing inventory groups omitted from the inventory.
    pub(crate) inventory_items: usize,
}

impl SearchTruncation {
    /// Whether any content was omitted.
    pub(crate) fn is_truncated(self) -> bool {
        self.passages > 0 || self.inventory_items > 0
    }

    /// Stable category names for the transport envelopes.
    pub(crate) fn omitted(self) -> Vec<&'static str> {
        let mut omitted = Vec::new();
        if self.passages > 0 {
            omitted.push("passages");
        }
        if self.inventory_items > 0 {
            omitted.push("inventory_items");
        }
        omitted
    }
}

/// A search bundle and the semantic units removed to fit the common wire limit.
pub(crate) struct BoundedSearch {
    /// Reduced transport-neutral evidence bundle.
    pub(crate) bundle: Bundle,
    /// Counts of complete units omitted from the bundle.
    pub(crate) truncation: SearchTruncation,
}

/// Why a search response could not be encoded or reduced to the common limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SearchOutputError {
    /// The bundle or its MCP result could not be serialized safely.
    Format,
    /// Non-evidence bundle fields alone exceed the common response limit.
    TooLarge,
}

/// Drops lowest-ranked passages, then stable trailing inventory groups, until MCP fits.
pub(crate) fn truncate_search_bundle(
    mut bundle: Bundle,
) -> Result<BoundedSearch, SearchOutputError> {
    let mut truncation = SearchTruncation::default();
    let mut original_gap_count = bundle.known_gaps.len();
    let inventory_groups = match bundle.inventory.as_ref() {
        Some(Inventory::DocumentsBySet { sets, .. }) => sets.len(),
        Some(Inventory::Versions { versions, .. }) => versions.len(),
        None => 0,
    };
    let reductions = bundle.passages.len().saturating_add(inventory_groups);
    for _ in 0..=reductions {
        write_truncation_gaps(&mut bundle, original_gap_count, truncation);
        if shared_search_result_fits(&bundle, truncation)? {
            return Ok(BoundedSearch { bundle, truncation });
        }
        if let Some(number) = drop_lowest_passage(&mut bundle) {
            bundle.known_gaps.truncate(original_gap_count);
            let before = bundle.known_gaps.len();
            bundle.known_gaps.retain(|gap| !names_passage(gap, number));
            original_gap_count -= before - bundle.known_gaps.len();
            truncation.passages += 1;
            update_evidence_bytes(&mut bundle)?;
            continue;
        }
        if drop_inventory_item(&mut bundle) {
            truncation.inventory_items += 1;
            continue;
        }
        return Err(SearchOutputError::TooLarge);
    }
    Err(SearchOutputError::TooLarge)
}

/// Builds the MCP success result; its first text block is the reduced bundle JSON.
pub(crate) fn search_tool_result(
    bundle: &Bundle,
    truncation: SearchTruncation,
) -> Result<CallToolResult, SearchOutputError> {
    let value = serde_json::to_value(bundle).map_err(|_| SearchOutputError::Format)?;
    let mut result = CallToolResult::structured(value);
    if truncation.is_truncated() {
        let warning = truncation_warning(truncation);
        let mut metadata = MetaObject::new();
        metadata.0.insert(
            "maestro/truncation".to_owned(),
            json!({
                "truncated": true,
                "limit_bytes": RESPONSE_LIMIT_BYTES,
                "omitted": truncation.omitted(),
                "dropped": truncation,
                "warning": warning,
            }),
        );
        result = result.with_meta(Some(metadata));
        result.content.push(ContentBlock::text(warning));
    }
    Ok(result)
}

/// Measures the actual framed JSON-RPC response for one request ID and peer version.
pub(crate) fn response_fits(
    result: &CallToolResult,
    request_id: &RequestId,
    protocol_version: Option<&ProtocolVersion>,
) -> Result<bool, McpError> {
    Ok(response_size(result, request_id, protocol_version)? <= RESPONSE_LIMIT_BYTES)
}

/// Measures the complete response line, including its framing newline.
pub(crate) fn response_size(
    result: &CallToolResult,
    request_id: &RequestId,
    protocol_version: Option<&ProtocolVersion>,
) -> Result<usize, McpError> {
    let mut result: ServerResult = CallToolResponse::Complete(result.clone()).into();
    if protocol_version
        .is_none_or(|version| version.as_str() < ProtocolVersion::V_2026_07_28.as_str())
    {
        result.strip_result_type_for_legacy_peer();
    }
    let response = TxJsonRpcMessage::<RoleServer>::response(result, request_id.clone());
    let bytes = serde_json::to_vec(&response)
        .map_err(|_| McpError::internal_error("response serialization failed", None))?;
    Ok(bytes.len().saturating_add(1))
}

/// Removes one largest passage number, the last-ranked item in the evidence ordering.
fn drop_lowest_passage(bundle: &mut Bundle) -> Option<u32> {
    let (index, number) = bundle
        .passages
        .iter()
        .enumerate()
        .max_by_key(|(_, passage)| passage.n)
        .map(|(index, passage)| (index, passage.n))?;
    bundle.passages.remove(index);
    bundle.trace.retain(|trace| trace.n != number);
    for conflict in &mut bundle.conflicts {
        conflict.passages.retain(|passage| *passage != number);
    }
    bundle
        .conflicts
        .retain(|conflict| conflict.passages.len() >= 2);
    Some(number)
}

/// Removes one stable trailing group without changing the exact population total.
fn drop_inventory_item(bundle: &mut Bundle) -> bool {
    match bundle.inventory.as_mut() {
        Some(Inventory::DocumentsBySet { sets, .. }) => sets.pop().is_some(),
        Some(Inventory::Versions { versions, .. }) => versions.pop().is_some(),
        None => false,
    }
}

/// Refreshes the bundle's counter after whole passages were removed.
fn update_evidence_bytes(bundle: &mut Bundle) -> Result<(), SearchOutputError> {
    if bundle.passages.is_empty() {
        bundle.budget.evidence_bytes = 0;
        return Ok(());
    }
    if bundle.budget.counter.as_deref() != Some("evidence-utf8-bytes/1") {
        return Err(SearchOutputError::Format);
    }
    let bytes = serde_json::to_vec(&bundle.passages).map_err(|_| SearchOutputError::Format)?;
    bundle.budget.evidence_bytes =
        u32::try_from(bytes.len()).map_err(|_| SearchOutputError::Format)?;
    Ok(())
}

/// Replaces only the truncation gaps, preserving every remaining T032 gap.
fn write_truncation_gaps(
    bundle: &mut Bundle,
    original_gap_count: usize,
    truncation: SearchTruncation,
) {
    bundle.known_gaps.truncate(original_gap_count);
    if truncation.passages > 0 {
        bundle.known_gaps.push(format!(
            "{} lowest-ranked passages were omitted to fit the response limit.",
            truncation.passages
        ));
    }
    if truncation.inventory_items > 0 {
        bundle.known_gaps.push(format!(
            "{TRUNCATED_INVENTORY_GAP_PREFIX} {} groups were omitted; \
             remaining entries are incomplete.",
            truncation.inventory_items
        ));
    }
}

/// Whether an inherited gap identifies the omitted passage by its number.
fn names_passage(gap: &str, number: u32) -> bool {
    gap.strip_prefix("Passage ")
        .and_then(|tail| tail.split_once(' '))
        .and_then(|(number_text, _)| number_text.parse::<u32>().ok())
        == Some(number)
}

/// Uses one MCP framing policy for both transports so their reduced bundles match.
pub(super) fn shared_search_result_fits(
    bundle: &Bundle,
    truncation: SearchTruncation,
) -> Result<bool, SearchOutputError> {
    let result = search_tool_result(bundle, truncation)?;
    let request_id = RequestId::String("x".repeat(256).into());
    let version = ProtocolVersion::V_2026_07_28;
    response_fits(&result, &request_id, Some(&version)).map_err(|_| SearchOutputError::Format)
}

/// The second MCP text block warns clients that retained evidence is incomplete.
fn truncation_warning(truncation: SearchTruncation) -> String {
    format!(
        "Search result truncated: dropped {} passages and {} inventory items; not exhaustive.",
        truncation.passages, truncation.inventory_items
    )
}
