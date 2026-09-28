//! A kernel's scratch directory, which may sit on tmpfs, is gone once the
//! kernel is dropped: a leak there would hold memory until a reboot.

use super::kernel::Kernel;
use maestro_test_scratch::scratch_root;

#[test]
fn a_kernel_removes_its_scratch_directory_when_dropped() {
    let kernel = Kernel::with_guides(1);
    let directory = kernel.database_path().parent().unwrap().to_path_buf();
    assert!(directory.starts_with(scratch_root().unwrap()));
    assert!(directory.join("kernel.sqlite3").is_file());
    drop(kernel);
    assert!(!directory.exists(), "{}", directory.display());
}
