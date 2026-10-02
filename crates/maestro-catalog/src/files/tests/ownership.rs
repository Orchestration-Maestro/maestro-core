//! Committed metadata bounds are independent of injected output byte ceilings.
use super::support::{apply, preview, with_trust};
use crate::{
    files::{FileInput, FilePlan},
    limits::Limits,
};
use maestro_test_scratch::scratch_directory;
use std::fs;

#[test]
fn committed_record_byte_boundary_accepts_limit_and_refuses_one_over() {
    let root = scratch_directory().unwrap();
    let bytes = b"captured lock".to_vec();
    let relative = ".maestro/authoring.lock.json";
    let plan = preview(&root, [FileInput::new(relative, bytes.clone())]).unwrap();
    apply(&root, &plan).unwrap();
    let path = fs::read_dir(root.join(".maestro-files"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("ownership-")
        })
        .unwrap();
    let original = fs::read(&path).unwrap();
    // First isolate the aggregate ceiling; then the independent production file ceiling.
    for (size, aggregate) in [
        (
            u64::try_from(original.len()).unwrap(),
            u64::try_from(original.len()).unwrap(),
        ),
        (
            Limits::PRODUCTION.source_file_bytes,
            Limits::PRODUCTION.archive_total_bytes,
        ),
    ] {
        let mut record = original.clone();
        record.resize(usize::try_from(size).unwrap(), b' ');
        fs::write(&path, &record).unwrap();
        let limits = Limits {
            source_file_bytes: 1,
            archive_total_bytes: aggregate,
            ..Limits::PRODUCTION
        };
        with_trust(&root, |trust| {
            assert!(FilePlan::committed_file(&root, relative, &bytes, trust, &limits).is_ok());
            record.push(b' ');
            fs::write(&path, &record).unwrap();
            let error =
                FilePlan::committed_file(&root, relative, &bytes, trust, &limits).unwrap_err();
            assert!(error.to_string().contains("larger than"), "{error}");
        });
    }
    fs::remove_dir_all(root).unwrap();
}
