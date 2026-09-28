//! G25 route B: the patched `lbug` (`Orchestration-Maestro/lbug`, default
//! features off) builds without its extension installer, so it neither
//! downloads extensions nor links `OpenSSL`.

#[cfg(target_os = "linux")]
use std::fs::read_to_string;

use lbug::{Connection, Database, SystemConfig};

#[test]
fn install_fails_without_the_installer() {
    let db = Database::in_memory(SystemConfig::default()).expect("in-memory database");
    let conn = Connection::new(&db).expect("connect");
    let error = conn
        .query("INSTALL json;")
        .expect_err("a build without the installer downloads nothing")
        .to_string();
    assert!(error.contains("has no extension installer"), "{error}");
}

#[cfg(target_os = "linux")]
#[test]
fn no_openssl_library_is_loaded() {
    let maps = read_to_string("/proc/self/maps").expect("process maps");
    for library in ["libssl", "libcrypto"] {
        assert!(!maps.contains(library), "{library} is loaded");
    }
}
