//! Registration and listing of scoped model cards through the public CLI.

use super::support::{Home, synthetic};
use maestro_kernel::gateway::{
    Role, RouterEntry,
    card_v2::{Capability, CardIdentity, Dimensions, EmbeddingFormat, QualificationMethod},
};
use serde_json::json;
use std::{fs, io::BufReader, net::TcpStream, path::PathBuf};

#[test]
fn register_is_idempotent_and_list_shows_the_card_in_text_and_json() {
    let home = Home::new();
    home.add_synthetic();
    let card = card_file(&home, false);
    let path = card.to_str().unwrap();

    let registered = home.run(&[
        "--json",
        "model",
        "register",
        "--collection",
        "synthetic",
        "--card",
        path,
    ]);
    assert_eq!(
        (registered.code, registered.stderr.as_str()),
        (Some(0), ""),
        "{registered:?}"
    );
    let registration = registered.json();
    let digest = registration["digest"].as_str().unwrap().to_owned();
    assert_eq!(digest.len(), 64, "{registered:?}");
    assert_eq!(registration["schema"], "maestro-cli/model-register/1");

    let again = home.run(&[
        "model",
        "register",
        "--collection",
        "synthetic",
        "--card",
        path,
    ]);
    assert_eq!(again.code, Some(0), "{again:?}");
    assert!(again.stdout.contains(&digest), "{again:?}");

    let listed = home.run(&["--json", "model", "list", "--collection", "synthetic"]);
    assert_eq!(listed.code, Some(0), "{listed:?}");
    assert_eq!(
        listed.json(),
        json!({
            "schema": "maestro-cli/model-list/1",
            "collection": "synthetic",
            "cards": [{"digest": digest, "role": "embedder"}],
            "evaluations": [],
            "selections": [],
        })
    );

    let text = home.run(&["model", "list", "--collection", "synthetic"]);
    assert_eq!(text.code, Some(0), "{text:?}");
    assert!(
        text.stdout.contains(&digest) && text.stdout.contains("embedder"),
        "{text:?}"
    );
}

#[test]
fn a_passing_reranker_health_gate_allows_exact_card_selection() {
    let home = Home::new();
    home.add_synthetic();
    let card = card_file(&home, true);
    let registered = home.run(&[
        "--json",
        "model",
        "register",
        "--collection",
        "synthetic",
        "--card",
        card.to_str().unwrap(),
    ]);
    let digest = registered.json()["digest"].as_str().unwrap().to_owned();
    let router = StubRouter::serve([0.9, 0.1]);
    let mut command = home.command(&[
        "--json",
        "model",
        "check",
        "--collection",
        "synthetic",
        "--digest",
        &digest,
    ]);
    command.env("MAESTRO_ROUTER_URL", &router.url);
    let checked = super::support::Running::of(command).finish();
    assert_eq!(checked.code, Some(0), "{checked:?}");
    assert_eq!(checked.json()["disposition"], "eligible");

    let selected = home.run(&[
        "--json",
        "model",
        "select",
        "--collection",
        "synthetic",
        "--role",
        "reranker",
        "--digest",
        &digest,
    ]);
    assert_eq!(selected.code, Some(0), "{selected:?}");
    assert_eq!(selected.json()["digest"], digest);
    assert_eq!(selected.json()["role"], "reranker");
    let wrong_role = home.run(&[
        "model",
        "select",
        "--collection",
        "synthetic",
        "--role",
        "embedder",
        "--digest",
        &digest,
    ]);
    assert_eq!(wrong_role.code, Some(2), "{wrong_role:?}");
    assert!(
        wrong_role.stderr.contains("not requested role embedder"),
        "{wrong_role:?}"
    );

    let listed = home.run(&[
        "--json",
        "model",
        "list",
        "--collection",
        "synthetic",
        "--role",
        "reranker",
    ]);
    assert_eq!(listed.code, Some(0), "{listed:?}");
    assert_eq!(listed.json()["evaluations"][0]["disposition"], "eligible");
    assert_eq!(listed.json()["selections"][0]["digest"], digest);
    let text_list = home.run(&[
        "model",
        "list",
        "--collection",
        "synthetic",
        "--role",
        "reranker",
    ]);
    assert_eq!(text_list.code, Some(0), "{text_list:?}");
    assert!(text_list.stdout.contains("evaluation") && text_list.stdout.contains("eligible"));
    assert_eq!(router.requests(), 3);
}

#[test]
fn checking_an_embedder_refuses_and_names_prepare() {
    let home = Home::new();
    home.add_synthetic();
    let card = card_file(&home, false);
    let registered = home.run(&[
        "--json",
        "model",
        "register",
        "--collection",
        "synthetic",
        "--card",
        card.to_str().unwrap(),
    ]);
    let registration = registered.json();
    let digest = registration["digest"].as_str().unwrap();
    let checked = home.run(&[
        "model",
        "check",
        "--collection",
        "synthetic",
        "--digest",
        digest,
    ]);
    assert_eq!(checked.code, Some(2), "{checked:?}");
    assert!(
        checked
            .stderr
            .contains("embedder qualification runs through prepare"),
        "{checked:?}"
    );
}

#[test]
fn reranker_check_records_a_failed_health_gate_for_list_and_select_to_refuse() {
    let home = Home::new();
    home.add_synthetic();
    let card = card_file(&home, true);
    let path = card.to_str().unwrap();
    let registered = home.run(&[
        "model",
        "register",
        "--collection",
        "synthetic",
        "--card",
        path,
    ]);
    assert_eq!(registered.code, Some(0), "{registered:?}");
    let digest = registered
        .stdout
        .split_whitespace()
        .find(|word| word.len() == 64)
        .unwrap()
        .to_owned();
    let router = StubRouter::serve([0.1, 0.2]);

    let mut command = home.command(&[
        "--json",
        "model",
        "check",
        "--collection",
        "synthetic",
        "--digest",
        &digest,
    ]);
    command.env("MAESTRO_ROUTER_URL", &router.url);
    let checked = super::support::Running::of(command).finish();
    assert_eq!(checked.code, Some(0), "{checked:?}");
    assert_eq!(checked.json()["disposition"], "ineligible");
    assert!(
        checked.json()["reason"]
            .as_str()
            .unwrap()
            .contains("positive score")
    );

    let listed = home.run(&[
        "--json",
        "model",
        "list",
        "--collection",
        "synthetic",
        "--role",
        "reranker",
    ]);
    assert_eq!(listed.code, Some(0), "{listed:?}");
    assert_eq!(listed.json()["evaluations"][0]["disposition"], "ineligible");
    let selected = home.run(&[
        "model",
        "select",
        "--collection",
        "synthetic",
        "--role",
        "reranker",
        "--digest",
        &digest,
    ]);
    assert_eq!(selected.code, Some(2), "{selected:?}");
    assert!(
        selected.stderr.contains("eligible real evaluation"),
        "{selected:?}"
    );
    assert_eq!(router.requests(), 3);
}

fn card_file(home: &Home, reranker: bool) -> PathBuf {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    struct CardInput {
        schema: String,
        identity: CardIdentity,
    }

    #[derive(Serialize)]
    struct CardOutput {
        schema: String,
        identity: CardIdentity,
    }

    let source = synthetic()
        .ancestors()
        .nth(3)
        .unwrap()
        .join("crates/maestro-knowledge/tests/fixtures/synthetic/evals/model-card-v2.json");
    let input: CardInput = serde_json::from_slice(&fs::read(source).unwrap()).unwrap();
    let mut identity = input.identity;
    let qualification = b"synthetic qualification";
    let digest = home
        .database()
        .put(qualification, "application/json")
        .unwrap();
    identity.formats.qualification_digest = digest.clone();
    identity
        .provenance
        .artifacts
        .insert("native-qualification".to_owned(), digest);
    if reranker {
        identity.role = Role::Reranker;
        identity.router_entry = RouterEntry::parse("rerank").unwrap();
        identity.invocation.dimensions = Dimensions::NotApplicable;
        identity.formats.embedding = EmbeddingFormat::NotApplicable;
        identity.formats.document = Capability::NotApplicable;
        identity.formats.query = Capability::NotApplicable;
        identity.provenance.qualification_method = QualificationMethod::NativeRuntime;
    }
    let output = CardOutput {
        schema: input.schema,
        identity,
    };
    let path = home.root().join("card.json");
    fs::write(&path, serde_json::to_vec(&output).unwrap()).unwrap();
    path
}

struct StubRouter {
    requests: usize,
    url: String,
}

impl StubRouter {
    fn serve(scores: [f64; 2]) -> Self {
        use std::{net::TcpListener, thread};

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let requests = 3;
        thread::spawn(move || {
            for _ in 0..requests {
                let (stream, _) = listener.accept().unwrap();
                answer_request(stream, scores);
            }
        });
        Self {
            requests,
            url: format!("http://{address}"),
        }
    }

    fn requests(&self) -> usize {
        self.requests
    }
}

fn answer_request(stream: TcpStream, scores: [f64; 2]) {
    use std::io::Write as _;

    let mut reader = BufReader::new(stream);
    let path = request_path_and_body(&mut reader);
    let response = serde_json::to_vec(&router_response(&path, scores)).unwrap();
    let stream = reader.get_mut();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.len()
    )
    .unwrap();
    stream.write_all(&response).unwrap();
}

fn request_path_and_body(reader: &mut BufReader<TcpStream>) -> String {
    use std::io::{BufRead as _, Read as _};

    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let path = line.split_whitespace().nth(1).unwrap().to_owned();
    let mut length = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        if header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.trim_end().split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    path
}

fn router_response(path: &str, scores: [f64; 2]) -> serde_json::Value {
    if path.ends_with("/props") {
        json!({"build_info":"b1234-abcdef"})
    } else {
        json!({"results":[
            {"index":0,"relevance_score":scores[0]},
            {"index":1,"relevance_score":scores[1]}
        ]})
    }
}
