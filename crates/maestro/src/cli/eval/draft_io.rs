//! Allocation-bounded reads for private drafting inputs.
use std::{
    fs::File,
    io::{self, Read as _},
    path::Path,
};

/// Read private bytes within the caller's allocation budget.
pub(super) fn read(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let limit = u64::try_from(limit).map_err(io::Error::other)?;
    if file.metadata()?.len() > limit {
        return Err(io::Error::other("private draft file exceeds limit"));
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::other("private draft file exceeds limit"));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::read;
    use std::{
        env,
        fs::{self, File},
        io::Write as _,
    };
    use ulid::Ulid;

    #[test]
    fn private_draft_reads_accept_exact_fit_and_refuse_oversized_files() {
        let path = env::temp_dir().join(format!("draft-io-{}", Ulid::generate()));
        let mut file = File::create(&path).unwrap();
        file.write_all(b"abcd").unwrap();
        assert_eq!(read(&path, 4).unwrap(), b"abcd");
        assert!(read(&path, 3).is_err());
        file.set_len(1 << 34).unwrap();
        assert!(read(&path, 4).is_err());
        drop(file);
        fs::remove_file(path).unwrap();
    }
}
