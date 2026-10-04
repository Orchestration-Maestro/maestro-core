//! The native adapter's single rooted construction boundary.

use lbug::{Database, Error, RootDirectory, SystemConfig};

/// Opens one child of an already-held root; the fork validates every native operation.
pub(super) fn open(
    root: &RootDirectory,
    name: &str,
    config: SystemConfig,
) -> Result<Database, Error> {
    #[cfg(test)]
    tests::OPEN_CALLS.with(|calls| calls.set(calls.get() + 1));
    Database::new_rooted(root, name, config)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use lbug::{Connection, Value};
    use std::cell::Cell;

    thread_local! {
        /// Counts real database constructions in this test thread, not registry lookups.
        pub(in crate::graph::projection::engine)
        static OPEN_CALLS: Cell<usize> = const { Cell::new(0) };
    }
    use maestro_test_scratch::scratch_directory;
    #[cfg(windows)]
    use std::path::Path;
    use std::{fs, path::PathBuf};

    /// An owned, private scratch root, removed only after native handles drop.
    pub(in crate::graph::projection::engine) struct Scratch(
        pub(in crate::graph::projection::engine) PathBuf,
    );

    impl Scratch {
        pub(in crate::graph::projection::engine) fn new() -> Self {
            let directory = scratch_directory().unwrap();
            // Unix journals need resolved roots; Windows aliases need non-verbatim paths.
            #[cfg(unix)]
            let directory = fs::canonicalize(directory).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
            }
            #[cfg(windows)]
            private_windows_fixture(&directory);
            Self(directory)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    /// Small explicit native test limits, not product graphdb settings.
    pub(in crate::graph::projection::engine) fn config() -> SystemConfig {
        SystemConfig::default()
            .buffer_pool_size(16 * 1024 * 1024)
            .max_db_size(64 * 1024 * 1024)
            .max_num_threads(1)
    }

    /// Fixture ACLs only: the production adapter never repairs permissions.
    #[cfg(windows)]
    pub(in crate::graph::projection::engine) fn private_windows_fixture(path: &Path) {
        use std::process::Command;
        let script = r"
$ErrorActionPreference = 'Stop'
$path = $env:MAESTRO_TEST_PRIVATE_ROOT
$user = [Security.Principal.WindowsIdentity]::GetCurrent().User
$acl = [Security.AccessControl.DirectorySecurity]::new()
$acl.SetOwner($user)
$acl.SetAccessRuleProtection($true, $false)
foreach ($sid in @($user.Value, 'S-1-5-18', 'S-1-5-32-544')) {
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
        [Security.Principal.SecurityIdentifier]::new($sid), 'FullControl',
        'ContainerInherit,ObjectInherit', 'None', 'Allow'))
}
Set-Acl -LiteralPath $path -AclObject $acl
foreach ($file in Get-ChildItem -LiteralPath $path -File) {
    $fileAcl = Get-Acl -LiteralPath $file.FullName
    $fileAcl.SetOwner($user)
    Set-Acl -LiteralPath $file.FullName -AclObject $fileAcl
}
";
        let status = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env_remove("PSModulePath")
            .env("MAESTRO_TEST_PRIVATE_ROOT", path)
            .status()
            .unwrap();
        assert!(status.success(), "private Windows fixture ACL failed");
    }

    /// A committed disk row, created rooted except for the Windows reader fixture.
    fn populated_fixture() -> (Scratch, RootDirectory) {
        let scratch = Scratch::new();
        let root = RootDirectory::open(&scratch.0).unwrap();
        {
            // Windows rooted writes deliberately refuse in the qualified fork.
            // This one legacy construction prepares an immutable read-only fixture.
            #[cfg(windows)]
            #[expect(
                clippy::disallowed_methods,
                reason = "legacy unrooted read-only fixture creation; \
                          product opens use the rooted constructor"
            )]
            let database = Database::new(scratch.0.join("graph.lbdb"), config()).unwrap();
            #[cfg(not(windows))]
            let database = open(&root, "graph.lbdb", config()).unwrap();
            let connection = Connection::new(&database).unwrap();
            connection
                .query("CREATE NODE TABLE Item(id INT64, PRIMARY KEY(id))")
                .unwrap();
            connection.query("CREATE (:Item {id: 7})").unwrap();
            connection.query("CHECKPOINT").unwrap();
        }
        #[cfg(windows)]
        private_windows_fixture(&scratch.0);
        assert!(scratch.0.join("graph.lbdb").is_file());
        (scratch, root)
    }

    #[test]
    fn rooted_create_query_close_and_read_only_reopen() {
        let (scratch, root) = populated_fixture();
        let before = fs::read(scratch.0.join("graph.lbdb")).unwrap();
        for _ in 0..2 {
            let database = open(&root, "graph.lbdb", config().read_only(true)).unwrap();
            let connection = Connection::new(&database).unwrap();
            assert_eq!(
                connection
                    .query("MATCH (i:Item) RETURN i.id")
                    .unwrap()
                    .next()
                    .unwrap(),
                [Value::Int64(7)]
            );
            assert!(connection.query("CREATE (:Item {id: 8})").is_err());
        }
        assert_eq!(fs::read(scratch.0.join("graph.lbdb")).unwrap(), before);
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
    }

    #[test]
    fn rooted_open_refuses_extension_install() {
        let (_scratch, root) = populated_fixture();
        let database = open(&root, "graph.lbdb", config().read_only(true)).unwrap();
        let connection = Connection::new(&database).unwrap();
        let error = connection
            .query("INSTALL json;")
            .expect_err("rooted construction downloads no extensions")
            .to_string();
        assert!(
            error.contains("Rooted mode refuses extension install."),
            "{error}"
        );
    }

    // Linux maps prove loaded libraries; qualification inspects linkage on all hosts.
    #[cfg(target_os = "linux")]
    #[test]
    fn no_openssl_library_is_loaded() {
        let (_scratch, root) = populated_fixture();
        let _database = open(&root, "graph.lbdb", config().read_only(true)).unwrap();
        let maps = fs::read_to_string("/proc/self/maps").expect("process maps");
        for library in ["libssl", "libcrypto"] {
            assert!(!maps.contains(library), "{library} is loaded");
        }
    }

    #[test]
    fn rooted_open_refuses_non_child_names_without_touching_outside_bytes() {
        let (scratch, root) = populated_fixture();
        drop(open(&root, "graph.lbdb", config().read_only(true)).unwrap());
        let owned_path = scratch.0.join("graph.lbdb");
        let before = fs::read(&owned_path).unwrap();
        let outside = Scratch::new();
        let sentinel = outside.0.join("sentinel.lbdb");
        fs::write(&sentinel, b"outside sentinel").unwrap();
        for name in [
            "",
            ".",
            "..",
            "../sentinel.lbdb",
            "nested/file.lbdb",
            "./graph.lbdb",
            "nested/../graph.lbdb",
            ":memory:",
            owned_path.to_str().unwrap(),
            sentinel.to_str().unwrap(),
        ] {
            assert!(
                open(&root, name, config().read_only(true)).is_err(),
                "accepted {name:?}"
            );
        }
        assert_eq!(fs::read(sentinel).unwrap(), b"outside sentinel");
        assert_eq!(fs::read(&owned_path).unwrap(), before);
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
    }

    // E02 supports Windows rooted readers only; a writer must fail before I/O.
    #[cfg(windows)]
    #[test]
    fn windows_rooted_writable_open_refuses_without_creating_a_file() {
        let scratch = Scratch::new();
        let root = RootDirectory::open(&scratch.0).unwrap();
        let error = open(&root, "missing.lbdb", config()).unwrap_err();
        assert!(
            error.to_string().contains("writable database construction"),
            "{error}"
        );
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
    }
}
