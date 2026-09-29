use super::{
    knowledge_server::KnowledgeServer,
    operations::{CollectionsRequest, InputFailure, parse_operation},
    response::{get_result, operation_error},
};
use crate::{
    kernel::Kernel,
    knowledge::operations::{GetData, GetExcerpt, KnowledgeError},
    knowledge::{RESPONSE_LIMIT_BYTES, RefreshScratch, RequestError},
    mcp::transport::BoundedStdio,
};
use maestro_test_scratch::scratch_directory;
use rmcp::{ServerHandler, ServiceExt, model::ProtocolVersion};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Barrier, mpsc},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, DuplexStream, duplex},
    spawn,
    task::spawn_blocking,
    time::timeout,
};

mod call_deadlines;
mod search_workers;

/// A bound that only stops a hung test; it is generous so a loaded
/// machine cannot fail a correct run.
pub(super) const HANG_GUARD: Duration = Duration::from_secs(30);

#[test]
fn empty_collection_arguments_are_strict_and_refuse_identity_fields() {
    assert!(CollectionsRequest::parse(json!({})).is_ok());
    assert!(matches!(
        CollectionsRequest::parse(json!({"principal": "other"})),
        Err(RequestError::InvalidArguments)
    ));
}

#[test]
fn malformed_search_arguments_are_tool_errors_before_worker_admission() {
    let error = match parse_operation(
        "knowledge_search",
        json!({"collection": "collection", "query": "question", "principal": "other"}),
    ) {
        Err(InputFailure::Tool { code, .. }) => code,
        Err(InputFailure::Protocol(_)) | Ok(_) => panic!("malformed search arguments refused"),
    };
    assert_eq!(error, "invalid_arguments");
}

#[test]
fn server_info_and_tool_lookup_match_the_read_only_contract() {
    let server = KnowledgeServer::for_tests();
    let info = server.get_info();
    assert_eq!(info.server_info.name, "maestro");
    assert!(info.instructions.is_some());

    for name in [
        "knowledge_collections",
        "knowledge_get",
        "knowledge_search",
        "knowledge_ask",
    ] {
        let tool = server.get_tool(name).expect("known tool");
        assert_eq!(tool.name.as_ref(), name);
    }
    assert!(server.get_tool("unknown").is_none());
}

#[test]
fn protocol_versions_match_rmcp_defaults() {
    let server = KnowledgeServer::for_tests();
    assert_eq!(
        server.supported_protocol_versions().as_ref(),
        ProtocolVersion::KNOWN_VERSIONS
    );
}

#[tokio::test]
async fn collections_delivery_refuses_access_revoked_after_operation_refresh() {
    let scratch = RefreshScratch::new();
    let server = KnowledgeServer::with_kernel_opener(move || scratch.kernel(Some(1)), HANG_GUARD);
    let response = serve_one_call(server, handshake_and_call(2), 2).await;
    assert_eq!(tool_error(&response)["error"]["code"], "access_changed");
}

#[tokio::test]
async fn get_delivery_refuses_access_revoked_after_operation_refreshes() {
    let scratch = RefreshScratch::new();
    let server = KnowledgeServer::with_kernel_opener(move || scratch.kernel(Some(2)), HANG_GUARD);
    let response = serve_one_call(server, handshake_and_get_call(2), 2).await;
    assert_eq!(tool_error(&response)["error"]["code"], "access_changed");
}

#[test]
fn get_result_keeps_exact_source_data_in_structured_content() {
    let result = get_result(GetData {
        schema: "maestro-knowledge-get/1",
        collection: "collection".to_owned(),
        generation: 7,
        excerpt: GetExcerpt {
            chunk_id: Some("chunk".to_owned()),
            document_id: "document".to_owned(),
            revision_id: "revision".to_owned(),
            section_id: Some("section".to_owned()),
            source_ref: "corpus-path:source.md".to_owned(),
            title: Some("Source".to_owned()),
            version: Some("1".to_owned()),
            section_path: None,
            span: [0, 5],
            digest: "sha256:digest".to_owned(),
            text: "exact".to_owned(),
        },
    })
    .expect("serialize exact get result");
    let value: Value = serde_json::to_value(result).expect("structured result JSON");
    assert_eq!(value["isError"], false);
    assert_eq!(
        value["structuredContent"]["schema"],
        "maestro-knowledge-get/1"
    );
    assert_eq!(value["structuredContent"]["excerpt"]["text"], "exact");
    assert_eq!(value["structuredContent"]["excerpt"]["span"], json!([0, 5]));
    assert_eq!(
        value["structuredContent"]["excerpt"]["digest"],
        "sha256:digest"
    );
    assert!(value["structuredContent"]["excerpt"]["section_path"].is_null());
}

#[test]
fn operation_error_marks_an_over_limit_section_as_truncated_without_excerpt_data() {
    let result = operation_error(KnowledgeError::Refused {
        code: "response_too_large",
        message: "the exact excerpt exceeds the response limit",
    });
    let value = serde_json::to_value(result).expect("tool error JSON");
    assert_eq!(value["isError"], true);
    assert!(value["structuredContent"].is_null());
    let text = value["content"][0]["text"]
        .as_str()
        .expect("tool error text");
    let error: Value = serde_json::from_str(text).expect("structured tool error");
    assert_eq!(error["truncated"], true);
    assert_eq!(error["omitted"], json!(["excerpt"]));
}

#[test]
fn get_result_refuses_only_text_over_the_indivisible_excerpt_limit() {
    for (text_len, is_error) in [
        (RESPONSE_LIMIT_BYTES, false),
        (RESPONSE_LIMIT_BYTES + 1, true),
    ] {
        let result = get_result(GetData {
            schema: "maestro-knowledge-get/1",
            collection: "collection".to_owned(),
            generation: 7,
            excerpt: GetExcerpt {
                chunk_id: Some("chunk".to_owned()),
                document_id: "document".to_owned(),
                revision_id: "revision".to_owned(),
                section_id: None,
                source_ref: "source".to_owned(),
                title: None,
                version: None,
                section_path: None,
                span: [0, text_len],
                digest: "sha256:digest".to_owned(),
                text: "x".repeat(text_len),
            },
        })
        .expect("get result");
        let value = serde_json::to_value(result).expect("get result JSON");
        assert_eq!(value["isError"], is_error, "text length {text_len}");
        if is_error {
            let text = value["content"][0]["text"]
                .as_str()
                .expect("error JSON text");
            let error: Value = serde_json::from_str(text).expect("tool error JSON");
            assert_eq!(error["error"]["code"], "response_too_large");
            assert!(value["structuredContent"].is_null());
        } else {
            assert_eq!(
                value["structuredContent"]["excerpt"]["text"]
                    .as_str()
                    .map(str::len),
                Some(text_len)
            );
        }
    }
}

#[tokio::test]
async fn cancellation_keeps_the_worker_permit_and_busy_is_a_json_text_error() {
    let home = ServerHome::new();
    let barrier = Arc::new(Barrier::new(2));
    let (started_tx, started_rx) = mpsc::channel();
    let server = home.server_with_barrier(barrier.clone(), started_tx, Duration::from_secs(5));
    let workers = server.workers.clone();
    let held = workers
        .clone()
        .try_acquire_many_owned(3)
        .expect("reserve three worker permits");
    let (server_input, mut client_input) = duplex(4096);
    let (server_output, client_output) = duplex(16_384);
    let serving = spawn(server.serve(BoundedStdio::new(server_input, server_output)));
    client_input
        .write_all(&handshake_and_call(2))
        .await
        .expect("write initial call");
    let service = timeout(HANG_GUARD, serving)
        .await
        .expect("MCP handshake deadline")
        .expect("join service handshake")
        .expect("start test service");
    let waiting = spawn(service.waiting());
    spawn_blocking(move || started_rx.recv_timeout(HANG_GUARD))
        .await
        .expect("join opener wait")
        .expect("blocking operation started");
    let release_worker = ReleaseBarrierOnDrop(barrier.clone());
    client_input
        .write_all(&cancel_and_call(2, 3))
        .await
        .expect("write cancel and busy call");
    let responses = responses_for(client_output, &[3]).await;
    drop(client_input);
    assert!(!responses.iter().any(|response| response["id"] == 2));
    assert_eq!(tool_error(response(&responses, 3))["error"]["code"], "busy");
    drop(release_worker);
    let worker_finished = timeout(HANG_GUARD, workers.acquire_owned())
        .await
        .expect("blocking worker shutdown deadline")
        .expect("worker permit released");
    drop(worker_finished);
    drop(held);
    timeout(HANG_GUARD, waiting)
        .await
        .expect("service shutdown deadline")
        .expect("join service")
        .expect("clean service shutdown");
}

#[tokio::test]
async fn deadline_returns_a_refusal_while_the_blocking_worker_keeps_its_permit() {
    let home = ServerHome::new();
    let barrier = Arc::new(Barrier::new(2));
    let (started_tx, started_rx) = mpsc::channel();
    let server = home.server_with_barrier(barrier.clone(), started_tx, Duration::from_millis(50));
    let workers = server.workers.clone();
    let held = workers
        .clone()
        .try_acquire_many_owned(3)
        .expect("reserve three worker permits");
    let (server_input, mut client_input) = duplex(4096);
    let (server_output, client_output) = duplex(16_384);
    let serving = spawn(server.serve(BoundedStdio::new(server_input, server_output)));
    client_input
        .write_all(&handshake_and_call(2))
        .await
        .expect("write initial call");
    let service = timeout(HANG_GUARD, serving)
        .await
        .expect("MCP handshake deadline")
        .expect("join service handshake")
        .expect("start test service");
    let waiting = spawn(service.waiting());
    spawn_blocking(move || started_rx.recv_timeout(HANG_GUARD))
        .await
        .expect("join opener wait")
        .expect("blocking operation started");
    let release_worker = ReleaseBarrierOnDrop(barrier.clone());
    let responses = responses_for(client_output, &[2]).await;
    let response = response(&responses, 2);
    assert_eq!(tool_error(response)["error"]["code"], "deadline_exceeded");
    assert!(
        workers.clone().try_acquire_owned().is_err(),
        "blocking worker keeps its permit after the deadline",
    );
    drop(release_worker);
    let worker_finished = timeout(HANG_GUARD, workers.acquire_owned())
        .await
        .expect("blocking worker shutdown deadline")
        .expect("worker permit released");
    drop(worker_finished);
    drop(held);
    drop(client_input);
    timeout(HANG_GUARD, waiting)
        .await
        .expect("service shutdown deadline")
        .expect("join service")
        .expect("clean service shutdown");
}

struct ReleaseBarrierOnDrop(Arc<Barrier>);

impl Drop for ReleaseBarrierOnDrop {
    fn drop(&mut self) {
        self.0.wait();
    }
}

struct ServerHome(PathBuf);

impl ServerHome {
    fn new() -> Self {
        let root = scratch_directory().unwrap();
        let data = root.join("data");
        let config = root.join("config");
        fs::create_dir_all(&data).expect("create data directory");
        fs::create_dir_all(&config).expect("create config directory");
        fs::write(config.join("config.toml"), "[access]\nread = []\n")
            .expect("write isolated config");
        Self(root)
    }

    fn server_with_barrier(
        &self,
        barrier: Arc<Barrier>,
        started: mpsc::Sender<()>,
        deadline: Duration,
    ) -> KnowledgeServer {
        let data = self.0.join("data");
        let config = self.0.join("config");
        KnowledgeServer::with_kernel_opener(
            move || {
                started.send(()).expect("signal blocking opener");
                barrier.wait();
                Kernel::open_at(&data, &config)
            },
            deadline,
        )
    }
}

impl Drop for ServerHome {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove isolated MCP home");
    }
}

fn handshake_and_call(id: i64) -> Vec<u8> {
    handshake_and_tool_call(id, "knowledge_collections", &json!({}))
}

fn handshake_and_get_call(id: i64) -> Vec<u8> {
    handshake_and_tool_call(
        id,
        "knowledge_get",
        &json!({"chunk_id": "chunk", "collection": "collection"}),
    )
}

pub(super) fn handshake_and_tool_call(id: i64, name: &str, arguments: &Value) -> Vec<u8> {
    let lines = [
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments}
        }),
    ];
    format!(
        "{}\n",
        lines
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    )
    .into_bytes()
}

fn cancel_and_call(cancelled_id: i64, busy_id: i64) -> Vec<u8> {
    cancel_and_tool_call(cancelled_id, busy_id, "knowledge_collections", &json!({}))
}

fn cancel_and_tool_call(cancelled_id: i64, next_id: i64, name: &str, arguments: &Value) -> Vec<u8> {
    let lines = [
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": {"requestId": cancelled_id, "reason": "test cancellation"}
        }),
        json!({
            "jsonrpc": "2.0",
            "id": next_id,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments}
        }),
    ];
    format!(
        "{}\n",
        lines
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    )
    .into_bytes()
}

pub(super) async fn serve_one_call(server: KnowledgeServer, request: Vec<u8>, id: i64) -> Value {
    let (server_input, mut client_input) = duplex(4096);
    let (server_output, client_output) = duplex(16_384);
    let serving = spawn(server.serve(BoundedStdio::new(server_input, server_output)));
    client_input
        .write_all(&request)
        .await
        .expect("write MCP call");
    let service = timeout(HANG_GUARD, serving)
        .await
        .expect("MCP startup deadline")
        .expect("join service startup")
        .expect("start service");
    let waiting = spawn(service.waiting());
    let responses = responses_for(client_output, &[id]).await;
    drop(client_input);
    timeout(HANG_GUARD, waiting)
        .await
        .expect("service shutdown deadline")
        .expect("join service")
        .expect("clean service shutdown");
    response(&responses, id).clone()
}

async fn responses_for(output: DuplexStream, ids: &[i64]) -> Vec<Value> {
    let mut reader = BufReader::new(output);
    let mut responses = Vec::new();
    while !ids.iter().all(|id| {
        responses
            .iter()
            .any(|response: &Value| response["id"] == *id)
    }) {
        let mut line = String::new();
        timeout(HANG_GUARD, reader.read_line(&mut line))
            .await
            .expect("response deadline")
            .expect("read response");
        responses.push(serde_json::from_str(&line).expect("JSON-RPC response"));
    }
    responses
}

fn response(responses: &[Value], id: i64) -> &Value {
    responses
        .iter()
        .find(|response| response["id"] == id)
        .expect("expected response ID")
}

fn tool_error(response: &Value) -> Value {
    assert_eq!(response["result"]["isError"], true);
    assert!(response["result"]["structuredContent"].is_null());
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("JSON text tool error");
    serde_json::from_str(text).expect("tool error envelope")
}
