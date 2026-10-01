//! Wire-size and output-shaping tests for the MCP handler.

use super::response::{
    CollectionsOutput, bounded_collections_result, response_fits, response_size,
};
use crate::{
    knowledge::RESPONSE_LIMIT_BYTES,
    knowledge::operations::{CollectionItem, CollectionsData},
    mcp::transport::BoundedStdio,
};
use rmcp::{
    model::{
        CallToolResponse, CallToolResult, MetaObject, ProtocolVersion, RequestId, ServerResult,
    },
    service::{RoleServer, TxJsonRpcMessage},
    transport::Transport,
};
use serde_json::json;
use std::cmp::Ordering;
use tokio::{
    io::{AsyncReadExt as _, duplex},
    runtime::Builder,
};

#[test]
fn exact_wire_limits_include_large_ids_escaping_and_sdk_duplication() {
    let protocol_version = ProtocolVersion::V_2025_11_25;
    for (target, request_id) in [
        (65_535, RequestId::String("i".repeat(255).into())),
        (65_536, RequestId::String("i".repeat(256).into())),
        (65_537, RequestId::String("i".repeat(255).into())),
    ] {
        let content = find_payload(target, &request_id, &protocol_version)
            .expect("payload at exact wire size");
        let result = bounded_result(&content);
        assert_eq!(
            response_size(&result, &request_id, Some(&protocol_version)).expect("wire size"),
            target
        );
        assert_eq!(
            response_fits(&result, &request_id, Some(&protocol_version)).expect("fit"),
            target <= RESPONSE_LIMIT_BYTES
        );
        let written = transport_wire(&result, &request_id, &protocol_version);
        if target <= RESPONSE_LIMIT_BYTES {
            assert_eq!(written.len(), target);
        } else {
            assert!(written.len() <= RESPONSE_LIMIT_BYTES);
            assert_ne!(written.len(), target);
            let fallback: serde_json::Value =
                serde_json::from_slice(&written[..written.len() - 1]).expect("bounded fallback");
            assert_eq!(
                fallback["id"],
                serde_json::to_value(request_id).expect("request ID")
            );
            assert_eq!(fallback["error"]["message"], "response_too_large");
        }
    }
}

#[test]
fn legacy_protocols_omit_the_complete_result_type() {
    let result = CallToolResult::structured(json!({"payload": "value"}));
    let request_id = RequestId::Number(1);
    let legacy = response_size(&result, &request_id, Some(&ProtocolVersion::V_2025_11_25))
        .expect("legacy wire size");
    let current = response_size(&result, &request_id, Some(&ProtocolVersion::V_2026_07_28))
        .expect("current wire size");
    assert!(legacy < current);
}

#[test]
fn collection_truncation_drops_whole_trailing_entries_and_marks_the_response() {
    let request_id = RequestId::Number(1);
    let protocol_version = ProtocolVersion::V_2025_11_25;
    let result = bounded_collections_result(
        CollectionsOutput {
            data: CollectionsData {
                schema: "maestro-knowledge-collections/1",
                collections: vec![
                    CollectionItem {
                        id: "first".to_owned(),
                        title: "visible".to_owned(),
                        published_generation: Some(1),
                    },
                    CollectionItem {
                        id: "second".to_owned(),
                        title: "x".repeat(40_000),
                        published_generation: None,
                    },
                ],
            },
            truncated: false,
            omitted: Vec::new(),
        },
        &request_id,
        Some(&protocol_version),
    )
    .expect("bounded result");
    let value = serde_json::to_value(&result).expect("serializable result");
    assert!(response_fits(&result, &request_id, Some(&protocol_version)).expect("size"));
    assert_eq!(
        value["structuredContent"]["collections"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(value["structuredContent"]["collections"][0]["id"], "first");
    assert_eq!(
        value["_meta"]["maestro/truncation"]["omitted"][0],
        "collections"
    );
    assert_eq!(
        value["_meta"]["maestro/truncation"]["warning"],
        "Some collection entries were omitted to fit the response limit."
    );
}

fn transport_wire(
    result: &CallToolResult,
    request_id: &RequestId,
    protocol_version: &ProtocolVersion,
) -> Vec<u8> {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let mut result: ServerResult = CallToolResponse::Complete(result.clone()).into();
        if protocol_version.as_str() < ProtocolVersion::V_2026_07_28.as_str() {
            result.strip_result_type_for_legacy_peer();
        }
        let message = TxJsonRpcMessage::<RoleServer>::response(result, request_id.clone());
        let (server_input, _client_input) = duplex(1);
        let (server_output, mut client_output) = duplex(RESPONSE_LIMIT_BYTES + 1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        transport
            .send(message)
            .await
            .expect("write bounded response");
        drop(transport);
        let mut wire = Vec::new();
        client_output
            .read_to_end(&mut wire)
            .await
            .expect("read written response");
        wire
    })
}

fn find_payload(
    target: usize,
    request_id: &RequestId,
    protocol_version: &ProtocolVersion,
) -> Option<String> {
    let prefix = "é\\\"";
    for suffix in ["", "\""] {
        let mut low = 0;
        let mut high = target;
        while low <= high {
            let middle = low + (high - low) / 2;
            let content = format!("{prefix}{}{suffix}", "x".repeat(middle));
            let result = bounded_result(&content);
            let size = response_size(&result, request_id, Some(protocol_version)).ok()?;
            match size.cmp(&target) {
                Ordering::Equal => return Some(content),
                Ordering::Less => low = middle + 1,
                Ordering::Greater if middle > 0 => high = middle - 1,
                Ordering::Greater => break,
            }
        }
    }
    None
}

fn bounded_result(content: &str) -> CallToolResult {
    let mut metadata = MetaObject::new();
    metadata.0.insert(
        "maestro/truncation".to_owned(),
        json!({"truncated": true, "limit_bytes": RESPONSE_LIMIT_BYTES, "omitted": ["inventory"]}),
    );
    CallToolResult::structured(json!({"payload": content})).with_meta(Some(metadata))
}
