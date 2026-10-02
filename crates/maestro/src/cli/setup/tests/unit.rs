//! Where the service lives and the unit that runs it: Qdrant bound to the
//! loopback address with telemetry off, its data under the kernel's data
//! directory, each path written as systemd reads it back.

use super::super::{
    command::graph_text,
    graph::GraphSetup,
    release::QDRANT,
    service::{Layout, unit_text},
};
use crate::failure::Failure;
use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _, path::Path};

/// The layout of a kernel whose data directory is `data`, under the
/// configuration home `/config`.
fn layout(data: &str) -> Layout {
    Layout::new(Path::new(data), Path::new("/config"))
}

#[test]
fn graph_setup_text_covers_every_action_and_changed_state() {
    for (action, changed, expected) in [
        (
            "create_directory",
            true,
            "Graph: the embedded engine's directory /data/graph, created.",
        ),
        (
            "create_directory",
            false,
            concat!(
                "Graph: the embedded engine's directory /data/graph, to create, which ",
                "`maestro setup --yes` does."
            ),
        ),
        (
            "secure_permissions",
            true,
            "Graph: the embedded engine's directory /data/graph, given mode 0700.",
        ),
        (
            "secure_permissions",
            false,
            concat!(
                "Graph: the embedded engine's directory /data/graph, to give mode 0700, which ",
                "`maestro setup --yes` does."
            ),
        ),
        (
            "ready",
            false,
            "Graph: the embedded engine's directory /data/graph, in place.",
        ),
    ] {
        let graph = GraphSetup {
            engine: "ladybug",
            directory: Some("/data/graph".to_owned()),
            action,
            changed,
            detail: None,
        };
        assert_eq!(graph_text(&graph), expected, "{action}, changed={changed}");
    }
}

#[test]
fn the_service_lives_under_the_kernels_data_directory() {
    let layout = layout("/data/maestro");
    assert_eq!(layout.root, Path::new("/data/maestro/qdrant"));
    assert_eq!(layout.binary, Path::new("/data/maestro/qdrant/bin/qdrant"));
    assert_eq!(layout.storage, Path::new("/data/maestro/qdrant/storage"));
    assert_eq!(
        layout.snapshots,
        Path::new("/data/maestro/qdrant/snapshots")
    );
    assert_eq!(
        layout.unit,
        Path::new("/config/systemd/user/maestro-qdrant.service")
    );
}

#[test]
fn the_unit_runs_qdrant_on_the_loopback_with_telemetry_off() {
    assert_eq!(
        unit_text(&layout("/data/maestro"), &QDRANT).unwrap(),
        "# Written by `maestro setup`, which writes it again whenever it differs.\n\
         [Unit]\n\
         Description=Qdrant 1.19.1, the search service of Maestro\n\
         \n\
         [Service]\n\
         Type=simple\n\
         WorkingDirectory=/data/maestro/qdrant\n\
         ExecStart=\"/data/maestro/qdrant/bin/qdrant\" --disable-telemetry\n\
         Environment=\"QDRANT__SERVICE__HOST=127.0.0.1\"\n\
         Environment=\"QDRANT__SERVICE__HTTP_PORT=6333\"\n\
         Environment=\"QDRANT__SERVICE__GRPC_PORT=6334\"\n\
         Environment=\"QDRANT__STORAGE__STORAGE_PATH=/data/maestro/qdrant/storage\"\n\
         Environment=\"QDRANT__STORAGE__SNAPSHOTS_PATH=/data/maestro/qdrant/snapshots\"\n\
         Environment=\"QDRANT__TELEMETRY_DISABLED=true\"\n\
         Restart=on-failure\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    );
}

#[test]
fn a_path_systemd_would_read_otherwise_is_escaped() {
    let text = unit_text(&layout("/data/a (b) 100% & $HOME;#"), &QDRANT).unwrap();
    for line in [
        "WorkingDirectory=/data/a (b) 100%% & $HOME;#/qdrant\n",
        "ExecStart=\"/data/a (b) 100%% & $HOME;#/qdrant/bin/qdrant\" --disable-telemetry\n",
        "Environment=\"QDRANT__STORAGE__STORAGE_PATH=\
         /data/a (b) 100%% & $HOME;#/qdrant/storage\"\n",
    ] {
        assert!(text.contains(line), "{line} is missing from:\n{text}");
    }
}

#[test]
fn a_path_no_unit_can_hold_is_refused_naming_it() {
    for data in [
        OsStr::new("/data/two\nlines"),
        OsStr::new("/data/it's"),
        OsStr::new("/data/\"quoted\""),
        OsStr::new("/data/back\\slash"),
        OsStr::from_bytes(b"/data/not-utf-8-\xff"),
    ] {
        let layout = Layout::new(Path::new(data), Path::new("/config"));
        let refusal = unit_text(&layout, &QDRANT).unwrap_err();
        assert!(matches!(refusal, Failure::Refused(_)), "{refusal:?}");
        assert!(
            refusal.to_string().contains("/data/"),
            "names the path: {refusal}"
        );
    }
}
