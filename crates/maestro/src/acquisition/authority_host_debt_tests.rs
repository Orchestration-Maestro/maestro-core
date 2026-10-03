//! Descriptor and digest contracts, independent of live qualification.
use super::authority_host::{
    binary_digest, bounded, bytes, checked, protected, protected_file, runtime_digest,
    witness_state,
};
use super::authority_host_tests::fixture;
use maestro_kernel::artifact::Digest;
use rustix::{
    fs::{CWD, Mode, mkfifoat},
    process::geteuid,
};
use sha2::{Digest as _, Sha256};
use std::{
    env,
    fs::{self, File},
    os::unix::fs::{MetadataExt as _, PermissionsExt as _, symlink},
    sync::mpsc,
    thread,
    time::Duration,
};

/// A broken descriptor guard must fail here, not hang a later service test.
fn deadline<T: Send + 'static>(call: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        drop(sender.send(call()));
    });
    receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("authority descriptor operation failed or exceeded test deadline")
}

#[test]
fn debt_host_bounded_bytes_preserve_exact_limit() {
    deadline(|| {
        let (scratch, _, _) = fixture();
        let path = scratch.0.join("bytes");
        let expected = vec![b'x'; 16_384];
        fs::write(&path, &expected).unwrap();
        assert_eq!(bytes(&path).unwrap(), expected);
        assert_eq!(bounded(File::open(&path).unwrap()).unwrap(), expected);
        fs::write(&path, vec![b'x'; 16_385]).unwrap();
        assert!(bounded(File::open(&path).unwrap()).is_err());
    });
}

#[test]
fn debt_host_protected_descriptor_checks_each_condition() {
    deadline(|| {
        let (scratch, host, _) = fixture();
        let path = scratch.0.join("protected");
        fs::write(&path, b"canary").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(protected_file(&path, host.owner_uid).is_ok());
        assert!(protected_file(&path, host.owner_uid + 1).is_err());
        for mode in [0o620, 0o602] {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            assert!(
                protected_file(&path, host.owner_uid).is_err(),
                "mode {mode:o}"
            );
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(protected_file(&scratch.0, host.owner_uid).is_err());
        let link = scratch.0.join("linked");
        symlink(&path, &link).unwrap();
        assert!(protected_file(&link, host.owner_uid).is_err());
        let fifo = scratch.0.join("fifo");
        mkfifoat(CWD, &fifo, Mode::from_raw_mode(0o600)).unwrap();
        assert!(protected_file(&fifo, host.owner_uid).is_err());
    });
}

#[test]
fn debt_host_witness_and_executable_digests_are_exact() {
    deadline(|| {
        let (scratch, host, _) = fixture();
        let path = scratch.0.join("witness");
        let content = vec![b'q'; 9000];
        fs::write(&path, &content).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        assert_eq!(
            witness_state(&path, host.owner_uid).unwrap(),
            (
                geteuid().as_raw(),
                metadata.mode(),
                Digest::of(&content).as_str().into()
            )
        );
        assert_eq!(
            binary_digest(&path).unwrap(),
            Sha256::digest(&content).to_vec()
        );
        assert_eq!(
            runtime_digest().unwrap(),
            binary_digest(&env::current_exe().unwrap()).unwrap()
        );
    });
}

#[test]
fn debt_host_store_must_be_a_directory_even_with_private_mode() {
    let (scratch, mut host, config) = fixture();
    host.store = scratch.0.join("not-directory");
    fs::write(&host.store, b"regular file").unwrap();
    fs::set_permissions(&host.store, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(&config, serde_json::to_vec(&host).unwrap()).unwrap();
    assert!(checked(&config).is_err());
}

#[test]
fn debt_host_sticky_bit_does_not_protect_owner_writable_ancestors() {
    let (scratch, host, _) = fixture();
    // A sticky directory owned by this non-root caller is not a trusted scratch root.
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o1700)).unwrap();
    assert!(protected(&host.store, host.owner_uid).is_ok());
    fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o1777)).unwrap();
    assert!(protected(&host.store, host.owner_uid).is_err());
}

#[test]
fn debt_host_qualification_canary_changes_refuse_without_receipt() {
    use super::authority_host::qualify_with;
    for changed in [false, true] {
        let (_scratch, host, config) = fixture();
        let result = qualify_with(&config, Duration::from_secs(1), |host, _| {
            if changed {
                fs::write(host.store.join("probe-witness"), b"changed canary").unwrap();
            }
            Ok(())
        });
        assert_eq!(result.is_ok(), !changed);
        assert_eq!(host.store.join("qualification").exists(), !changed);
        assert!(!host.store.join("probe-witness").exists());
    }
}

#[test]
fn debt_host_probe_dispatch_requires_current_executable_digest() {
    use super::authority_host::{probes_with, qualify};
    use std::cell::RefCell;
    let (scratch, host, config) = fixture();
    let observed = RefCell::new(Vec::new());
    assert!(qualify(&config, Duration::from_secs(1)).is_err());
    assert!(
        probes_with(&host, Duration::from_secs(1), |_, _| {
            panic!("mismatched executable dispatched")
        })
        .is_err()
    );
    fs::copy(env::current_exe().unwrap(), scratch.0.join("maestro")).unwrap();
    fs::set_permissions(scratch.0.join("maestro"), fs::Permissions::from_mode(0o600)).unwrap();
    assert!(
        probes_with(&host, Duration::from_secs(1), |probe, _| {
            observed.borrow_mut().push(probe.uid);
            Ok(())
        })
        .is_ok()
    );
    assert_eq!(*observed.borrow(), [host.pipeline_uid, host.connector_uid]);
}

#[test]
fn debt_host_probe_requires_all_three_denials_and_delivers_report() {
    use super::authority_host::probe;
    use std::{
        io::{BufRead as _, BufReader},
        os::unix::net::UnixListener,
    };
    for (create, edit, delete) in [
        (false, true, true),
        (true, false, true),
        (true, true, false),
        (true, true, true),
    ] {
        let (scratch, host, _) = fixture();
        let listener = UnixListener::bind(&host.socket).unwrap();
        let created = host.store.join("probe-created");
        let witness = host.store.join("probe-witness");
        if create {
            fs::write(&created, b"existing").unwrap();
        }
        if !edit || !delete {
            fs::write(&witness, b"canary").unwrap();
        }
        if edit && witness.exists() {
            fs::set_permissions(&witness, fs::Permissions::from_mode(0o400)).unwrap();
        }
        if delete {
            fs::set_permissions(&host.store, fs::Permissions::from_mode(0o500)).unwrap();
        }
        // A writable store is required for the create-allowed case.
        if !create {
            fs::set_permissions(&host.store, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let result = probe(&host.store, &host.socket);
        fs::set_permissions(&host.store, fs::Permissions::from_mode(0o700)).unwrap();
        let (peer, _) = listener.accept().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut line = String::new();
        BufReader::new(peer).read_line(&mut line).unwrap();
        let observed: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            observed,
            serde_json::json!({"create_denied":create,"edit_denied":edit,"delete_denied":delete})
        );
        assert_eq!(result.is_ok(), create && edit && delete);
        drop(scratch);
    }
}

#[test]
fn debt_host_protected_metadata_sticky_root_requires_all_three_properties() {
    use super::authority_host::protected_metadata;
    for (directory, uid, mode, accepted) in [
        (true, 0, 0o1777, true),
        (true, 0, 0o777, false),
        (true, 1000, 0o1777, false),
        (false, 0, 0o1777, false),
        (true, 0, 0o755, true),
        (true, 1000, 0o700, true),
    ] {
        assert_eq!(
            protected_metadata(directory, uid, mode, false, 1000),
            accepted,
            "dir={directory} uid={uid} mode={mode:o}"
        );
    }
}

#[test]
fn debt_host_open_flags_are_disjoint() {
    use rustix::fs::OFlags;
    let flags = [OFlags::RDONLY, OFlags::NOFOLLOW, OFlags::NONBLOCK];
    for left in flags {
        for right in flags {
            if left != right {
                assert!((left & right).is_empty());
            }
        }
    }
    assert_eq!(
        flags[0] | flags[1] | flags[2],
        flags[0] ^ flags[1] ^ flags[2]
    );
}

#[test]
fn debt_host_qualified_refuses_special_receipt_before_reading() {
    use super::authority_host::qualified;
    deadline(|| {
        let (_scratch, host, _) = fixture();
        mkfifoat(
            CWD,
            host.store.join("qualification"),
            Mode::from_raw_mode(0o600),
        )
        .unwrap();
        assert!(qualified(&host).is_err());
    });
}
