//! A scripted OpenAI-compatible model endpoint on the loopback interface. The
//! hosts under probe use it instead of an account: it answers each chat
//! request with the first planned tool call that the request offers and has
//! not made yet, then with a plain final answer, and it keeps every request
//! body so a probe can read the exact prompt and tools a host sent.

use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
};

/// The model identifier every probe configures.
pub(super) const MODEL: &str = "probe-model";

/// One planned tool call: the tool, its arguments, and the text the request's
/// system prompt must hold for the call to be made (so that a parent session
/// does not make its child's call).
pub(super) struct Step {
    /// The tool name as the host offers it.
    pub(super) tool: &'static str,
    /// The call's arguments.
    pub(super) arguments: Value,
    /// Text the system prompt must contain, if any.
    pub(super) when: Option<&'static str>,
}

/// A running endpoint and the request bodies it received.
pub(super) struct Provider {
    port: u16,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl Provider {
    /// Starts the endpoint with `plan` on a free loopback port.
    pub(super) fn start(plan: Vec<Step>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let plan = Arc::new(plan);
        let received = Arc::clone(&requests);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let plan = Arc::clone(&plan);
                let received = Arc::clone(&received);
                thread::spawn(move || serve(stream, &plan, &received));
            }
        });
        Self { port, requests }
    }

    /// The OpenAI-compatible base URL, ending in `/v1`.
    pub(super) fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }

    /// Every chat request body received so far, in arrival order.
    pub(super) fn requests(&self) -> Vec<Value> {
        self.requests.lock().unwrap().clone()
    }
}

fn serve(stream: TcpStream, plan: &[Step], received: &Mutex<Vec<Value>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    let mut length = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        if header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    if request_line.starts_with("GET") {
        let models = json!({"object": "list", "data": [{"id": MODEL, "object": "model"}]});
        return respond(stream, "application/json", &models.to_string());
    }
    let request = serde_json::from_slice::<Value>(&body).unwrap();
    received.lock().unwrap().push(request.clone());
    let (message, finish) = reply(plan, &request);
    if request["stream"] == true {
        let mut delta = message;
        if let Some(call) = delta["tool_calls"].get_mut(0) {
            call["index"] = json!(0);
        }
        let events = [
            json!({"id": "probe", "object": "chat.completion.chunk", "model": MODEL,
                "choices": [{"index": 0, "delta": delta, "finish_reason": null}]}),
            json!({"id": "probe", "object": "chat.completion.chunk", "model": MODEL,
                "choices": [{"index": 0, "delta": {}, "finish_reason": finish}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}}),
        ];
        let text = events
            .iter()
            .map(|event| format!("data: {event}\n\n"))
            .chain(["data: [DONE]\n\n".to_owned()])
            .collect::<String>();
        respond(stream, "text/event-stream", &text);
    } else {
        let completion = json!({"id": "probe", "object": "chat.completion", "model": MODEL,
            "choices": [{"index": 0, "message": message, "finish_reason": finish}],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}});
        respond(stream, "application/json", &completion.to_string());
    }
}

fn respond(mut stream: TcpStream, kind: &str, body: &str) {
    let head = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: {kind}\r\ncontent-length: {}\r\n\
         connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).unwrap();
    stream.write_all(body.as_bytes()).unwrap();
}

/// The assistant message for `request` and its finish reason: the first
/// planned call the request offers, whose condition holds and which the
/// conversation has not made yet, or else the final answer.
fn reply(plan: &[Step], request: &Value) -> (Value, &'static str) {
    let offered = offered_tools(request);
    let system = system_prompt(request);
    let made = request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|message| message["tool_calls"].as_array().into_iter().flatten())
        .filter_map(|call| call["function"]["name"].as_str())
        .collect::<Vec<_>>();
    let step = plan.iter().find(|step| {
        offered.iter().any(|tool| tool == step.tool)
            && !made.contains(&step.tool)
            && step.when.is_none_or(|text| system.contains(text))
    });
    match step {
        Some(step) => (
            json!({"role": "assistant", "content": null, "tool_calls": [{
                "id": format!("call-{}", step.tool), "type": "function",
                "function": {"name": step.tool, "arguments": step.arguments.to_string()}
            }]}),
            "tool_calls",
        ),
        None => (
            json!({"role": "assistant", "content": "PROBE-DONE"}),
            "stop",
        ),
    }
}

/// The tool names a chat request offers, in the host's order.
pub(super) fn offered_tools(request: &Value) -> Vec<String> {
    request["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_owned))
        .collect()
}

/// The request's system prompt, joined when a host sends it in parts.
pub(super) fn system_prompt(request: &Value) -> String {
    messages_text(request, "system")
}

/// Every tool result the request carries back to the model, joined.
pub(super) fn tool_results(request: &Value) -> String {
    messages_text(request, "tool")
}

fn messages_text(request: &Value, role: &str) -> String {
    request["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|message| message["role"] == role)
        .map(|message| match &message["content"] {
            Value::String(text) => text.clone(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
