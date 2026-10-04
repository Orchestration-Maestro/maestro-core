//! Actual delegated-cgroup effects; ownership, parsing and rollback stay in drivers.
use super::port::CgroupIo;
use std::{fs, io, path::Path};

/// Mandatory production adapter; no ordinary-process fallback.
#[derive(Debug)]
pub(super) struct HostIo;
impl CgroupIo for HostIo {
    fn read(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }
    fn write(&self, path: &Path, value: &str) -> io::Result<()> {
        fs::write(path, value)
    }
    fn create(&self, path: &Path) -> io::Result<()> {
        fs::create_dir(path)
    }
    fn remove(&self, path: &Path) -> io::Result<()> {
        fs::remove_dir(path)
    }
    fn probe_kill(&self, path: &Path) -> io::Result<()> {
        fs::OpenOptions::new().write(true).open(path).map(|_| ())
    }
}
