//! Tests of the counter's invocation: its environment, its arguments and each platform's loader.
//! The counter here is this test executable itself: a binary outside the system's protected
//! directories, so macOS passes it `DYLD_LIBRARY_PATH` as it passes the real counter.
#[cfg(unix)]
use super::super::TokenCounter;
use super::super::loader::Loader;
use super::super::native::configured_command;
use super::super::process::run_native;
use super::super::{binding::NativeBinding, contract::parse_contract};
use super::Scratch;
#[cfg(unix)]
use super::fake_tokenizer;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    ffi::OsStr,
    fs,
    io::{self, Write},
    path::Path,
    time::Duration,
};

/// The model argument of the probe run; no other run of this test executable receives it.
const PROBE_MODEL: &str = "/a path/model;literal.gguf";
/// What precedes the probe's report among the test harness's own output.
const PROBE_MARK: &str = "ctm-environment-probe:";

/// Not a check of its own: started as the counter by the test below, report the environment and
/// arguments it received on one marked line; in any other run, do nothing.
#[test]
fn environment_probe() {
    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument == PROBE_MODEL) {
        let environment: BTreeMap<String, String> = env::vars().collect();
        let report = json!([environment, arguments]);
        writeln!(io::stdout(), "{PROBE_MARK}{report}").unwrap();
    }
}

#[test]
fn invocation_replaces_environment_and_preserves_model_argument() {
    let mut contract = parse_contract(include_str!("../../../tokenizer-contract.json")).unwrap();
    let probe = "tokenizer::tests::invocation::environment_probe";
    contract["invocation"]["args"] = json!([probe, "--exact", "--nocapture", "{model}"]);
    let binding = NativeBinding {
        model: PROBE_MODEL.into(),
        counter: env::current_exe().unwrap(),
        library_directory: "/somewhere/lib".into(),
        source_root: "/somewhere/src".into(),
    };
    let mut command = configured_command(&contract, &binding).unwrap();
    let output = run_native(&mut command, b"", Duration::from_secs(10)).unwrap();
    let output = String::from_utf8(output).unwrap();
    let (_, report) = output.split_once(PROBE_MARK).expect("the probe reported");
    let report = report.lines().next().unwrap();
    let result: Value = serde_json::from_str(report).unwrap();
    let environment = result[0].as_object().unwrap();
    // macOS may set these in a child whatever environment it was given: CoreFoundation's text
    // encoding, and the SDK paths its xcrun shims export. They are tolerated, never required.
    let injected: &[&str] = if cfg!(target_os = "macos") {
        &[
            "CPATH",
            "LIBRARY_PATH",
            "MANPATH",
            "SDKROOT",
            "__CF_USER_TEXT_ENCODING",
        ]
    } else {
        &[]
    };
    let (loader, search) = if cfg!(windows) {
        ("PATH", "/somewhere/lib;/usr/bin:/bin")
    } else if cfg!(target_os = "macos") {
        ("DYLD_LIBRARY_PATH", "/somewhere/lib")
    } else {
        ("LD_LIBRARY_PATH", "/somewhere/lib")
    };
    // On failure, show only key names, never inherited environment values.
    assert_eq!(
        environment
            .keys()
            .map(String::as_str)
            .filter(|key| !injected.contains(key))
            // The probe is this test executable: under coverage measurement its own profile
            // runtime sets this marker as it starts, so the child adds it itself.
            .filter(|key| !key.starts_with("__LLVM_PROFILE_"))
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["CUDA_VISIBLE_DEVICES", "LC_ALL", "PATH", loader])
    );
    assert_eq!(environment[loader], search);
    assert_eq!(environment["CUDA_VISIBLE_DEVICES"], "");
    assert_eq!(environment["LC_ALL"], "C.UTF-8");
    if loader != "PATH" {
        assert_eq!(environment["PATH"], "/usr/bin:/bin");
    }
    assert_eq!(
        result[1],
        json!([probe, "--exact", "--nocapture", PROBE_MODEL])
    );
}

#[test]
fn each_loader_searches_the_library_directory_first() {
    let directory = Path::new("/somewhere/lib");
    let path = Some(OsStr::new("/usr/bin:/bin"));
    for (loader, name, value) in [
        (Loader::LdSo, "LD_LIBRARY_PATH", "/somewhere/lib"),
        (Loader::Dyld, "DYLD_LIBRARY_PATH", "/somewhere/lib"),
        (Loader::Windows, "PATH", "/somewhere/lib;/usr/bin:/bin"),
    ] {
        let variable = loader.search_variable(directory, path).unwrap();
        assert_eq!(variable, (name, value.into()), "{loader:?}");
    }
    let alone = Loader::Windows.search_variable(directory, None).unwrap();
    assert_eq!(alone, ("PATH", "/somewhere/lib".into()));
    // A directory holding the list separator would be searched as two directories.
    for (loader, directory) in [
        (Loader::LdSo, "/a:b/lib"),
        (Loader::Dyld, "/a:b/lib"),
        (Loader::Windows, "/a;b/lib"),
    ] {
        assert!(loader.search_variable(Path::new(directory), path).is_err());
    }
    assert!(
        Loader::LdSo
            .search_variable(Path::new("/a;b"), path)
            .is_ok()
    );
    assert!(
        Loader::Windows
            .search_variable(Path::new("/a:b"), None)
            .is_ok()
    );
}

#[test]
fn only_windows_requires_the_counter_beside_its_libraries() {
    let scratch = Scratch::new();
    let libraries = scratch.0.join("bin");
    fs::create_dir(&libraries).unwrap();
    let beside = libraries.join("count");
    let outside = scratch.0.join("count");
    for counter in [&beside, &outside] {
        fs::write(counter, b"counter").unwrap();
    }
    for loader in [Loader::LdSo, Loader::Dyld, Loader::Windows] {
        loader.check_counter_location(&beside, &libraries).unwrap();
    }
    for loader in [Loader::LdSo, Loader::Dyld] {
        loader.check_counter_location(&outside, &libraries).unwrap();
    }
    let refused = Loader::Windows.check_counter_location(&outside, &libraries);
    assert!(refused.unwrap_err().to_string().contains("Windows"));
    let missing = scratch.0.join("missing");
    assert!(
        Loader::Windows
            .check_counter_location(&missing, &libraries)
            .is_err()
    );
    // Named through a different spelling of the same directory, it is still beside them.
    let respelled = libraries.join("..").join("bin").join("count");
    Loader::Windows
        .check_counter_location(&respelled, &libraries)
        .unwrap();
}

#[test]
fn the_host_uses_its_own_platforms_loader() {
    let expected = if cfg!(windows) {
        Loader::Windows
    } else if cfg!(target_os = "macos") {
        Loader::Dyld
    } else {
        Loader::LdSo
    };
    assert_eq!(Loader::HOST, expected);
}

#[cfg(unix)]
#[test]
fn an_unverified_library_stops_the_tokenizer_before_it_counts() {
    let scratch = Scratch::new();
    let tokenizer = fake_tokenizer(&scratch);
    let extra = scratch.0.join("lib/libextra.so");
    fs::write(&extra, b"extra").unwrap();
    assert!(tokenizer.verify_artifacts().is_err());
    assert!(tokenizer.token_ids("any text").is_err());
    fs::remove_file(extra).unwrap();
    assert_eq!(tokenizer.token_ids("any text").unwrap(), [0, 17, 2]);
}

#[cfg(unix)]
#[test]
fn a_changed_counter_is_refused_before_it_runs() {
    let scratch = Scratch::new();
    let mut tokenizer = fake_tokenizer(&scratch);
    let counter = scratch.0.join("counter");
    fs::copy(&tokenizer.binding.counter, &counter).unwrap();
    tokenizer.binding.counter.clone_from(&counter);
    tokenizer.verify_artifacts().unwrap();
    fs::write(&counter, b"changed").unwrap();
    let refused = tokenizer.token_ids("any text").unwrap_err();
    assert_eq!(
        refused.to_string(),
        "tokenizer artifact size or file type mismatch"
    );
}

#[cfg(unix)]
#[test]
fn a_failing_counter_process_returns_no_ids() {
    let scratch = Scratch::new();
    let mut tokenizer = fake_tokenizer(&scratch);
    tokenizer.contract["invocation"]["args"] = json!(["-c", "cat >/dev/null; exit 3", "{model}"]);
    let refused = tokenizer.token_ids("any text").unwrap_err();
    assert_eq!(refused.to_string(), "tokenizer process failed");
}
