//! Reads Git object graphs without checking out candidate content.

use std::{
    collections::{BTreeSet, HashMap},
    error::Error,
    ffi::OsStr,
    io::{self, Write},
    path::Path,
    process::{Command, Stdio},
    result::Result as StdResult,
    str,
};

/// Internal error type for Git plumbing operations.
type Result<T> = StdResult<T, Box<dyn Error>>;

/// Git object contents returned by batch reads.
#[derive(Debug)]
pub(super) struct Object {
    /// Full object ID.
    pub(super) oid: String,
    /// Git object kind.
    pub(super) kind: String,
    /// Raw object contents.
    pub(super) bytes: Vec<u8>,
}

/// Blob ID and byte-preserving path from a committed tree.
#[derive(Debug)]
pub(super) struct TreeEntry {
    /// Full blob object ID.
    pub(super) oid: String,
    /// Raw tree path bytes.
    pub(super) path: Vec<u8>,
}

/// Refuses a scan when the repository does not define its selected remote.
pub(super) fn check_remote(repository: &Path, remote: &str) -> Result<()> {
    drop(git(repository, &["remote", "get-url", remote])?);
    Ok(())
}

/// Queries whether the repository has complete history.
pub(super) fn shallow(repository: &Path) -> Result<bool> {
    let output = git(repository, &["rev-parse", "--is-shallow-repository"])?;
    Ok(text(&output)? == "true")
}

/// Reads the declared Git object type for a full object ID.
pub(super) fn object_type(repository: &Path, oid: &str) -> Result<String> {
    text(&git(repository, &["cat-file", "-t", oid])?)
}

/// Resolves a commit or annotated tag to its peeled commit ID.
pub(super) fn peeled_commit(repository: &Path, oid: &str) -> Result<String> {
    let expression = format!("{oid}^{{commit}}");
    let commit = text(&git(repository, &["rev-parse", "--verify", &expression])?)?;
    if !valid_oid(&commit) {
        return Err(invalid().into());
    }
    Ok(commit)
}

/// Lists objects reachable from the source ref and absent from its base.
pub(super) fn revision_objects(
    repository: &Path,
    source: &str,
    base: Option<&str>,
    remote: &str,
) -> Result<BTreeSet<String>> {
    let mut args = vec![
        "rev-list".to_owned(),
        "--objects".to_owned(),
        "--no-object-names".to_owned(),
        "--missing=print".to_owned(),
        source.to_owned(),
    ];
    args.push("--not".to_owned());
    args.push(format!("--remotes={remote}"));
    if let Some(base) = base {
        args.push(base.to_owned());
    }
    let bytes = git(repository, &args)?;
    let mut objects = BTreeSet::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        if line.first() == Some(&b'?') {
            return Err(invalid().into());
        }
        let oid = text(line)?;
        if !valid_oid(&oid) {
            return Err(invalid().into());
        }
        objects.insert(oid);
    }
    Ok(objects)
}

/// Reads validated commit, tag and blob contents through `git cat-file`.
pub(super) fn read_objects(repository: &Path, ids: &BTreeSet<String>) -> Result<Vec<Object>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let input = object_input(ids);
    let types = git_with_input(
        repository,
        &["cat-file", "--batch-check=%(objectname) %(objecttype)"],
        input.as_bytes(),
    )?;
    let mut requested = Vec::new();
    let mut seen = BTreeSet::new();
    for line in types
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let mut fields = line.split(u8::is_ascii_whitespace);
        let oid = text(fields.next().ok_or_else(invalid)?)?;
        let kind = text(fields.next().ok_or_else(invalid)?)?;
        if !valid_oid(&oid)
            || !seen.insert(oid.clone())
            || !matches!(kind.as_str(), "blob" | "commit" | "tag" | "tree")
        {
            return Err(invalid().into());
        }
        if kind != "tree" {
            requested.push((oid, kind));
        }
    }
    if &seen != ids {
        return Err(invalid().into());
    }
    if requested.is_empty() {
        return Ok(Vec::new());
    }
    let requested_ids: BTreeSet<String> = requested.iter().map(|(oid, _)| oid.clone()).collect();
    let batch_input = object_input(&requested_ids);
    let batch_output =
        git_with_input(repository, &["cat-file", "--batch"], batch_input.as_bytes())?;
    let content = parse_batch(&batch_output)?;
    if content.len() != requested.len() {
        return Err(invalid().into());
    }
    let mut objects = Vec::with_capacity(requested.len());
    for (oid, expected_kind) in requested {
        let (kind, bytes) = content.get(&oid).ok_or_else(invalid)?;
        if kind != &expected_kind {
            return Err(invalid().into());
        }
        objects.push(Object {
            oid,
            kind: expected_kind,
            bytes: bytes.clone(),
        });
    }
    Ok(objects)
}

/// Encodes a sorted object-ID set for Git's line-oriented batch input.
fn object_input(ids: &BTreeSet<String>) -> String {
    let mut input = String::new();
    for oid in ids {
        input.push_str(oid);
        input.push('\n');
    }
    input
}

/// Parses `cat-file --batch` output with checked object boundaries.
fn parse_batch(bytes: &[u8]) -> Result<HashMap<String, (String, Vec<u8>)>> {
    let mut objects = HashMap::new();
    let mut offset = 0_usize;
    while offset < bytes.len() {
        let remaining = bytes.get(offset..).ok_or_else(invalid)?;
        let header_length = remaining
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or_else(invalid)?;
        let header = str::from_utf8(remaining.get(..header_length).ok_or_else(invalid)?)?;
        let header_end = offset.checked_add(header_length).ok_or_else(invalid)?;
        let mut fields = header.split_whitespace();
        let oid = fields.next().ok_or_else(invalid)?.to_owned();
        let kind = fields.next().ok_or_else(invalid)?.to_owned();
        let length = fields.next().ok_or_else(invalid)?.parse::<usize>()?;
        if !valid_oid(&oid) || !matches!(kind.as_str(), "blob" | "commit" | "tag") {
            return Err(invalid().into());
        }
        let start = header_end.checked_add(1).ok_or_else(invalid)?;
        let end = start.checked_add(length).ok_or_else(invalid)?;
        if bytes.get(end) != Some(&b'\n') {
            return Err(invalid().into());
        }
        let object = bytes.get(start..end).ok_or_else(invalid)?.to_vec();
        objects.insert(oid, (kind, object));
        offset = end.checked_add(1).ok_or_else(invalid)?;
    }
    Ok(objects)
}

/// Lists committed blob IDs and raw paths without checking out the tree.
pub(super) fn tree_entries(repository: &Path, commit: &str) -> Result<Vec<TreeEntry>> {
    let bytes = git(repository, &["ls-tree", "-rz", "-r", "--full-tree", commit])?;
    let mut entries = Vec::new();
    for entry in bytes
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let separator = entry
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(invalid)?;
        let metadata = entry.get(..separator).ok_or_else(invalid)?;
        let name_start = separator.checked_add(1).ok_or_else(invalid)?;
        let path = entry.get(name_start..).ok_or_else(invalid)?;
        let mut fields = metadata.split(u8::is_ascii_whitespace);
        let mode = fields.next().ok_or_else(invalid)?;
        let kind = fields.next().ok_or_else(invalid)?;
        let oid = text(fields.next().ok_or_else(invalid)?)?;
        if kind != b"blob" || mode == b"160000" || !valid_oid(&oid) {
            return Err(invalid().into());
        }
        entries.push(TreeEntry {
            oid,
            path: path.to_vec(),
        });
    }
    Ok(entries)
}

/// Runs one Git plumbing command with repository-override variables removed.
fn git<S: AsRef<OsStr>>(repository: &Path, args: &[S]) -> Result<Vec<u8>> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repository)
        .args(args)
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .stderr(Stdio::null());
    let output = command.output()?;
    if !output.status.success() {
        return Err(invalid().into());
    }
    Ok(output.stdout)
}

/// Runs Git plumbing with explicit stdin and sanitized repository state.
fn git_with_input(repository: &Path, args: &[&str], input: &[u8]) -> Result<Vec<u8>> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repository)
        .args(args)
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn()?;
    let mut stdin = child.stdin.take().ok_or_else(invalid)?;
    stdin.write_all(input)?;
    drop(stdin);
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(invalid().into());
    }
    Ok(output.stdout)
}

/// Converts Git plumbing output to trimmed UTF-8 text.
pub(super) fn text(bytes: &[u8]) -> Result<String> {
    Ok(str::from_utf8(bytes)?.trim().to_owned())
}

/// Checks whether a full SHA-1 or SHA-256 object ID is hexadecimal.
pub(super) fn valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Creates the generic error used to refuse invalid Git input.
pub(super) fn invalid() -> io::Error {
    io::Error::other("invalid git input")
}
