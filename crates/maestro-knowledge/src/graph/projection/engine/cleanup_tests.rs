//! Native files/read-only handles for the feature-independent cleanup process suite.
use super::{
    backend::{Backend, Publication},
    reader::Reader,
    tests::config,
};
use crate::graph::projection::{
    ProjectionScope, content,
    writer::{ProjectionBackend, ProjectionBackendReader},
};
use lbug::RootDirectory;
use maestro_kernel::store::Database;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Test-only immutable single-file installation, as in the private backend contract.
struct Install(PathBuf);

impl Publication for Install {
    fn install(&mut self, staging: &str, published: &str) -> Result<(), String> {
        fs::hard_link(self.0.join(staging), self.0.join(published))
            .map_err(|error| error.to_string())?;
        fs::remove_file(self.0.join(staging)).map_err(|error| error.to_string())
    }
}

/// Build a real rooted native file before any cleanup reader enters the access domain.
pub(in crate::graph::projection) fn install(path: &Path, scope: &ProjectionScope, name: &str) {
    let mut backend = Backend::new(
        RootDirectory::open(path).unwrap(),
        RootDirectory::open(path).unwrap(),
        "cleanup-staging.lbdb".into(),
        config(),
        Install(path.to_path_buf()),
    );
    backend.create_unpublished(scope).unwrap();
    let found = backend.verify_unpublished(scope).unwrap();
    assert_eq!(found.content_digest, content::digest(&[], &[]).unwrap());
    assert_eq!(found.fact_count, 0);
    backend.publish_unpublished(scope, name).unwrap();
}

/// Hold an actual native read-only database in the independently synchronized reader process.
/// Every native handle remains beneath a held root, alongside the supported shared access guard.
pub(in crate::graph::projection) fn reader(
    path: &Path,
    generation: i64,
) -> impl ProjectionBackendReader {
    let kernel = Database::open_in(path.parent().unwrap()).unwrap();
    let scopes = kernel.visible("cleaner").unwrap();
    let receipt = kernel
        .projection_ready(&scopes, generation)
        .unwrap()
        .unwrap();
    let scope = ProjectionScope {
        collection_id: receipt.collection_id.clone(),
        generation_id: generation,
    };
    let root = RootDirectory::open(path).unwrap();
    let reader = Reader::published(&root, config(), &scope, &receipt).unwrap();
    assert_eq!(
        reader.verification().unwrap().content_digest,
        receipt.content_digest
    );
    reader
}
