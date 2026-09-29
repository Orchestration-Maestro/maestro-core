//! Search fits half the stack of a Windows main thread.
//!
//! The CLI and the MCP server poll search on the main thread, which has
//! 1 MiB of stack on Windows, and 8 MiB on Linux.

use super::{KnowledgeError, search_with, tests::Scratch};
use crate::knowledge::SearchRequest;
use maestro_kernel::gateway::{RouterClient, Url};
use maestro_knowledge::index::Qdrant;
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
                .block_on(search_with(kernel, &request, &model_port, &qdrant))
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
