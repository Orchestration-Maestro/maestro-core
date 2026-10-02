//! Real CLI/MCP/evaluation asks inspect trusted prompts through a controlled router.

use super::{
    knowledge_get::knowledge_search::published_identifier_source,
    knowledge_prepare_v2::identity,
    support::{Home, Running, initialize_mcp, local, make_safe_preferences_path},
};
use maestro_kernel::{
    artifact::Store,
    gateway::{
        ModelCard, Role, RouterEntry,
        card_v2::{Capability, Dimensions, EmbeddingFormat, Sampling, SamplingParameters},
    },
    model::NewModelCard,
};
use maestro_knowledge::answer::{Answer, LanguageCheck};
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead as _, BufReader, Read as _, Write as _},
    net::{TcpListener, TcpStream},
    num::NonZeroU32,
    process::Command,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

const QUESTION: &str = "Combien de documents?";
const QUOTE: &str = "Run `maestro search --force` to restore the archive.";

/// Register the existing synthetic answerer fixture in a real published collection.
fn prepared() -> Home {
    let home = Home::new();
    published_identifier_source(&home);
    let database = home.database();
    let model = database
        .put(b"answerer weights", "application/octet-stream")
        .unwrap();
    let qualification = database.put(b"{}", "application/json").unwrap();
    let mut answerer = identity(&model, qualification);
    answerer.role = Role::Answerer;
    answerer.router_entry = RouterEntry::parse("answer").unwrap();
    answerer.invocation.limits.output_tokens = NonZeroU32::new(1024);
    answerer.invocation.dimensions = Dimensions::NotApplicable;
    answerer.invocation.sampling = Sampling::Configured(SamplingParameters {
        temperature: 0.1,
        top_p: 0.9,
        top_k: 40,
        min_p: 0.0,
        typical_p: 1.0,
        repeat_penalty: 1.0,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        seed: Some(1),
    });
    answerer.invocation.reasoning = Capability::Unsupported;
    answerer.formats.embedding = EmbeddingFormat::NotApplicable;
    answerer.formats.document = Capability::NotApplicable;
    answerer.formats.query = Capability::NotApplicable;
    answerer.formats.tool_format = Capability::Unsupported;
    let answerer =
        ModelCard::record_v2(&Store::new(home.data().join("artifacts")), &answerer).unwrap();
    database
        .record_model_card(
            &local(&database),
            &NewModelCard {
                collection_id: "synthetic",
                card: &answerer,
            },
        )
        .unwrap();
    home
}

/// Serve props and one completion, returning the actual trusted chat request.
fn router(reply: &'static str) -> (String, Receiver<Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        for stream in listener.incoming().take(2).flatten() {
            let (path, body, mut stream) = read_request(stream);
            let response = if path.ends_with("/props") {
                json!({"build_info": "test-build"})
            } else {
                assert!(path.ends_with("/v1/chat/completions"), "{path}");
                sender.send(body).unwrap();
                json!({"choices": [{"index": 0, "finish_reason": "stop",
                    "message": {"role": "assistant", "content": reply}}]})
            }
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                Content-Length: {}\r\nConnection: close\r\n\r\n{response}",
                response.len()
            )
            .unwrap();
            if !path.ends_with("/props") {
                break;
            }
        }
    });
    (url, receiver)
}

/// Read the complete HTTP body, not merely the first socket fragment.
fn read_request(stream: TcpStream) -> (String, Value, TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let path = line.split_whitespace().nth(1).unwrap().to_owned();
    let mut length = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        let Some((name, value)) = header.trim_end().split_once(':') else {
            break;
        };
        if name.eq_ignore_ascii_case("content-length") {
            length = value.trim().parse().unwrap();
        }
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (path, body, reader.into_inner())
}

/// Fixed search knobs avoid external services without bypassing the evidence validator.
fn command(home: &Home, url: &str, args: &[&str]) -> Command {
    let mut command = home.command(args);
    command
        .env("MAESTRO_ROUTER_URL", url)
        .env("MAESTRO_QDRANT_URL", "http://127.0.0.1:1");
    command
}

/// The canonical explicit language and tone appear only in the trusted system message.
fn assert_prompt(chat: &Value, tag: &str, tone: &str) {
    let system = chat["messages"][0]["content"].as_str().unwrap();
    assert!(
        system.contains(&format!("BCP 47 tag is \"{tag}\"")),
        "{system}"
    );
    assert!(!system.contains("in the language of the question"));
    assert!(
        system.contains("logs and documentation stay English"),
        "{system}"
    );
    assert!(system.contains("Keep source quotations and citation identities unchanged"));
    assert!(system.contains("reply exactly NOT_FOUND"));
    assert_eq!(system.contains("Keep the answer brief"), tone == "brief");
    assert_eq!(
        system.contains("Give a detailed answer"),
        tone == "detailed"
    );
    assert_eq!(chat["max_tokens"], 1024);
    assert_eq!(chat["stream"], false);
    let user = chat["messages"][1]["content"].as_str().unwrap();
    assert!(user.contains(QUESTION));
    assert!(user.contains(QUOTE));
}

/// Existing settings flags pin code-only retrieval while leaving answer generation on.
fn conversation_args<'a>(language: &'a str, tone: &'a str) -> Vec<&'a str> {
    vec![
        "--language",
        language,
        "--tone",
        tone,
        "--set",
        "search.routes.dense=false",
        "--set",
        "search.rerank.enabled=false",
        "--set",
        "search.routes.lexical=false",
        "--set",
        "search.routes.identifier=false",
        "--set",
        "search.routes.structured=true",
    ]
}

#[test]
fn cli_and_mcp_pass_canonical_preferences_to_real_ask_without_changing_evidence() {
    let home = prepared();
    for (input, canonical) in [
        ("EN", "en"),
        ("FR-ca", "fr-CA"),
        ("ES-419", "es-419"),
        ("JA", "ja"),
    ] {
        for tone in ["brief", "normal", "detailed"] {
            let (url, captured) =
                router("Run `maestro search --force` to restore the archive. [1]");
            let mut args = conversation_args(input, tone);
            args.extend([
                "--json",
                "knowledge",
                "ask",
                "--collection",
                "synthetic",
                "--model",
                "answer",
                "--question",
                QUESTION,
                "--explain",
            ]);
            let cli = Running::of(command(&home, &url, &args)).finish();
            assert_eq!(cli.code, Some(0), "{cli:?}");
            let answer = cli.json();
            assert!(answer["refusal"].is_null(), "{cli:?}");
            assert_prompt(
                &captured.recv_timeout(Duration::from_secs(2)).unwrap(),
                canonical,
                tone,
            );
            assert_eq!(
                serde_json::from_value::<Answer>(answer.clone())
                    .unwrap()
                    .language_check,
                LanguageCheck::Unchecked
            );
            assert_eq!(answer["lang"], canonical);
            assert_eq!(answer["answer"], format!("{QUOTE} [1]"));
            assert_eq!(answer["uncalibrated"], true);
            assert!(
                cli.stderr.contains("explain: language unchecked"),
                "{cli:?}"
            );
            assert!(!answer.as_object().unwrap().contains_key("language_check"));
            let (url, captured) =
                router("Run `maestro search --force` to restore the archive. [1]");
            let mut args = conversation_args(input, tone);
            args.push("mcp");
            let (server, mut stdin) = Running::with_stdin(command(&home, &url, &args));
            initialize_mcp(&mut stdin);
            writeln!(
                stdin,
                "{}",
                json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
            )
            .unwrap();
            writeln!(
                stdin,
                "{}",
                json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
                    "params": {"name": "knowledge_ask", "arguments": {
                        "collection": "synthetic", "model": "answer", "question": QUESTION}}})
            )
            .unwrap();
            drop(stdin);
            let mcp = server.finish();
            assert_eq!(mcp.code, Some(0), "{mcp:?}");
            assert_prompt(
                &captured.recv_timeout(Duration::from_secs(2)).unwrap(),
                canonical,
                tone,
            );
            let response = mcp
                .stdout
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .find(|value| value["id"] == 2)
                .unwrap();
            assert_eq!(response["result"]["structuredContent"], answer);
        }
    }
}

#[test]
fn eval_real_ask_ignores_user_workspace_and_flag_language_and_tone() {
    let home = prepared();
    let project = home.root().join("project");
    fs::create_dir_all(project.join(".maestro")).unwrap();
    let workspace = project.join(".maestro/config.toml");
    fs::write(
        home.config().join("preferences.toml"),
        "schema = 'maestro-preferences/1'\nlanguage = 'fr'\ntone = 'brief'\n",
    )
    .unwrap();
    fs::write(
        &workspace,
        "schema = 'maestro-preferences/1'\nlanguage = 'es'\ntone = 'detailed'\n",
    )
    .unwrap();
    make_safe_preferences_path(&project.join(".maestro"));
    make_safe_preferences_path(&workspace);
    let suite = project.join("suite.jsonl");
    fs::write(
        &suite,
        json!({"schema": "maestro-suite/1", "id": "q", "language": "fr",
        "question": QUESTION, "answerable": false, "expected": []})
        .to_string(),
    )
    .unwrap();
    let database = home.database();
    let digest = database
        .model_cards(&local(&database), "synthetic", Role::Answerer)
        .unwrap()[0]
        .digest
        .clone();
    let manifest = project.join("ladder.json");
    fs::write(
        &manifest,
        json!({
            "schema": "maestro-ladder-manifest/1", "suite": "suite.jsonl",
            "collection": "synthetic", "warm_ups": 0, "output": "out",
            "rungs": [{"name": "pinned", "configuration": {
                "routes": {"dense": false, "lexical": false,
                    "identifier": false, "structured": true}, "rrf_k": 60,
                "weights": {"dense": 1.0, "lexical": 1.0, "identifier": 1.0, "structured": 1.0},
                "rerank": null, "min_rerank_score": null},
                "ask": {"prompt": "v1", "card": digest.as_str(), "output_tokens": 900}}]
        })
        .to_string(),
    )
    .unwrap();
    for (layer, directory, flags) in [
        ("user", home.root(), Vec::new()),
        ("workspace", project.as_path(), Vec::new()),
        (
            "flag",
            project.as_path(),
            vec!["--language", "ja", "--tone", "brief"],
        ),
    ] {
        let mut manifest_value: Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        manifest_value["output"] = json!(format!("out-{layer}"));
        fs::write(&manifest, manifest_value.to_string()).unwrap();
        let (url, captured) = router("NOT_FOUND");
        let mut args = flags;
        args.extend([
            "--json",
            "eval",
            "ladder",
            "--manifest",
            manifest.to_str().unwrap(),
        ]);
        let mut run = command(&home, &url, &args);
        run.current_dir(directory);
        let result = Running::of(run).finish();
        assert_eq!(result.code, Some(0), "{layer}: {result:?}");
        let chat = captured.recv_timeout(Duration::from_secs(2)).unwrap();
        let system = chat["messages"][0]["content"].as_str().unwrap();
        assert!(
            system.starts_with("Answer in the language of the question using only"),
            "{layer}: {system}"
        );
        assert!(!system.contains("BCP 47"));
        assert!(!system.contains("Keep the answer brief"));
        assert!(!system.contains("Give a detailed answer"));
        assert_eq!(chat["max_tokens"], 900);
        let report: Value = serde_json::from_slice(
            &fs::read(project.join(format!("out-{layer}/pinned.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(report["ask_settings"]["prompt"], "v1", "{report}");
    }
}
