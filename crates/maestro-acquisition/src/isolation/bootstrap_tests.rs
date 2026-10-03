//! Default bootstrap decisions use the production dispatch and init drivers.
use super::{
    bootstrap::{close_result, descriptors, dispatch, init_with},
    launch::Configuration,
    port::{BootstrapIo, Refusal},
};
use nix::{errno::Errno, fcntl::AtFlags, sched::CloneFlags};
use rustix::process::{getgid, getuid};
use std::{
    cell::RefCell, ffi::CString, fs::File, io, iter::empty, os::fd::AsRawFd as _, path::Path,
    process::Command,
};

/// Ordered effects with an independently selected refusal point.
#[derive(Default)]
pub(super) struct Effects {
    pub(super) events: RefCell<Vec<String>>,
    failure: Option<&'static str>,
    pub(super) profile: String,
    pid: i32,
    success: bool,
    parser_fd: RefCell<Option<i32>>,
}
impl Effects {
    fn record(&self, event: impl Into<String>) -> Result<(), Refusal> {
        let event = event.into();
        self.events.borrow_mut().push(event.clone());
        if self
            .failure
            .is_some_and(|failure| event.starts_with(failure))
        {
            Err(Refusal::Unsupported)
        } else {
            Ok(())
        }
    }
}
impl BootstrapIo for Effects {
    fn hygiene(&self, keep: &[i32]) -> Result<(), Refusal> {
        self.record(format!("fds:{keep:?}"))
    }
    fn profile(&self) -> Result<String, Refusal> {
        self.record("profile")?;
        Ok(self.profile.clone())
    }
    fn unshare(&self, flags: CloneFlags) -> Result<(), Refusal> {
        self.record(format!("unshare:{:x}", flags.bits()))
    }
    fn map(&self, path: &str, value: &str) -> Result<(), Refusal> {
        self.record(format!("map:{path}:{value}"))
    }
    fn pid(&self) -> i32 {
        self.pid
    }
    fn parser(&self, descriptor: i32) -> Result<File, Refusal> {
        self.record(format!("parser:{descriptor}"))?;
        let parser = File::open("/dev/null").unwrap();
        *self.parser_fd.borrow_mut() = Some(parser.as_raw_fd());
        Ok(parser)
    }
    fn filesystem(&self, root: &Path, memory_bytes: u64) -> Result<(), Refusal> {
        assert_eq!(root, Path::new("/synthetic"));
        assert_eq!(memory_bytes, 123);
        self.record("filesystem")
    }
    fn landlock(&self, root: &Path, loader: Option<&str>) -> Result<(), Refusal> {
        assert_eq!(root, Path::new("/"));
        self.record(format!("landlock:{loader:?}"))
    }
    fn capabilities(&self) -> Result<(), Refusal> {
        self.record("capabilities")
    }
    fn restrict(&self) -> Result<(), Refusal> {
        self.record("seccomp")
    }
    fn exec(
        &self,
        _: &File,
        args: &[CString],
        environment: &[CString],
        flags: AtFlags,
    ) -> Result<(), Refusal> {
        assert!(environment.is_empty());
        assert_eq!(flags.bits(), 0x1000);
        self.record(format!("exec:{args:?}:env={environment:?}"))
    }
    fn handoff(&self, command: &mut Command) -> Result<bool, Refusal> {
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(*args.first().unwrap(), "init");
        let config: Configuration =
            serde_json::from_str(args.get(1).unwrap().to_str().unwrap()).unwrap();
        assert_eq!(
            command.get_program(),
            format!("/proc/self/fd/{}", config.bootstrap_fd).as_str()
        );
        self.record(format!(
            "handoff:{}:env=[]:stderr=null",
            config.bootstrap_fd
        ))?;
        Ok(self.success)
    }
}
pub(super) fn config() -> Configuration {
    Configuration {
        root: "/synthetic".into(),
        parser_fd: 17,
        bootstrap_fd: 18,
        arguments: vec!["--scoped".into()],
        interpreter: Some("loader".into()),
        memory_bytes: 123,
    }
}
pub(super) fn effects() -> Effects {
    Effects {
        pid: 1,
        success: true,
        profile: "maestro-n17-parser-bootstrap (enforce)\n".into(),
        ..Effects::default()
    }
}
pub(super) fn invoke(
    args: &[&str],
    bytes: &[u8],
    profile: bool,
    effects: &Effects,
) -> (Result<(), Refusal>, Vec<u8>, Vec<u8>) {
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    let result = dispatch(
        args.iter().map(|arg| (*arg).to_owned()),
        &mut io::Cursor::new(bytes),
        (&mut output, &mut diagnostics),
        profile,
        effects,
    );
    (result, output, diagnostics)
}
#[test]
fn n17_bootstrap_dispatch_bounds_modes_profiles_and_handoff() {
    for args in [
        vec!["unknown"],
        vec!["init"],
        vec!["probe-unprivileged"],
        vec!["probe-unprivileged", "root"],
    ] {
        assert_eq!(
            invoke(&args, b"", false, &effects()).0,
            Err(Refusal::Configuration)
        );
    }
    for bytes in [b"".as_slice(), b"{}", b"invalid", b"{\"unknown\":true}"] {
        assert_eq!(
            invoke(&[], bytes, false, &effects()).0,
            Err(Refusal::Configuration)
        );
    }
}
#[test]
fn n17_bootstrap_profile_maps_and_handoff_order() {
    let encoded = serde_json::to_vec(&config()).unwrap();
    let fx = effects();
    let (result, _, diagnostic) = invoke(&[], &encoded, true, &fx);
    assert_eq!(result, Ok(()));
    assert_eq!(
        String::from_utf8(diagnostic).unwrap(),
        "N17_APPARMOR_ATTACHED maestro-n17-parser-bootstrap (enforce)\n"
    );
    let events = fx.events.borrow();
    assert_eq!(events[0], "fds:[18, 17]");
    assert_eq!(events[1], "profile");
    assert_eq!(events[2], "unshare:10000000");
    assert_eq!(events[3], "map:/proc/self/setgroups:deny");
    assert_eq!(
        events[4],
        format!("map:/proc/self/uid_map:0 {} 1\n", getuid().as_raw())
    );
    assert_eq!(
        events[5],
        format!("map:/proc/self/gid_map:0 {} 1\n", getgid().as_raw())
    );
    assert_eq!(events[6], "unshare:6c020000");
    assert_eq!(events[7], "handoff:18:env=[]:stderr=null");
    drop(events);
}
#[test]
fn n17_bootstrap_profile_and_child_crash_refusals() {
    let encoded = serde_json::to_vec(&config()).unwrap();
    for profile in [
        "unconfined",
        "maestro-n17-parser-bootstrap",
        "other (enforce)",
    ] {
        let fx = Effects {
            profile: profile.into(),
            ..effects()
        };
        assert_eq!(
            invoke(&[], &encoded, true, &fx).0,
            Err(Refusal::Unsupported)
        );
        assert_eq!(fx.events.borrow().len(), 2);
    }
    let fx = Effects {
        success: false,
        ..effects()
    };
    assert_eq!(invoke(&[], &encoded, false, &fx).0, Err(Refusal::Crash));
    assert!(!fx.events.borrow().contains(&"profile".to_owned()));
}
#[test]
fn n17_bootstrap_config_limit_equality_one_over_and_io_error() {
    let mut cfg = config();
    cfg.arguments = vec![String::new()];
    let base = serde_json::to_vec(&cfg).unwrap().len();
    cfg.arguments[0] = "x".repeat(4 * 1024 * 1024 - base);
    let mut bytes = serde_json::to_vec(&cfg).unwrap();
    assert_eq!(bytes.len(), 4 * 1024 * 1024);
    assert_eq!(invoke(&[], &bytes, false, &effects()).0, Ok(()));
    bytes.push(b' ');
    assert_eq!(
        invoke(&[], &bytes, false, &effects()).0,
        Err(Refusal::Configuration)
    );
    let mut failing = io::BufReader::new(ReadError);
    assert_eq!(
        dispatch(
            empty(),
            &mut failing,
            (&mut Vec::new(), &mut Vec::new()),
            false,
            &effects()
        ),
        Err(Refusal::Configuration)
    );
}
#[test]
fn n17_bootstrap_init_pid_arguments_ready_exec_and_failure_order() {
    let cfg = config();
    for pid in [0, 2, 100] {
        let fx = Effects { pid, ..effects() };
        assert_eq!(
            init_with(&cfg, &mut Vec::new(), &fx),
            Err(Refusal::Containment)
        );
        assert!(fx.events.borrow().is_empty());
    }
    let fx = effects();
    let mut output = Vec::new();
    assert_eq!(init_with(&cfg, &mut output, &fx), Ok(()));
    assert_eq!(output, b"{\"kind\":\"ready\"}\n");
    let events = fx.events.borrow();
    assert_eq!(events[0], "parser:17");
    assert!(events[1].starts_with("fds:["));
    assert_eq!(
        &events[2..6],
        [
            "filesystem",
            "landlock:Some(\"loader\")",
            "capabilities",
            "seccomp"
        ]
    );
    assert_eq!(events[6], "exec:[\"/parser\", \"--scoped\"]:env=[]");
    drop(events);
    for step in [
        "parser",
        "fds",
        "filesystem",
        "landlock",
        "capabilities",
        "seccomp",
        "exec",
    ] {
        let fx = Effects {
            failure: Some(step),
            ..effects()
        };
        let mut out = Vec::new();
        assert_eq!(init_with(&cfg, &mut out, &fx), Err(Refusal::Unsupported));
        assert!(fx.events.borrow().last().unwrap().starts_with(step));
        assert_eq!(!out.is_empty(), step == "exec");
    }
    let mut cfg = config();
    cfg.arguments.push("bad\0arg".into());
    assert_eq!(
        init_with(&cfg, &mut Vec::new(), &effects()),
        Err(Refusal::Configuration)
    );
}
#[test]
fn n17_bootstrap_fd_selection_and_closed_iterator_errors() {
    assert_eq!(
        descriptors(
            [
                Ok("0".into()),
                Ok("1".into()),
                Ok("2".into()),
                Ok("3".into()),
                Ok("4".into()),
                Ok("8".into())
            ],
            &[4]
        )
        .unwrap(),
        [3, 8]
    );
    assert_eq!(
        descriptors([Ok("bad".into())], &[]),
        Err(Refusal::Containment)
    );
    assert_eq!(
        descriptors([Err(io::Error::other("iterator"))], &[]),
        Err(Refusal::Containment)
    );
    assert_eq!(close_result(Ok(())), Ok(()));
    assert_eq!(close_result(Err(Errno::EBADF)), Ok(()));
    assert_eq!(close_result(Err(Errno::EPERM)), Err(Refusal::Containment));
}

#[test]
fn n17_bootstrap_each_unprivileged_observation_is_mandatory() {
    use super::bootstrap::{qualify_files, qualify_network};
    assert_eq!(qualify_files(true, false, false, b"scoped"), Ok(()));
    for (privileges, canary, writable, bytes) in [
        (false, false, false, b"scoped".as_slice()),
        (true, true, false, b"scoped"),
        (true, false, true, b"scoped"),
        (true, false, false, b"changed"),
    ] {
        assert_eq!(
            qualify_files(privileges, canary, writable, bytes),
            Err(Refusal::Containment)
        );
    }
    assert_eq!(qualify_network(false, false), Ok(()));
    for (tcp, udp) in [(true, false), (false, true), (true, true)] {
        assert_eq!(qualify_network(tcp, udp), Err(Refusal::Containment));
    }
}

/// A writer that observes the same ordering trace as the pinned exec effect.
struct Output<'a> {
    effects: &'a Effects,
    fail: Option<&'static str>,
}
impl io::Write for Output<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        assert_eq!(bytes, b"{\"kind\":\"ready\"}\n");
        self.effects.record("ready").unwrap();
        if self.fail == Some("write") {
            Err(io::Error::other("write"))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.effects.record("flush").unwrap();
        if self.fail == Some("flush") {
            Err(io::Error::other("flush"))
        } else {
            Ok(())
        }
    }
}
#[test]
fn n17_bootstrap_ready_flush_precedes_exec_and_write_failure_refuses() {
    let fx = effects();
    let mut output = Output {
        effects: &fx,
        fail: None,
    };
    assert_eq!(init_with(&config(), &mut output, &fx), Ok(()));
    assert_eq!(
        &fx.events.borrow()[6..],
        ["ready", "flush", "exec:[\"/parser\", \"--scoped\"]:env=[]"]
    );
    for fail in ["write", "flush"] {
        let fx = effects();
        let mut output = Output {
            effects: &fx,
            fail: Some(fail),
        };
        assert_eq!(
            init_with(&config(), &mut output, &fx),
            Err(Refusal::Containment)
        );
        assert!(
            !fx.events
                .borrow()
                .iter()
                .any(|event| event.starts_with("exec:"))
        );
    }
}

#[test]
fn n17_bootstrap_namespace_failures_and_dispatch_probe_init_routes() {
    let encoded = serde_json::to_vec(&config()).unwrap();
    for step in [
        "fds",
        "profile",
        "unshare:100",
        "map:/proc/self/setgroups",
        "map:/proc/self/uid_map",
        "map:/proc/self/gid_map",
        "unshare:6c",
        "handoff",
    ] {
        let fx = Effects {
            failure: Some(step),
            ..effects()
        };
        assert_eq!(
            invoke(&[], &encoded, true, &fx).0,
            Err(Refusal::Unsupported)
        );
        assert!(fx.events.borrow().last().unwrap().starts_with(step));
    }
    let (result, output, _) = invoke(&["probe"], b"", false, &effects());
    assert_eq!(result, Ok(()));
    assert_eq!(
        output,
        b"N17_PROFILE_PROBE maestro-n17-parser-bootstrap (enforce)\n"
    );
    // A test effect that deliberately leaves a real inherited FD must be refused.
    let file = File::open("/dev/null").unwrap();
    let fd = file.as_raw_fd().to_string();
    let fx = effects();
    assert_eq!(
        invoke(&["probe", &fd], b"", false, &fx).0,
        Err(Refusal::Containment)
    );
    assert_eq!(*fx.events.borrow(), ["fds:[]"]);
    let encoded = String::from_utf8(encoded).unwrap();
    assert_eq!(
        invoke(&["init", &encoded], b"", false, &effects()).0,
        Ok(())
    );
    assert_eq!(
        invoke(&["init", "invalid"], b"", false, &effects()).0,
        Err(Refusal::Configuration)
    );
    assert_eq!(
        invoke(
            &["init", &"x".repeat(4 * 1024 * 1024 + 1)],
            b"",
            false,
            &effects()
        )
        .0,
        Err(Refusal::Configuration)
    );
}

/// Deterministic failed launch-barrier read, rather than an empty input approximation.
struct ReadError;
impl io::Read for ReadError {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("barrier read"))
    }
}
#[test]
fn n17_default_namespace_plan_has_every_required_namespace() {
    let fx = effects();
    assert_eq!(invoke(&["probe"], b"", false, &fx).0, Ok(()));
    let events = fx.events.borrow();
    assert!(events.contains(&"unshare:10000000".to_owned()));
    assert_eq!(events.last().unwrap(), "unshare:6c020000");
}

#[test]
fn n17_bootstrap_required_profile_capture_is_explicit() {
    use super::bootstrap::profile_required;
    assert!(profile_required(Some("required")));
    for value in [None, Some(""), Some("optional"), Some("Required")] {
        assert!(!profile_required(value));
    }
}

#[test]
fn n17_bootstrap_init_preserves_reopened_fd_root_and_optional_loader() {
    for loader in [None, Some("loader".to_owned())] {
        let mut cfg = config();
        cfg.interpreter = loader.clone();
        cfg.parser_fd = -1;
        let fx = effects();
        assert_eq!(init_with(&cfg, &mut Vec::new(), &fx), Ok(()));
        let fd = fx.parser_fd.borrow().unwrap();
        assert_ne!(fd, cfg.parser_fd);
        let events = fx.events.borrow();
        assert_eq!(events[1], format!("fds:[{fd}]"));
        assert_eq!(events[3], format!("landlock:{loader:?}"));
    }
}
