use super::{
    HANG_GUARD, ServerHome, cancel_and_tool_call, handshake_and_tool_call, response, responses_for,
    tool_error,
};
use crate::{kernel::Kernel, mcp::transport::BoundedStdio};
use rmcp::ServiceExt;
use std::{
    sync::{Arc, Condvar, Mutex, mpsc},
    time::Duration,
};
use tokio::{
    io::{AsyncWriteExt as _, duplex},
    spawn,
    task::spawn_blocking,
    time::timeout,
};

#[tokio::test]
async fn cancelling_four_searches_cannot_free_the_worker_cap() {
    let home = ServerHome::new();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let (started_tx, started_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let server = home.server_with_gate(
        release.clone(),
        started_tx,
        finished_tx,
        Duration::from_millis(200),
    );
    let (server_input, mut client_input) = duplex(4096);
    let (server_output, client_output) = duplex(32_768);
    let serving = spawn(server.serve(BoundedStdio::new(server_input, server_output)));
    client_input
        .write_all(&handshake_and_search_call(2))
        .await
        .expect("write initial search call");
    let service = timeout(HANG_GUARD, serving)
        .await
        .expect("MCP handshake deadline")
        .expect("join service handshake")
        .expect("start test service");
    let waiting = spawn(service.waiting());

    let mut started_rx = started_rx;
    for id in 2..=5 {
        let (started, next_rx) = spawn_blocking(move || {
            let started = started_rx.recv_timeout(HANG_GUARD);
            (started, started_rx)
        })
        .await
        .expect("join opener wait");
        started.expect("search worker started");
        started_rx = next_rx;
        client_input
            .write_all(&cancel_and_tool_call(
                id,
                id + 1,
                "knowledge_search",
                &serde_json::json!({"collection": "collection", "query": "question"}),
            ))
            .await
            .expect("write cancel and next search");
    }

    let responses = responses_for(client_output, &[6]).await;
    let code = tool_error(response(&responses, 6))["error"]["code"]
        .as_str()
        .expect("tool error code")
        .to_owned();
    let extra_workers = usize::from(started_rx.try_recv().is_ok());
    spawn_blocking({
        let release = release.clone();
        move || {
            let (released, wake) = &*release;
            *released.lock().expect("lock opener gate") = true;
            wake.notify_all();
        }
    })
    .await
    .expect("release blocked search workers");
    let mut finished_rx = finished_rx;
    for _ in 0..(4 + extra_workers) {
        let (finished, next_rx) = spawn_blocking(move || {
            let finished = finished_rx.recv_timeout(HANG_GUARD);
            (finished, finished_rx)
        })
        .await
        .expect("join kernel-open wait");
        finished.expect("cancelled search worker finished");
        finished_rx = next_rx;
    }
    assert_eq!(code, "busy");
    drop(client_input);
    timeout(HANG_GUARD, waiting)
        .await
        .expect("service shutdown deadline")
        .expect("join service")
        .expect("clean service shutdown");
}

impl ServerHome {
    fn server_with_gate(
        &self,
        release: Arc<(Mutex<bool>, Condvar)>,
        started: mpsc::Sender<()>,
        finished: mpsc::Sender<()>,
        deadline: Duration,
    ) -> super::KnowledgeServer {
        let data = self.0.join("data");
        let config = self.0.join("config");
        super::KnowledgeServer::with_kernel_opener(
            move || {
                started.send(()).expect("signal blocked opener");
                let (released, wake) = &*release;
                let mut released = released.lock().expect("lock opener gate");
                while !*released {
                    released = wake.wait(released).expect("wait for opener gate");
                }
                let result = Kernel::open_at(&data, &config);
                let _ = finished.send(());
                result
            },
            deadline,
        )
    }
}

fn handshake_and_search_call(id: i64) -> Vec<u8> {
    handshake_and_tool_call(
        id,
        "knowledge_search",
        &serde_json::json!({"collection": "collection", "query": "question"}),
    )
}
