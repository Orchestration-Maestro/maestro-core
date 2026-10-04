//! Each admitted image flag has an independent observable contract.
use super::{
    launch::{image_file, read, sealed},
    port::Refusal,
};
use maestro_test_scratch::scratch_directory;
use rustix::{
    fs::{SealFlags, fcntl_add_seals},
    io::{FdFlags, fcntl_getfd},
};
use std::fs;

#[test]
fn n17_writable_modes_refuse_before_staging_charge() {
    let scratch = scratch_directory().unwrap();
    fs::create_dir_all(&scratch).unwrap();
    let path = scratch.join("image");
    fs::write(&path, b"exact bytes").unwrap();
    for readable in [false, true] {
        let mut file = fs::OpenOptions::new()
            .read(readable)
            .write(true)
            .open(&path)
            .unwrap();
        let mut remaining = 100;
        assert_eq!(read(&mut file, &mut remaining), Err(Refusal::Configuration));
        assert_eq!(
            remaining, 100,
            "writable handles refuse before staging charge"
        );
    }
    fs::remove_dir_all(scratch).unwrap();
}
#[test]
fn n17_sealed_image_refuses_growth_and_later_seals() {
    let file = sealed(b"exact bytes").unwrap();
    assert!(file.set_len(12).is_err());
    assert!(fcntl_add_seals(&file, SealFlags::FUTURE_WRITE).is_err());
}
#[test]
fn n17_fresh_image_is_cloexec_and_permits_sealing() {
    let file = image_file().unwrap();
    assert!(fcntl_getfd(&file).unwrap().contains(FdFlags::CLOEXEC));
    fcntl_add_seals(&file, SealFlags::WRITE).unwrap();
}
