//! Each tool call opens its `gen_ai.execute_tool` span, which names a listed
//! tool alone and records how the call ended.

use super::{
    knowledge_server::KnowledgeServer,
    response::tool_error,
    tests::{HANG_GUARD, handshake_and_tool_call, serve_one_call},
};
use crate::{
    knowledge::RefreshScratch,
    mcp::outcome::{call_outcome, tool_name},
};
use maestro_kernel::telemetry::stage::Outcome;
use rmcp::{ErrorData as McpError, model::CallToolResponse};
use serde_json::{Value, json};
use std::{
    fmt,
    sync::{LazyLock, Mutex},
};
use tracing::{
    Dispatch, Event, Metadata, Subscriber, dispatcher,
    field::{Field, Visit},
    span::{Attributes, Id, Record},
    subscriber::NoSubscriber,
};

/// A dispatcher that stays registered for the life of the test process:
/// with two registered, `tracing` never caches a callsite as unwanted
/// because another test's thread, which has no subscriber, reached it first.
static SECOND_DISPATCHER: LazyLock<Dispatch> =
    LazyLock::new(|| Dispatch::new(NoSubscriber::default()));

/// Every span opened while it is the default: its name, then each field
/// given a value, as `name=value`.
#[derive(Debug, Default)]
struct Calls(Mutex<Vec<(&'static str, Vec<String>)>>);

/// Collects fields as `name=value`.
struct Values<'a>(&'a mut Vec<String>);

impl Visit for Values<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.push(format!("{}={value:?}", field.name()));
    }
}

impl Subscriber for Calls {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &Attributes<'_>) -> Id {
        let mut values = Vec::new();
        span.record(&mut Values(&mut values));
        let mut calls = self.0.lock().unwrap();
        calls.push((span.metadata().name(), values));
        Id::from_u64(calls.len().try_into().unwrap())
    }

    fn record(&self, span: &Id, record: &Record<'_>) {
        let mut calls = self.0.lock().unwrap();
        let index = usize::try_from(span.into_u64() - 1).unwrap();
        record.record(&mut Values(&mut calls[index].1));
    }

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, _event: &Event<'_>) {}

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

/// Serves one call of `tool` with `arguments` on a fresh kernel, and returns
/// the tool name and outcome of each tool-call span it opened.
async fn traced_call(tool: &str, arguments: &Value) -> Vec<(String, String)> {
    traced_values(tool, arguments)
        .await
        .into_iter()
        .map(|values| {
            let value = |field: &str| {
                let prefix = format!("{field}=");
                let found = values.iter().find_map(|value| value.strip_prefix(&prefix));
                found.unwrap().to_owned()
            };
            assert!(values.iter().any(|value| value.starts_with("duration_us=")));
            (value("gen_ai.tool.name"), value("outcome"))
        })
        .collect()
}

/// Serves one call of `tool` with `arguments` on a fresh kernel, and returns
/// every value each tool-call span it opened recorded, as `name=value`.
async fn traced_values(tool: &str, arguments: &Value) -> Vec<Vec<String>> {
    LazyLock::force(&SECOND_DISPATCHER);
    let dispatch = Dispatch::new(Calls::default());
    let _default = dispatcher::set_default(&dispatch);
    let scratch = RefreshScratch::new();
    let server = KnowledgeServer::with_kernel_opener(move || scratch.kernel(None), HANG_GUARD);
    serve_one_call(server, handshake_and_tool_call(2, tool, arguments), 2).await;
    let calls = dispatch.downcast_ref::<Calls>().unwrap().0.lock().unwrap();
    calls
        .iter()
        .filter(|(name, _)| *name == "gen_ai.execute_tool")
        .map(|(_, values)| values.clone())
        .collect()
}

#[tokio::test]
async fn a_tool_call_that_answers_records_its_tool_and_ok() {
    let calls = traced_call("knowledge_collections", &json!({})).await;
    assert_eq!(
        calls,
        [(
            r#""knowledge_collections""#.to_owned(),
            r#""ok""#.to_owned()
        )]
    );
}

#[tokio::test]
async fn a_call_with_refused_arguments_records_refused() {
    let calls = traced_call("knowledge_get", &json!({"collection": "c"})).await;
    assert_eq!(
        calls,
        [(r#""knowledge_get""#.to_owned(), r#""refused""#.to_owned())]
    );
}

#[tokio::test]
async fn an_ask_records_its_tool_and_outcome_never_its_question() {
    let question = "Which canary phrase opens the vault?";
    let arguments = json!({"collection": "absent", "question": question});
    let calls = traced_values("knowledge_ask", &arguments).await;

    let [values] = calls.as_slice() else {
        panic!("one tool call, not {calls:?}");
    };
    assert!(values.contains(&r#"gen_ai.tool.name="knowledge_ask""#.to_owned()));
    assert!(values.contains(&r#"outcome="refused""#.to_owned()));
    assert!(values.iter().all(|value| !value.contains(question)));
}

#[tokio::test]
async fn a_call_of_an_unlisted_tool_names_no_client_text() {
    let calls = traced_call("what is the root password", &json!({})).await;
    assert_eq!(
        calls,
        [(r#""unknown""#.to_owned(), r#""refused""#.to_owned())]
    );
}

#[test]
fn a_listed_tool_keeps_its_name_and_any_other_is_unknown() {
    assert_eq!(tool_name("knowledge_collections"), "knowledge_collections");
    assert_eq!(tool_name("knowledge_get"), "knowledge_get");
    assert_eq!(tool_name("knowledge_search"), "knowledge_search");
    assert_eq!(tool_name("knowledge_ask"), "knowledge_ask");
    assert_eq!(tool_name("knowledge_delete"), "unknown");
}

#[test]
fn each_error_code_ends_its_call_with_its_outcome() {
    let codes = [
        ("busy", Outcome::Unavailable),
        ("kernel_unavailable", Outcome::Unavailable),
        ("answerer_unavailable", Outcome::Unavailable),
        ("search_failed", Outcome::Error),
        ("deadline_exceeded", Outcome::Timeout),
        ("cancelled", Outcome::Error),
        ("integrity_error", Outcome::Error),
        ("not_found", Outcome::Refused),
        ("response_too_large", Outcome::Refused),
    ];
    for (code, outcome) in codes {
        let response = Ok(CallToolResponse::Complete(tool_error(
            code,
            "message",
            false,
            Vec::new(),
        )));
        assert_eq!(call_outcome(&response), outcome, "{code}");
    }
    let failed: Result<CallToolResponse, McpError> =
        Err(McpError::internal_error("knowledge operation failed", None));
    assert_eq!(call_outcome(&failed), Outcome::Error);
}
