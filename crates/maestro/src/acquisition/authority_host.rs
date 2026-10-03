//! Linux host qualification, kept outside the serving path.
use super::authority_probe::{self, Probe, Report};
use crate::failure::Failure;
use maestro_kernel::{artifact::Digest, retrieval::SystemClock};
use rustix::{
    fs::{Mode, OFlags, open},
    process::geteuid,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::env;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    os::unix::{
        fs::{MetadataExt as _, OpenOptionsExt as _},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

/// Trusted host bindings: source manifests cannot choose identities or files.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Host {
    /// Separately owned, private authority directory.
    pub(super) store: PathBuf,
    /// Socket in an owner-controlled, traversable directory.
    pub(super) socket: PathBuf,
    /// Owner/authority peer authenticated by the kernel.
    pub(super) owner_uid: u32,
    /// Actual separately confined pipeline identity.
    pub(super) pipeline_uid: u32,
    /// Actual separately confined connector identity.
    pub(super) connector_uid: u32,
    /// Admin-authorized identity launcher, never used by serve.
    pub(super) launcher: PathBuf,
}
/// Fixed content-free qualification refusal.
pub(super) fn unqualified() -> Failure {
    Failure::refused("authority unqualified")
}
/// Bound small local input before decoding; duplicate struct keys refuse in serde.
pub(super) fn read<T: DeserializeOwned>(path: &Path) -> Result<T, Failure> {
    let bytes = bytes(path)?;
    serde_json::from_slice(&bytes).map_err(|_| unqualified())
}
/// Bound reads before allocation, including malicious local request files.
fn bytes(path: &Path) -> Result<Vec<u8>, Failure> {
    bounded(File::open(path).map_err(|_| unqualified())?)
}
/// Read a previously validated descriptor, never reopen the path.
fn bounded(file: File) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::new();
    file.take(16_385)
        .read_to_end(&mut bytes)
        .map_err(|_| unqualified())?;
    if bytes.len() > 16_384 {
        return Err(unqualified());
    }
    Ok(bytes)
}
/// Owner confirmation/inspection bytes must be protected, unlike untrusted client decisions.
pub(super) fn read_protected<T: DeserializeOwned>(path: &Path, owner: u32) -> Result<T, Failure> {
    let file = protected_file(path, owner)?;
    serde_json::from_slice(&bounded(file)?).map_err(|_| unqualified())
}
/// No symlink traversal or blocking special file; fstat checks the same descriptor read later.
fn protected_file(path: &Path, owner: u32) -> Result<File, Failure> {
    let file = File::from(
        open(
            path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|_| unqualified())?,
    );
    let metadata = file.metadata().map_err(|_| unqualified())?;
    if !metadata.is_file() || metadata.uid() != owner || metadata.mode() & 0o022 != 0 {
        return Err(unqualified());
    }
    Ok(file)
}
/// Canary identity and digest are observed through one protected opened descriptor.
fn witness_state(path: &Path, owner: u32) -> Result<(u32, u32, String), Failure> {
    let file = protected_file(path, owner)?;
    let metadata = file.metadata().map_err(|_| unqualified())?;
    Ok((
        metadata.uid(),
        metadata.mode(),
        Digest::of(&bounded(file)?).as_str().into(),
    ))
}
/// Check non-root distinct identities and the protected directory/launcher chain.
pub(super) fn checked(path: &Path) -> Result<Host, Failure> {
    let host: Host = read(path)?;
    separated(&host, geteuid().as_raw())?;
    protected(&host.store, host.owner_uid)?;
    let metadata = fs::symlink_metadata(&host.store).map_err(|_| unqualified())?;
    if !metadata.is_dir() || metadata.uid() != host.owner_uid || metadata.mode() & 0o7777 != 0o700 {
        return Err(unqualified());
    }
    protected(&host.launcher, host.owner_uid)?;
    let launcher = fs::symlink_metadata(&host.launcher).map_err(|_| unqualified())?;
    if !launcher.is_file() || launcher.mode() & 0o111 == 0 {
        return Err(unqualified());
    }
    protected(
        host.socket.parent().ok_or_else(unqualified)?,
        host.owner_uid,
    )?;
    Ok(host)
}
/// Identity invariants independent of filesystem state and source configuration.
fn separated(host: &Host, actual_uid: u32) -> Result<(), Failure> {
    if host.owner_uid == 0
        || host.pipeline_uid == 0
        || host.connector_uid == 0
        || host.owner_uid != actual_uid
        || host.owner_uid == host.pipeline_uid
        || host.owner_uid == host.connector_uid
        || host.pipeline_uid == host.connector_uid
    {
        return Err(unqualified());
    }
    Ok(())
}
/// Reject symlinks and writable ancestors; root-owned sticky scratch roots are safe.
fn protected(path: &Path, owner: u32) -> Result<(), Failure> {
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|_| unqualified())?;
        let sticky_root = metadata.is_dir() && metadata.uid() == 0 && metadata.mode() & 0o1000 != 0;
        if metadata.file_type().is_symlink()
            || (metadata.uid() != owner && metadata.uid() != 0)
            || (metadata.mode() & 0o022 != 0 && !sticky_root)
        {
            return Err(unqualified());
        }
    }
    Ok(())
}
/// Digest binds identities, paths, owner/modes and exact launcher bytes.
pub(super) fn binding(host: &Host) -> Result<String, Failure> {
    let metadata = fs::symlink_metadata(&host.store).map_err(|_| unqualified())?;
    let launcher = fs::symlink_metadata(&host.launcher).map_err(|_| unqualified())?;
    let config = json!({
        "host":host,
        "store_owner":metadata.uid(),
        "store_mode":metadata.mode(),
        "store_device":metadata.dev(),
        "store_inode":metadata.ino(),
        "launcher_owner":launcher.uid(),
        "launcher_mode":launcher.mode(),
        "launcher_digest":Digest::of(&bytes(&host.launcher)?).as_str(),
        "probe_digest":binary_digest(&probe_binary(host)?)?,
        "runtime_digest":runtime_digest()?});
    Ok(
        Digest::of(&serde_json::to_vec(&config).map_err(|_| unqualified())?)
            .as_str()
            .into(),
    )
}
/// Cache only this process's immutable executable; mutable probe/launcher bytes rehash.
fn runtime_digest() -> Result<Vec<u8>, Failure> {
    static DIGEST: OnceLock<Result<Vec<u8>, ()>> = OnceLock::new();
    DIGEST
        .get_or_init(|| {
            let path = env::current_exe().map_err(|_| ())?;
            binary_digest(&path).map_err(|_| ())
        })
        .as_ref()
        .cloned()
        .map_err(|()| unqualified())
}

/// Qualify using real subprocess identities, retaining a protected matching receipt.
pub(super) fn qualify(path: &Path, budget: Duration) -> Result<Value, Failure> {
    let host = checked(path)?;
    let receipt = host.store.join("qualification");
    // A failed requalification invalidates any previous receipt first.
    if receipt.exists() {
        fs::remove_file(&receipt).map_err(|_| unqualified())?;
    }
    let witness = host.store.join("probe-witness");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&witness)
        .map_err(|_| unqualified())?;
    file.write_all(b"authority-probe")
        .map_err(|_| unqualified())?;
    file.sync_all().map_err(|_| unqualified())?;
    drop(file);
    let original = witness_state(&witness, host.owner_uid)?;
    let result = probes(&host, budget);
    let untouched = witness_state(&witness, host.owner_uid).is_ok_and(|state| state == original);
    fs::remove_file(&witness).map_err(|_| unqualified())?;
    result?;
    if !untouched {
        return Err(unqualified());
    }
    let digest = binding(&host)?;
    let mut receipt = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(receipt)
        .map_err(|_| unqualified())?;
    receipt
        .write_all(digest.as_bytes())
        .and_then(|()| receipt.sync_all())
        .map_err(|_| unqualified())?;
    File::open(&host.store)
        .and_then(|dir| dir.sync_all())
        .map_err(|_| unqualified())?;
    Ok(json!({"schema":"maestro-cli/authority/1","status":"qualified","binding":digest}))
}
/// Real probe completions authenticate through the kernel; launcher stdout is diagnostic only.
fn probes(host: &Host, budget: Duration) -> Result<(), Failure> {
    let binary = probe_binary(host)?;
    if binary_digest(&binary)? != runtime_digest()? {
        return Err(unqualified());
    }
    for uid in [host.pipeline_uid, host.connector_uid] {
        authority_probe::qualify(
            &Probe {
                launcher: &host.launcher,
                binary: &binary,
                store: &host.store,
                endpoint: host.socket.with_extension("probe"),
                uid,
            },
            budget,
            &SystemClock,
        )?;
    }
    Ok(())
}
/// The exact separately launched probe binary, protected against substitution.
fn probe_binary(host: &Host) -> Result<PathBuf, Failure> {
    let binary = host
        .launcher
        .parent()
        .ok_or_else(unqualified)?
        .join("maestro");
    protected(&binary, host.owner_uid)?;
    Ok(binary)
}
/// Stream binary identity without materializing large executables in memory.
fn binary_digest(path: &Path) -> Result<Vec<u8>, Failure> {
    let mut file = File::open(path).map_err(|_| unqualified())?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let count = file.read(&mut buffer).map_err(|_| unqualified())?;
        if count == 0 {
            break;
        }
        digest.update(buffer.get(..count).ok_or_else(unqualified)?);
    }
    Ok(digest.finalize().to_vec())
}
/// Actual unprivileged create/edit/delete checks; callers cannot manufacture the UID.
pub(super) fn probe(store: &Path, endpoint: &Path) -> Result<Value, Failure> {
    let create = store.join("probe-created");
    let witness = store.join("probe-witness");
    let mut stream = UnixStream::connect(endpoint).map_err(|_| unqualified())?;
    let report = Report {
        create_denied: OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&create)
            .is_err(),
        edit_denied: OpenOptions::new().write(true).open(&witness).is_err(),
        delete_denied: fs::remove_file(&witness).is_err(),
    };
    authority_probe::send(&mut stream, &report)?;
    if !report.create_denied || !report.edit_denied || !report.delete_denied {
        return Err(unqualified());
    }
    let uid = geteuid().as_raw();
    Ok(json!({"schema":"maestro-cli/authority/1","status":"probe_denied","uid":uid}))
}
/// Refuse missing, altered, linked or overly accessible qualification receipts.
pub(super) fn qualified(host: &Host) -> Result<(), Failure> {
    let path = host.store.join("qualification");
    let metadata = fs::symlink_metadata(&path).map_err(|_| unqualified())?;
    if !metadata.is_file()
        || metadata.uid() != host.owner_uid
        || metadata.mode() & 0o7777 != 0o600
        || bytes(&path)? != binding(host)?.as_bytes()
    {
        return Err(unqualified());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Host, binding, checked, probe, protected, qualified, separated};
    use maestro_test_scratch::scratch_directory;
    use rustix::process::geteuid;
    use std::{
        fs,
        os::unix::{
            fs::{PermissionsExt as _, symlink},
            net::UnixListener,
        },
        path::{Path, PathBuf},
    };

    /// Only pure metadata/identity tests; never runs a launcher or qualifies live grants.
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn fixture() -> (Scratch, Host, PathBuf) {
        let scratch = Scratch(scratch_directory().unwrap());
        let store = scratch.0.join("store");
        fs::create_dir(&store).unwrap();
        fs::set_permissions(&store, fs::Permissions::from_mode(0o700)).unwrap();
        let launcher = scratch.0.join("launcher");
        fs::write(&launcher, "synthetic metadata only").unwrap();
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(
            scratch.0.join("maestro"),
            "synthetic probe bytes, never executable",
        )
        .unwrap();
        fs::set_permissions(scratch.0.join("maestro"), fs::Permissions::from_mode(0o600)).unwrap();
        let host = Host {
            store,
            socket: scratch.0.join("socket"),
            owner_uid: geteuid().as_raw(),
            pipeline_uid: 65534,
            connector_uid: 65533,
            launcher,
        };
        let config = scratch.0.join("config.json");
        fs::write(&config, serde_json::to_vec(&host).unwrap()).unwrap();
        (scratch, host, config)
    }
    #[test]
    fn n05_host_identity_separation_has_no_root_or_same_user_fallback() {
        let (_scratch, mut host, _config) = fixture();
        assert!(separated(&host, host.owner_uid).is_ok());
        let original = host.owner_uid;
        for identities in [
            [0, 65534, 65533],
            [original, 0, 65533],
            [original, 65534, 0],
            [original, original, 65533],
            [original, 65534, original],
            [original, 65534, 65534],
            [original + 1, 65534, 65533],
        ] {
            host.owner_uid = identities[0];
            host.pipeline_uid = identities[1];
            host.connector_uid = identities[2];
            assert!(separated(&host, original).is_err());
        }
    }
    #[test]
    fn n05_host_metadata_and_configuration_are_strict_before_launch() {
        let (scratch, host, config) = fixture();
        assert!(checked(&config).is_ok());
        for mode in [0o755, 0o770, 0o1700] {
            fs::set_permissions(&host.store, fs::Permissions::from_mode(mode)).unwrap();
            assert!(checked(&config).is_err());
        }
        fs::set_permissions(&host.store, fs::Permissions::from_mode(0o700)).unwrap();
        for mode in [0o600, 0o720] {
            fs::set_permissions(&host.launcher, fs::Permissions::from_mode(mode)).unwrap();
            assert!(checked(&config).is_err());
        }
        fs::set_permissions(&host.launcher, fs::Permissions::from_mode(0o700)).unwrap();
        let link = scratch.0.join("link");
        symlink(&host.store, &link).unwrap();
        assert!(protected(&link, host.owner_uid).is_err());
        assert!(protected(&host.store, host.owner_uid + 1).is_err());
        assert!(protected(Path::new("relative"), host.owner_uid).is_err());
        fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(protected(&host.store, host.owner_uid).is_err());
        fs::set_permissions(&scratch.0, fs::Permissions::from_mode(0o700)).unwrap();
        let original = fs::read(&config).unwrap();
        let mut oversized = original.clone();
        oversized.resize(16_385, b' ');
        fs::write(&config, oversized).unwrap();
        assert!(checked(&config).is_err());
        fs::write(&config, original).unwrap();
        let mut value = serde_json::to_value(&host).unwrap();
        value["model_approval"] = true.into();
        fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(checked(&config).is_err());
        fs::write(&config, "{\"owner_uid\":1000,\"owner_uid\":1000}").unwrap();
        assert!(checked(&config).is_err());
        // Being writable by the actual owner is explicitly not a passing separation probe.
        fs::write(host.store.join("probe-witness"), "authority-probe").unwrap();
        let _listener = UnixListener::bind(&host.socket).unwrap();
        assert!(probe(&host.store, &host.socket).is_err());
    }
    #[test]
    fn n05_receipts_bind_current_host_metadata_and_protected_bytes() {
        let (_scratch, mut host, _config) = fixture();
        let receipt = host.store.join("qualification");
        assert!(qualified(&host).is_err());
        fs::write(&receipt, binding(&host).unwrap()).unwrap();
        fs::set_permissions(&receipt, fs::Permissions::from_mode(0o600)).unwrap();
        // Tests only the receipt validator; never starts a server with fabricated evidence.
        assert!(qualified(&host).is_ok());
        fs::set_permissions(&receipt, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(qualified(&host).is_err());
        fs::set_permissions(&receipt, fs::Permissions::from_mode(0o600)).unwrap();
        host.connector_uid = 65532;
        assert!(qualified(&host).is_err());
        host.connector_uid = 65533;
        fs::write(&host.launcher, "substituted launcher").unwrap();
        assert!(qualified(&host).is_err());
        fs::write(&receipt, "forged").unwrap();
        assert!(qualified(&host).is_err());
        fs::remove_file(&receipt).unwrap();
        symlink(&host.launcher, &receipt).unwrap();
        assert!(qualified(&host).is_err());
    }
}
