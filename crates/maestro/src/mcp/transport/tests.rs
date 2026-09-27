use super::*;
use std::{future::Future, time::Duration};
use tokio::{
    io::duplex,
    runtime::{Builder, Runtime},
    time::timeout,
};

/// A bound that only stops a hung test; it is generous so a loaded
/// machine cannot fail a correct run.
const HANG_GUARD: Duration = Duration::from_secs(30);

fn run_bounded(runtime: &Runtime, future: impl Future<Output = ()>) {
    runtime.block_on(async {
        timeout(Duration::from_secs(15), future)
            .await
            .expect("bounded transport test");
    });
}

#[test]
fn input_line_limit_includes_its_newline() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    run_bounded(&runtime, async {
        let valid = br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let (server_input, mut client_input) = duplex(INPUT_LIMIT_BYTES + 2);
        let (server_output, _client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        let mut frame = valid.to_vec();
        frame.resize(INPUT_LIMIT_BYTES - 1, b' ');
        let expected = frame.clone();
        frame.push(b'\n');
        client_input
            .write_all(&frame)
            .await
            .expect("write exact line");
        drop(client_input);
        assert_eq!(transport.line().await, Some(expected));
        drop(transport);

        let (server_input, mut client_input) = duplex(INPUT_LIMIT_BYTES + 1);
        let (server_output, _client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        let final_frame = vec![b'x'; INPUT_LIMIT_BYTES];
        client_input
            .write_all(&final_frame)
            .await
            .expect("write final frame at limit");
        drop(client_input);
        assert_eq!(
            timeout(HANG_GUARD, transport.line())
                .await
                .expect("final frame is bounded"),
            Some(final_frame)
        );
        drop(transport);

        let (server_input, mut client_input) = duplex(8);
        let (server_output, _client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        client_input
            .write_all(b"frame\r\n")
            .await
            .expect("write CRLF frame");
        drop(client_input);
        assert_eq!(transport.line().await, Some(b"frame".to_vec()));
        drop(transport);

        let (server_input, mut client_input) = duplex(INPUT_LIMIT_BYTES + 2);
        let (server_output, mut client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        let mut frame = valid.to_vec();
        frame.resize(INPUT_LIMIT_BYTES, b' ');
        frame.push(b'\n');
        client_input
            .write_all(&frame)
            .await
            .expect("write oversized line");
        drop(client_input);
        assert!(transport.line().await.is_none());
        transport.close().await.expect("close transport");
        drop(transport);
        let mut response = Vec::new();
        client_output
            .read_to_end(&mut response)
            .await
            .expect("read refusal");
        let response: Value = serde_json::from_slice(&response).expect("valid refusal");
        assert_eq!(response["id"], Value::Null);
        assert_eq!(response["error"]["code"], -32600);
    });
}

#[test]
fn dropping_receive_preserves_oversized_line_discard_state() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    run_bounded(&runtime, async {
        let oversized = vec![b' '; INPUT_LIMIT_BYTES + 1024];
        let (server_input, mut client_input) = duplex(oversized.len() + 1024);
        let (server_output, mut client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        client_input
            .write_all(&oversized)
            .await
            .expect("write oversized line prefix");
        assert!(
            timeout(Duration::from_millis(10), transport.receive())
                .await
                .is_err()
        );

        client_input
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
            .await
            .expect("write following notification");
        drop(client_input);
        assert!(transport.receive().await.is_none());
        transport.close().await.expect("close transport");
        drop(transport);

        let mut response = Vec::new();
        client_output
            .read_to_end(&mut response)
            .await
            .expect("read oversized-line refusal");
        let responses = response
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice::<Value>(line).expect("valid response"))
            .collect::<Vec<_>>();
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0]["id"], Value::Null);
        assert_eq!(responses[0]["error"]["code"], -32600);
    });
}

#[test]
fn blank_and_bom_prefixed_frames_are_skipped() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    run_bounded(&runtime, async {
        let mut input = b"\n \t\r\n\xEF\xBB\xBF".to_vec();
        input.extend_from_slice(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        input.push(b'\n');
        let (server_input, mut client_input) = duplex(input.len() + 1);
        let (server_output, _client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        client_input.write_all(&input).await.expect("write frames");
        drop(client_input);
        assert!(matches!(
            transport.receive().await,
            Some(JsonRpcMessage::Notification(_))
        ));
    });
}

#[test]
fn oversized_line_is_refused_and_the_next_frame_is_received() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    run_bounded(&runtime, async {
        let mut input = vec![b'x'; INPUT_LIMIT_BYTES + 1];
        input.extend_from_slice(
            b"\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        );
        let (server_input, mut client_input) = duplex(INPUT_LIMIT_BYTES * 2);
        let (server_output, mut client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        client_input.write_all(&input).await.expect("write frames");
        drop(client_input);
        assert!(matches!(
            transport.receive().await,
            Some(JsonRpcMessage::Notification(_))
        ));
        transport.close().await.expect("close transport");
        drop(transport);
        let mut response = Vec::new();
        client_output
            .read_to_end(&mut response)
            .await
            .expect("read refusal");
        let response: Value = serde_json::from_slice(&response).expect("valid refusal");
        assert_eq!(response["error"]["code"], -32600);
    });
}

#[test]
fn queued_refusal_flushes_after_receive_is_cancelled() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    run_bounded(&runtime, async {
        let (server_input, mut client_input) = duplex(16);
        let (server_output, mut client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        client_input
            .write_all(b"{\n")
            .await
            .expect("write malformed request");
        assert!(
            timeout(Duration::from_millis(25), transport.receive())
                .await
                .is_err()
        );

        let mut response = Vec::new();
        let mut byte = [0_u8; 1];
        loop {
            let read = timeout(HANG_GUARD, client_output.read(&mut byte))
                .await
                .expect("queued refusal flushes without another message or close")
                .expect("read refusal");
            assert_ne!(read, 0, "transport closed before sending refusal");
            response.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        let response: Value = serde_json::from_slice(&response).expect("valid refusal");
        assert_eq!(response["error"]["code"], -32700);

        transport.close().await.expect("close transport");
        drop(transport);
        let mut trailing = Vec::new();
        client_output
            .read_to_end(&mut trailing)
            .await
            .expect("read remaining output");
        assert!(trailing.is_empty(), "refusal was written more than once");
    });
}

#[test]
fn close_flushes_errors_already_queued() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    run_bounded(&runtime, async {
        let (server_input, _client_input) = duplex(16);
        let (server_output, mut client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        let mut queued =
            br#"{"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"queued"}}"#.to_vec();
        queued.push(b'\n');
        transport
            .pending_errors
            .lock()
            .expect("queued error lock")
            .push_back(queued.clone());

        transport.close().await.expect("close flushes queued error");
        drop(transport);
        let mut written = Vec::new();
        client_output
            .read_to_end(&mut written)
            .await
            .expect("read queued error");
        assert_eq!(written, queued);
    });
}

#[test]
fn malformed_json_uses_parse_error_and_does_not_drop_the_next_frame() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    run_bounded(&runtime, async {
        let input =
            b"{\"jsonrpc\":\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n";
        let (server_input, mut client_input) = duplex(input.len() + 1);
        let (server_output, mut client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        client_input.write_all(input).await.expect("write frames");
        drop(client_input);
        assert!(matches!(
            transport.receive().await,
            Some(JsonRpcMessage::Notification(_))
        ));
        transport.close().await.expect("close transport");
        drop(transport);
        let mut response = Vec::new();
        client_output
            .read_to_end(&mut response)
            .await
            .expect("read parse error");
        let response: Value = serde_json::from_slice(&response).expect("valid parse error");
        assert_eq!(response["error"]["code"], -32700);
    });
}

#[test]
fn string_request_ids_are_limited_by_utf8_bytes() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    run_bounded(&runtime, async {
        let valid_id = "é".repeat(128);
        let mut valid = serde_json::to_vec(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": valid_id,
            "method": "ping",
        }))
        .expect("valid request");
        valid.push(b'\n');
        let (server_input, mut client_input) = duplex(valid.len() + 1);
        let (server_output, _client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        client_input
            .write_all(&valid)
            .await
            .expect("write 256-byte ID");
        drop(client_input);
        assert!(matches!(
            transport.receive().await,
            Some(JsonRpcMessage::Request(_))
        ));
        drop(transport);

        let invalid_id = format!("{}a", "é".repeat(128));
        assert_eq!(invalid_id.len(), REQUEST_ID_LIMIT_BYTES + 1);
        let mut invalid = serde_json::to_vec(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": invalid_id,
            "method": "ping",
        }))
        .expect("oversized ID request");
        invalid.push(b'\n');
        let (server_input, mut client_input) = duplex(invalid.len() + 1);
        let (server_output, mut client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        client_input
            .write_all(&invalid)
            .await
            .expect("write 257-byte ID");
        drop(client_input);
        assert!(transport.receive().await.is_none());
        transport.close().await.expect("close transport");
        drop(transport);
        let mut response = Vec::new();
        client_output
            .read_to_end(&mut response)
            .await
            .expect("read refusal");
        let response: Value = serde_json::from_slice(&response).expect("valid refusal");
        assert_eq!(response["id"], Value::Null);
        assert_eq!(response["error"]["code"], -32600);
    });
}

#[test]
fn invalid_request_objects_use_the_invalid_request_code() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    run_bounded(&runtime, async {
        let mut input = br#"{"jsonrpc":"2.0","id":1,"method":null}"#.to_vec();
        input.push(b'\n');
        input.extend_from_slice(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
        input.push(b'\n');
        let (server_input, mut client_input) = duplex(input.len() + 1);
        let (server_output, mut client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        client_input.write_all(&input).await.expect("write frames");
        drop(client_input);
        assert!(matches!(
            transport.receive().await,
            Some(JsonRpcMessage::Notification(_))
        ));
        transport.close().await.expect("close transport");
        drop(transport);
        let mut response = Vec::new();
        client_output
            .read_to_end(&mut response)
            .await
            .expect("read refusal");
        let response: Value = serde_json::from_slice(&response).expect("valid refusal");
        assert_eq!(response["error"]["code"], -32600);
    });
}

#[test]
fn overlong_unterminated_input_is_refused_without_echoing_it() {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    run_bounded(&runtime, async {
        let (server_input, mut client_input) = duplex(INPUT_LIMIT_BYTES + 1);
        let (server_output, mut client_output) = duplex(1024);
        let mut transport = BoundedStdio::new(server_input, server_output);
        client_input
            .write_all(&vec![b'x'; INPUT_LIMIT_BYTES + 1])
            .await
            .expect("write oversized input");
        drop(client_input);
        assert!(transport.line().await.is_none());
        transport.close().await.expect("close transport");
        drop(transport);

        let mut response = Vec::new();
        client_output
            .read_to_end(&mut response)
            .await
            .expect("read refusal");
        let response: Value = serde_json::from_slice(&response).expect("valid refusal");
        assert_eq!(response["id"], Value::Null);
        assert_eq!(response["error"]["code"], -32600);
        assert!(
            serde_json::to_vec(&response)
                .expect("serialized refusal")
                .len()
                < RESPONSE_LIMIT_BYTES
        );
    });
}
