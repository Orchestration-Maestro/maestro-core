//! The client reads the server's version from its health check.

use super::{backends::backends, fake::FAKE_VERSION};

#[tokio::test]
async fn the_client_reads_the_version_the_health_check_answers() {
    for backend in backends("the_client_reads_the_version_the_health_check_answers") {
        let version = backend.client().version().await.unwrap();
        if backend.fake.is_some() {
            assert_eq!(version, FAKE_VERSION);
        } else {
            assert!(version.starts_with("1.19."), "{}: {version}", backend.name);
        }
    }
}
