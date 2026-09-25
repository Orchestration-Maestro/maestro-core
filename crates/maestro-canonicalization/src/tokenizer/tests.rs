//! Tests of the native tokenizer: profile identity, artifacts, process limits and output.
use super::NativeTokenizer;
use super::artifacts::{verify_artifact, verify_libraries};
use super::binding::NativeBinding;
use super::contract::{array_at, parse_contract, text_at};
use super::native::{configured_command, parse_ids};
use super::process::run_native;
use crate::{Error, digest};
#[cfg(unix)]
use rustix::fs::{Mode, mkfifoat};
use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::{
    collections::BTreeSet,
    env,
    fs::{self, File},
    path::{Path, PathBuf},
    process::{self, Command},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "ctm-tokenizer-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn write_binding(scratch: &Scratch, text: &str) -> PathBuf {
    let path = scratch.0.join("binding.json");
    fs::write(&path, text).unwrap();
    path
}

/// The size and SHA-256 record of a file, with its profile name when it has one.
fn record(path: &Path, file: Option<&str>) -> Value {
    let bytes = fs::read(path).unwrap();
    let mut record = json!({"bytes": bytes.len(), "sha256": digest(&bytes)});
    if let Some(file) = file {
        record["file"] = file.into();
    }
    record
}

/// A tokenizer whose artifacts are small files in `scratch` and whose counter is the system shell
/// printing `[0,17,2]`: every artifact and library check runs, without the qualified model.
#[cfg(unix)]
fn fake_tokenizer(scratch: &Scratch) -> NativeTokenizer {
    let root = &scratch.0;
    for directory in ["lib", "src"] {
        fs::create_dir(root.join(directory)).unwrap();
    }
    fs::write(root.join("model.gguf"), b"model").unwrap();
    fs::write(root.join("src/vocab.cpp"), b"source").unwrap();
    fs::write(root.join("lib/libfake.so.1.2"), b"library").unwrap();
    for alias in ["libfake.so", "libfake.so.1"] {
        symlink("libfake.so.1.2", root.join("lib").join(alias)).unwrap();
    }
    // The shell itself, not a link to it: artifacts are opened without following links.
    let counter = fs::canonicalize("/bin/sh").unwrap();
    let contract = json!({
        "artifacts": {
            "model": record(&root.join("model.gguf"), None),
            "counter": record(&counter, None),
            "sources": [record(&root.join("src/vocab.cpp"), Some("vocab.cpp"))],
            "libraries": [record(&root.join("lib/libfake.so.1.2"), Some("libfake.so.1.2"))],
        },
        "invocation": {
            "args": ["-c", "cat >/dev/null; printf '[0,17,2]'", "{model}"],
            "environment_replace": {"PATH": "/usr/bin:/bin"},
            "timeout_seconds": 10,
        },
    });
    NativeTokenizer {
        contract,
        binding: NativeBinding {
            model: root.join("model.gguf"),
            counter,
            library_directory: root.join("lib"),
            source_root: root.join("src"),
        },
    }
}

/// `verify_artifact` on its own thread: the test fails, rather than hangs, if the open blocks.
fn verify_without_blocking(path: &Path, bytes: u64, hash: &'static str) -> Result<(), Error> {
    let path = path.to_owned();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || sender.send(verify_artifact(&path, bytes, hash)));
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("opening the artifact blocked")
}

#[cfg(unix)]
#[test]
fn a_verified_installation_counts_through_its_counter() {
    let scratch = Scratch::new();
    let tokenizer = fake_tokenizer(&scratch);
    tokenizer.verify_artifacts().unwrap();
    assert_eq!(tokenizer.token_ids("any text").unwrap(), [0, 17, 2]);
}

#[cfg(unix)]
#[test]
fn a_changed_artifact_is_refused_until_restored() {
    let scratch = Scratch::new();
    let tokenizer = fake_tokenizer(&scratch);
    for name in ["model.gguf", "src/vocab.cpp", "lib/libfake.so.1.2"] {
        let path = scratch.0.join(name);
        let original = fs::read(&path).unwrap();
        fs::write(&path, b"changed").unwrap();
        assert!(tokenizer.verify_artifacts().is_err(), "{name}");
        fs::write(&path, original).unwrap();
        tokenizer.verify_artifacts().unwrap();
    }
}

#[cfg(unix)]
#[test]
fn the_reported_contract_id_is_the_committed_profiles() {
    let scratch = Scratch::new();
    let committed = parse_contract(include_str!("../../tokenizer-contract.json")).unwrap();
    assert_eq!(
        committed["contract_id"],
        fake_tokenizer(&scratch).contract_id()
    );
}

#[test]
fn a_binding_resolves_relative_paths_against_its_own_directory() {
    let scratch = Scratch::new();
    let path = write_binding(
        &scratch,
        r#"{"schema":"maestro-native-binding/1","model":"models/m.gguf",
            "counter":"/somewhere/bin/count","library_directory":"lib","source_root":"src"}"#,
    );
    let binding = NativeBinding::from_file(&path).unwrap();
    assert_eq!(binding.model, scratch.0.join("models/m.gguf"));
    assert_eq!(binding.counter, PathBuf::from("/somewhere/bin/count"));
    assert_eq!(binding.library_directory, scratch.0.join("lib"));
    assert_eq!(binding.source_root, scratch.0.join("src"));
}

#[test]
fn a_binding_refusal_names_the_variable_and_the_schema() {
    let scratch = Scratch::new();
    let fields = r#""model":"m","counter":"c","library_directory":"l","source_root":"s""#;
    let oversized = format!(
        r#"{{"schema":"maestro-native-binding/1",{fields},"x":"{}"}}"#,
        "a".repeat(1024 * 1024)
    );
    for text in [
        format!(r#"{{"schema":"maestro-native-binding/2",{fields}}}"#),
        format!(r#"{{"schema":"maestro-native-binding/1",{fields},"extra":1}}"#),
        r#"{"schema":"maestro-native-binding/1","model":"m"}"#.to_owned(),
        "not json".to_owned(),
        oversized,
    ] {
        let path = write_binding(&scratch, &text);
        let error = NativeBinding::from_file(&path).unwrap_err().to_string();
        assert!(error.contains("MAESTRO_NATIVE_BINDING"), "{error}");
        assert!(error.contains("maestro-native-binding/1"), "{error}");
    }
    let unset = NativeBinding::from_variable(None).unwrap_err().to_string();
    assert!(unset.contains("MAESTRO_NATIVE_BINDING"), "{unset}");
    assert!(NativeBinding::from_file(&scratch.0.join("missing.json")).is_err());
}

#[test]
fn the_committed_profile_names_files_never_machine_paths() {
    let profile = parse_contract(include_str!("../../tokenizer-contract.json")).unwrap();
    assert_eq!(profile["schema"], "local-tokenizer-contract/2");
    assert!(!profile.to_string().contains("\"path\""));
    let libraries = array_at(&profile, "/artifacts/libraries").unwrap();
    let sources = array_at(&profile, "/artifacts/sources").unwrap();
    for record in libraries.iter().chain(sources) {
        let file = text_at(record, "/file").unwrap();
        assert!(
            !Path::new(file).is_absolute() && !file.contains(".."),
            "{file}"
        );
    }
}

#[test]
fn ids_require_integer_vocabulary_values_and_model_specials() {
    assert_eq!(parse_ids(b"[0,35378,8999,2]").unwrap(), [0, 35378, 8999, 2]);
    assert_eq!(parse_ids(b"[0,2]").unwrap(), [0, 2]);
    for bytes in [
        b"[]".as_slice(),
        b"[0,2.0]",
        b"[0,-1,2]",
        b"[0,250002,2]",
        b"[35378,2]",
        b"[0,35378]",
        b"[0,2] trailing",
        b"[0,\"2\"]",
        b"[0,4294967296,2]",
        b"{\"ids\":[0,2]}",
        b"[0,true,2]",
    ] {
        assert!(parse_ids(bytes).is_err());
    }
}

#[test]
fn artifact_check_requires_original_bytes_and_regular_file() {
    let scratch = Scratch::new();
    let path = scratch.0.join("asset");
    fs::write(&path, b"abc").unwrap();
    let hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    verify_artifact(&path, 3, hash).unwrap();
    assert!(verify_artifact(&path, 4, hash).is_err());
    fs::write(&path, b"bad").unwrap();
    assert!(verify_artifact(&path, 3, hash).is_err());
    assert!(verify_artifact(&scratch.0.join("missing"), 3, hash).is_err());
    assert!(verify_artifact(&scratch.0, 0, hash).is_err());
    #[cfg(unix)]
    {
        // A link is refused even to the right bytes; so is a FIFO, whose empty content would match.
        fs::write(&path, b"abc").unwrap();
        let link = scratch.0.join("link");
        symlink(&path, &link).unwrap();
        assert!(verify_artifact(&link, 3, hash).is_err());
        let fifo = scratch.0.join("fifo");
        mkfifoat(
            File::open(&scratch.0).unwrap(),
            "fifo",
            Mode::RUSR | Mode::WUSR,
        )
        .unwrap();
        let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert!(verify_without_blocking(&fifo, 0, empty).is_err());
    }
}

#[test]
fn process_preserves_stdin_bytes_and_drains_both_streams() {
    let script = concat!(
        "import sys; data=sys.stdin.buffer.read(); sys.stderr.write('x'*100000); ",
        "sys.stdout.buffer.write(data)"
    );
    let mut command = Command::new("/usr/bin/python3");
    command.args(["-c", script]);
    let input = b"  a\0b\\n\r\n ".repeat(20000);
    assert_eq!(
        run_native(&mut command, &input, Duration::from_secs(10)).unwrap(),
        input
    );
}

#[test]
fn process_errors_and_timeouts_do_not_return_partial_output() {
    let mut failed = Command::new("/bin/sh");
    failed.args(["-c", "printf '[0,2]'; printf 'private-stderr' >&2; exit 7"]);
    let error = run_native(&mut failed, b"private-input", Duration::from_secs(2)).unwrap_err();
    assert!(!error.to_string().contains("private"));
    let mut hung = Command::new("/bin/sh");
    hung.args(["-c", "exec sleep 10"]);
    let started = Instant::now();
    assert!(
        run_native(
            &mut hung,
            &vec![b'x'; 1024 * 1024],
            Duration::from_millis(40)
        )
        .is_err()
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(
        run_native(
            &mut Command::new("/missing-ctm-tokenizer"),
            b"",
            Duration::from_secs(1)
        )
        .is_err()
    );
}

#[test]
fn contract_identity_cannot_be_self_asserted_or_silently_changed() {
    let text = include_str!("../../tokenizer-contract.json");
    let mut contract = parse_contract(text).unwrap();
    contract["chunk_hard_max"] = json!(701);
    assert!(parse_contract(&contract.to_string()).is_err());
    contract["contract_id"] = json!("sha256:unapproved");
    assert!(parse_contract(&contract.to_string()).is_err());
    for invalid in ["null", "[]", "{}", "{", "{\"contract_id\":0}"] {
        assert!(parse_contract(invalid).is_err());
    }
}

#[test]
fn output_and_bindings_at_their_exact_limits_are_accepted() {
    let mut command = Command::new("/usr/bin/python3");
    command.args([
        "-c",
        "import sys; sys.stdout.buffer.write(b'x'*(16*1024*1024))",
    ]);
    let output = run_native(&mut command, b"", Duration::from_secs(10)).unwrap();
    assert_eq!(output.len(), 16 * 1024 * 1024);
    let scratch = Scratch::new();
    let text = r#"{"schema":"maestro-native-binding/1","model":"m","counter":"c",
        "library_directory":"l","source_root":"s"}"#;
    let padded = format!("{text}{}", " ".repeat(1024 * 1024 - text.len()));
    NativeBinding::from_file(&write_binding(&scratch, &padded)).unwrap();
}

#[test]
fn oversized_process_output_is_refused_not_truncated() {
    for script in [
        "import sys; sys.stdout.buffer.write(b'x'*(16*1024*1024+1))",
        "import sys; sys.stderr.buffer.write(b'x'*(1024*1024+1)); print('[0,2]')",
    ] {
        let mut command = Command::new("/usr/bin/python3");
        command.args(["-c", script]);
        assert!(run_native(&mut command, b"", Duration::from_secs(10)).is_err());
    }
}

#[cfg(unix)]
#[test]
fn library_aliases_cannot_redirect_away_from_pinned_files() {
    let scratch = Scratch::new();
    let path = scratch.0.join("libfixture.so.1.2");
    fs::write(&path, b"abc").unwrap();
    let base = scratch.0.join("libfixture.so");
    symlink(&path, &base).unwrap();
    symlink(&path, scratch.0.join("libfixture.so.1")).unwrap();
    let libraries = [path.clone()];
    verify_libraries(&libraries).unwrap();
    fs::remove_file(&base).unwrap();
    assert!(verify_libraries(&libraries).is_err());
    let outside = Scratch::new();
    let replacement = outside.0.join("libfixture.so.1.2");
    fs::write(&replacement, b"abc").unwrap();
    symlink(&replacement, &base).unwrap();
    assert!(verify_libraries(&libraries).is_err());
    fs::remove_file(&base).unwrap();
    symlink(&path, &base).unwrap();
    let extra = scratch.0.join("libextra.so");
    fs::write(&extra, b"abc").unwrap();
    assert!(verify_libraries(&libraries).is_err());
    fs::remove_file(extra).unwrap();
    verify_libraries(&libraries).unwrap();
}

#[test]
fn invocation_replaces_environment_and_preserves_model_argument() {
    let mut contract = parse_contract(include_str!("../../tokenizer-contract.json")).unwrap();
    contract["invocation"]["args"] = json!([
        "-c",
        "import json,os,sys; print(json.dumps([dict(os.environ),sys.argv[1:]]))",
        "{model}"
    ]);
    let binding = NativeBinding {
        model: "/a path/model;literal.gguf".into(),
        counter: "/usr/bin/python3".into(),
        library_directory: "/somewhere/lib".into(),
        source_root: "/somewhere/src".into(),
    };
    let mut command = configured_command(&contract, &binding).unwrap();
    let output = run_native(&mut command, b"", Duration::from_secs(2)).unwrap();
    let result: Value = serde_json::from_slice(&output).unwrap();
    let environment = result[0].as_object().unwrap();
    // On failure, show only key names, never inherited environment values.
    assert_eq!(
        environment
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["CUDA_VISIBLE_DEVICES", "LC_ALL", "LD_LIBRARY_PATH", "PATH"])
    );
    assert_eq!(environment["LD_LIBRARY_PATH"], "/somewhere/lib");
    assert_eq!(environment["CUDA_VISIBLE_DEVICES"], "");
    assert_eq!(environment["LC_ALL"], "C.UTF-8");
    assert_eq!(environment["PATH"], "/usr/bin:/bin");
    assert_eq!(result[1], json!(["/a path/model;literal.gguf"]));
}
