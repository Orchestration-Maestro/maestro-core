//! Registration and listing of scoped model cards through the public CLI.

use super::support::{Home, synthetic};
use maestro_kernel::{
    artifact::Digest,
    gateway::{
        Role, RouterEntry,
        card_v2::{
            Capability, CardIdentity, Dimensions, EmbeddingFormat, FlagValue, QualificationMethod,
        },
    },
};
use serde_json::json;
use std::{
    fs,
    io::{BufReader, Result as IoResult},
    iter,
    net::{TcpListener, TcpStream},
    num::NonZeroU64,
    path::PathBuf,
};

#[test]
fn register_is_idempotent_and_list_shows_the_card_in_text_and_json() {
    let home = Home::new();
    home.add_synthetic();
    let files = card_file(&home, false);

    let registered = register_command(&home, &files, true);
    assert_eq!(
        (registered.code, registered.stderr.as_str()),
        (Some(0), ""),
        "{registered:?}"
    );
    let registration = registered.json();
    let digest = registration["digest"].as_str().unwrap().to_owned();
    assert_eq!(digest.len(), 64, "{registered:?}");
    assert_eq!(registration["schema"], "maestro-cli/model-register/1");

    let again = register_command(&home, &files, false);
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
#[expect(
    clippy::cognitive_complexity,
    reason = "one integration flow proves the healthy gate, exact-role selection and list output"
)]
fn a_passing_reranker_health_gate_allows_exact_card_selection() {
    let home = Home::new();
    home.add_synthetic();
    let files = card_file(&home, true);
    let registered = register_command(&home, &files, true);
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
    assert_eq!(checked.json()["eligible"], true);
    assert_eq!(checked.json()["reason"], serde_json::Value::Null);
    assert_eq!(checked.json()["report_digest"].as_str().unwrap().len(), 64);

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
}

#[test]
fn checking_an_embedder_refuses_and_names_prepare() {
    let home = Home::new();
    home.add_synthetic();
    let files = card_file(&home, false);
    let registered = register_command(&home, &files, true);
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
    let files = card_file(&home, true);
    let registered = register_command(&home, &files, false);
    assert_eq!(registered.code, Some(0), "{registered:?}");
    let digest = registered
        .stdout
        .split_whitespace()
        .find(|word| word.len() == 64)
        .unwrap()
        .to_owned();
    let eligible_router = StubRouter::serve([0.9, 0.1]);
    let mut eligible_command = home.command(&[
        "model",
        "check",
        "--collection",
        "synthetic",
        "--digest",
        &digest,
    ]);
    eligible_command.env("MAESTRO_ROUTER_URL", &eligible_router.url);
    assert_eq!(
        super::support::Running::of(eligible_command).finish().code,
        Some(0)
    );
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
    assert_eq!(checked.json()["eligible"], false);
    assert!(
        checked.json()["reason"]
            .as_str()
            .unwrap()
            .contains("positive score")
    );
    assert_eq!(checked.json()["report_digest"].as_str().unwrap().len(), 64);

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
    assert_eq!(
        listed.json()["evaluations"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["disposition"],
        "ineligible"
    );
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
        selected.stderr.contains("latest real evaluation")
            && selected.stderr.contains("ineligible"),
        "{selected:?}"
    );
}

pub(super) struct CardFiles {
    pub(super) card: PathBuf,
    pub(super) evidence: PathBuf,
    pub(super) gguf: PathBuf,
}

pub(super) fn register_command(
    home: &Home,
    files: &CardFiles,
    json_output: bool,
) -> super::support::Ended {
    let mut arguments = Vec::new();
    if json_output {
        arguments.push("--json");
    }
    arguments.extend([
        "model",
        "register",
        "--collection",
        "synthetic",
        "--card",
        files.card.to_str().unwrap(),
        "--evidence",
        files.evidence.to_str().unwrap(),
        "--gguf",
        files.gguf.to_str().unwrap(),
    ]);
    home.run(&arguments)
}

pub(super) fn card_file(home: &Home, reranker: bool) -> CardFiles {
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
    let database = home.database();
    let evidence_digest = database.put(qualification, "application/json").unwrap();
    identity.formats.qualification_digest = evidence_digest.clone();
    identity
        .provenance
        .artifacts
        .insert("native-qualification".to_owned(), evidence_digest);
    if reranker {
        identity.role = Role::Reranker;
        identity.router_entry = RouterEntry::parse("rerank").unwrap();
        identity.invocation.dimensions = Dimensions::NotApplicable;
        identity.formats.embedding = EmbeddingFormat::NotApplicable;
        identity.formats.document = Capability::NotApplicable;
        identity.formats.query = Capability::NotApplicable;
        identity.provenance.qualification_method = QualificationMethod::NativeRuntime;
    }
    let weight_bytes = vec![b'w'; 1024];
    let weight_digest = Digest::of(&weight_bytes);
    identity.weights.gguf_digest = weight_digest.clone();
    identity.weights.gguf_bytes =
        NonZeroU64::new(u64::try_from(weight_bytes.len()).unwrap()).unwrap();
    identity.formats.tokenizer_digest = weight_digest.clone();
    if let FlagValue::Asset { digest, .. } =
        identity.invocation.server_flags.get_mut("--model").unwrap()
    {
        *digest = weight_digest;
    }

    let evidence = home.root().join("evidence");
    fs::create_dir_all(&evidence).unwrap();
    for digest in iter::once(&identity.formats.qualification_digest)
        .chain(identity.provenance.artifacts.values())
    {
        fs::write(
            evidence.join(format!("{}.json", digest.as_str())),
            database.get(digest).unwrap(),
        )
        .unwrap();
    }
    let gguf = home.root().join("weights.gguf");
    fs::write(&gguf, weight_bytes).unwrap();
    let card = home.root().join("card.json");
    let output = CardOutput {
        schema: input.schema,
        identity,
    };
    fs::write(
        &card,
        format!("{}\n", serde_json::to_string_pretty(&output).unwrap()),
    )
    .unwrap();
    CardFiles {
        card,
        evidence,
        gguf,
    }
}

struct StubRouter {
    url: String,
}

impl StubRouter {
    fn serve(scores: [f64; 2]) -> Self {
        use std::{
            thread,
            time::{Duration, Instant},
        };

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            let mut accepted = 0;
            while accepted < 3 && Instant::now() < deadline {
                accepted += usize::from(serve_one(&listener, scores).is_ok());
                thread::yield_now();
            }
        });
        Self {
            url: format!("http://{address}"),
        }
    }
}

fn serve_one(listener: &TcpListener, scores: [f64; 2]) -> IoResult<()> {
    let (stream, _) = listener.accept()?;
    answer_request(stream, scores);
    Ok(())
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
