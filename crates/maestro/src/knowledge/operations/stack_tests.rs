//! Search fits half the stack of a Windows main thread.
//!
//! The CLI and the MCP server poll search on the main thread, which has
//! 1 MiB of stack on Windows, and 8 MiB on Linux.

use super::{
    KnowledgeError, SearchCards, ask::tests::register_reasoning_answerer, local_search_context,
    search_with, tests::Scratch,
};
use crate::{knowledge::SearchRequest, settings::KnowledgeSettings};
use maestro_kernel::{
    evidence::{RequestBudget, RouteStatus},
    gateway::{RouterClient, Url},
};
use maestro_knowledge::{
    index::Qdrant,
    search::{
        IntentExpansion, IntentTrigger, SearchConfiguration, SearchRequest as PipelineRequest,
        search,
    },
};
use std::thread;
use tokio::runtime::Builder;

/// Half a Windows main thread's stack. A debug build once needed close to
/// 1 MiB here on Linux, and more on Windows, for route futures copied
/// through the frames of the futures that join them.
const SEARCH_STACK: usize = 512 * 1024;

#[test]
fn a_search_runs_within_half_a_windows_main_thread_stack() {
    let scratch = Scratch::new();
    let kernel = scratch.kernel(None).expect("open test kernel");
    let searched = thread::Builder::new()
        .stack_size(SEARCH_STACK)
        .spawn(move || {
            let request = SearchRequest::parse(serde_json::json!({
                "collection": "collection",
                "query": "question",
            }))
            .expect("valid search request");
            let model_port =
                RouterClient::new(Url::parse("http://127.0.0.1:1").expect("router URL"))
                    .expect("router client");
            let qdrant = Qdrant::new("http://127.0.0.1:1").expect("Qdrant client");
            let runtime = Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("search runtime");
            runtime
                .block_on(search_with(
                    kernel,
                    &request,
                    &KnowledgeSettings::default(),
                    &model_port,
                    &qdrant,
                ))
                .map(|_| ())
        })
        .expect("spawn the bounded search thread")
        .join()
        .expect("search within the bounded stack");

    // Every route ran, and failed to reach the absent search service.
    assert!(matches!(
        searched,
        Err(KnowledgeError::Failed {
            code: "search_failed",
            ..
        })
    ));
}

#[test]
fn a_search_that_expands_runs_within_half_a_windows_main_thread_stack() {
    for trigger in [
        IntentTrigger::Always,
        IntentTrigger::LowConfidence {
            min_top_rerank: 2.0,
        },
    ] {
        let scratch = Scratch::new();
        let kernel = scratch.kernel(None).expect("open test kernel");
        let (_, answerer) = register_reasoning_answerer(&kernel, b"intent", false);
        let expanded = thread::Builder::new()
            .stack_size(SEARCH_STACK)
            .spawn(move || {
                let model_port =
                    RouterClient::new(Url::parse("http://127.0.0.1:1").expect("router URL"))
                        .expect("router client");
                let qdrant = Qdrant::new("http://127.0.0.1:1").expect("Qdrant client");
                let context = local_search_context(
                    &kernel.database,
                    &qdrant,
                    &model_port,
                    SearchCards {
                        intent: Some(&answerer),
                        ..SearchCards::default()
                    },
                );
                let request = PipelineRequest {
                    configuration: SearchConfiguration {
                        intent_expansion: IntentExpansion::Hyde,
                        intent_trigger: trigger,
                        ..SearchConfiguration::default()
                    },
                    ..PipelineRequest::new("collection", "question", None, RequestBudget::default())
                };
                let runtime = Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("search runtime");
                runtime
                    .block_on(search(&context, &request))
                    .map(|input| input.routes.get("intent_expansion").cloned())
            })
            .expect("spawn the bounded search thread")
            .join()
            .expect("search within the bounded stack");

        // The expansion ran, and failed to reach the absent router.
        assert!(
            matches!(
                &expanded,
                Ok(Some(RouteStatus::Unavailable(reason))) if reason == "intent_model_unavailable"
            ),
            "{trigger:?}: {expanded:?}"
        );
    }
}
