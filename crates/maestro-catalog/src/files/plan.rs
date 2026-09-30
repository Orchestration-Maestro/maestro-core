//! Preview immutable file bytes, validate relative names, and bind content digests.
use maestro_filesystem::Directory;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io,
    path::{Component, Path, PathBuf},
    str,
};

/// One intended file and its bytes at preview time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileInput {
    /// Root-relative, slash-separated file name.
    pub path: String,
    /// The complete bytes to write.
    pub bytes: Vec<u8>,
}

/// An immutable, digest-bound set of files previewed against an empty target set.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FilePlan {
    /// Digest of the ordered relative paths and intended bytes.
    pub(super) id: String,
    /// Files in deterministic path order.
    pub(super) entries: Vec<PlannedFile>,
    /// Whether this exact plan already has committed ownership.
    pub(super) applied: bool,
}

/// Existing committed ownership fields needed for a replayed plan.
#[derive(Deserialize)]
struct ExistingOwnership {
    /// The immutable plan identity.
    id: String,
    /// Planned paths and their content digests.
    files: Vec<ExistingOwnedFile>,
}

/// One file entry in an existing ownership record.
#[derive(Deserialize)]
struct ExistingOwnedFile {
    /// Root-relative file name.
    path: String,
    /// SHA-256 digest.
    digest: String,
}

/// One canonical file path and its intended bytes and digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct PlannedFile {
    /// The normalized root-relative file name.
    pub(super) path: String,
    /// Exact bytes encoded as hex in the recovery journal.
    #[serde(with = "byte_string")]
    pub(super) bytes: Vec<u8>,
    /// SHA-256 of `bytes`.
    pub(super) digest: String,
}

impl FileInput {
    /// Construct a file input for a root-relative path.
    pub fn new(path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            bytes: bytes.into(),
        }
    }
}

impl FilePlan {
    /// Preview new files, refusing collisions and paths that are not normalized relatives.
    ///
    /// # Errors
    /// Returns an error for invalid paths, collisions, duplicate names, or filesystem failures.
    pub fn preview(root: &Path, inputs: impl IntoIterator<Item = FileInput>) -> io::Result<Self> {
        let mut inputs: Vec<_> = inputs.into_iter().collect();
        inputs.sort_by(|left, right| left.path.cmp(&right.path));
        let mut paths = BTreeSet::new();
        let mut folded_paths = BTreeSet::new();
        for input in &inputs {
            validate_relative_path(&input.path)?;
            if !folded_paths.insert(input.path.to_ascii_lowercase()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "duplicate planned path",
                ));
            }
            if !paths.insert(input.path.clone()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "duplicate planned path",
                ));
            }
        }
        let mut entries = Vec::with_capacity(inputs.len());
        let mut has_existing_target = false;
        for input in inputs {
            validate_no_ancestor_conflicts(&input.path, &paths)?;
            let (parent, name) = split_path(&input.path)?;
            match Directory::open(root, &parent, false) {
                Ok(directory) => match directory.read_regular(name) {
                    Ok(_) => has_existing_target = true,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                },
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            entries.push(PlannedFile {
                digest: digest(&input.bytes),
                path: input.path,
                bytes: input.bytes,
            });
        }
        if entries.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty file plan",
            ));
        }
        let id = plan_identity(&entries)?;
        let mut plan = Self {
            id,
            entries,
            applied: false,
        };
        plan.applied = is_committed_and_unchanged(root, &plan)?;
        if has_existing_target && !plan.applied {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "preview target exists without matching committed ownership",
            ));
        }
        Ok(plan)
    }

    /// Stable digest identifying this exact plan.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Whether this preview exactly replays files already owned by the same plan.
    #[must_use]
    pub const fn is_applied(&self) -> bool {
        self.applied
    }
}

/// Check the committed record and held-handle bytes for an exact plan replay.
/// This only returns an already applied state; removal separately checks identity before unlinking.
fn is_committed_and_unchanged(root: &Path, plan: &FilePlan) -> io::Result<bool> {
    let state = match Directory::open(root, Path::new(".maestro-files"), false) {
        Ok(state) => state,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let name = format!("ownership-{}.toml", plan.id);
    let record = match state.read_regular(&name) {
        Ok(record) => record,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !ownership_matches(&record, plan)? {
        return Ok(false);
    }
    for entry in &plan.entries {
        let (parent, name) = split_path(&entry.path)?;
        let directory = Directory::open(root, &parent, false)?;
        if digest(&directory.read_regular(name)?) != entry.digest {
            return Err(io::Error::other(format!(
                "owned file changed: {}",
                entry.path
            )));
        }
    }
    Ok(true)
}

/// Compare a committed ownership record's identity and planned byte digests.
pub(super) fn ownership_matches(bytes: &[u8], plan: &FilePlan) -> io::Result<bool> {
    let text =
        str::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let existing: ExistingOwnership =
        toml::from_str(text).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(existing.id == plan.id
        && existing.files.len() == plan.entries.len()
        && existing
            .files
            .iter()
            .zip(&plan.entries)
            .all(|(owned, entry)| owned.path == entry.path && owned.digest == entry.digest))
}

/// Compute a lowercase SHA-256 digest with its algorithm prefix.
pub(super) fn digest(bytes: &[u8]) -> String {
    let mut digest = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        digest.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        digest.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    digest
}

/// Split a validated slash-separated file name into parent and leaf components.
pub(super) fn split_path(path: &str) -> io::Result<(PathBuf, &str)> {
    let target = Path::new(path);
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid planned path"))?;
    let parent = target.parent().unwrap_or_else(|| Path::new(""));
    Ok((parent.to_path_buf(), name))
}

/// Encode arbitrary file bytes as journal-safe hexadecimal text.
mod byte_string {
    use serde::{Deserialize, Deserializer, Serializer, de};
    use std::str;

    /// Serialize bytes as a TOML-compatible hexadecimal string.
    pub(super) fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        let mut encoded = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            encoded.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
            encoded.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
        }
        serializer.serialize_str(&encoded)
    }

    /// Decode a validated hexadecimal journal string into exact bytes.
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<u8>, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        if encoded.len() % 2 != 0 {
            return Err(de::Error::custom("invalid file bytes"));
        }
        let mut decoded = Vec::with_capacity(encoded.len() / 2);
        for pair in encoded.as_bytes().as_chunks::<2>().0 {
            let digits = str::from_utf8(pair).map_err(de::Error::custom)?;
            decoded.push(u8::from_str_radix(digits, 16).map_err(de::Error::custom)?);
        }
        Ok(decoded)
    }
}

/// Refuse absolute, traversing, empty, or platform-ambiguous relative file names.
pub(super) fn validate_relative_path(path: &str) -> io::Result<()> {
    if path.is_empty()
        || path.contains('\\')
        || path
            .split('/')
            .next()
            .is_some_and(|part| part.eq_ignore_ascii_case(".maestro-files"))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsafe planned path",
        ));
    }
    for part in path.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains(':')
            || part.ends_with('.')
            || part.ends_with(' ')
            || is_windows_device_name(part)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsafe planned path",
            ));
        }
    }
    for component in Path::new(path).components() {
        if let Component::Normal(_) = component {
            continue;
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "unsafe planned path",
        ));
    }
    Ok(())
}

/// Refuse DOS device names, whose device stem remains special even with an extension.
fn is_windows_device_name(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or(component);
    let bytes = stem.as_bytes();
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
    ) || bytes.len() == 4
        && (bytes
            .get(..3)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"COM"))
            || bytes
                .get(..3)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"LPT")))
        && bytes
            .get(3)
            .is_some_and(|digit| matches!(digit, b'1'..=b'9'))
}

/// Validate an externally supplied plan identifier before using it in a state-file name.
pub(super) fn validate_id(id: &str) -> io::Result<()> {
    let Some(digits) = id.strip_prefix("sha256:") else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid file plan id",
        ));
    };
    if digits.len() != 64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid file plan id",
        ));
    }
    for digit in digits.bytes() {
        if !digit.is_ascii_digit() && !(b'a'..=b'f').contains(&digit) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid file plan id",
            ));
        }
    }
    Ok(())
}

/// Validate journal contents before they are allowed to cause filesystem effects.
pub(super) fn validate_plan(plan: &FilePlan) -> io::Result<()> {
    let mut paths = BTreeSet::new();
    let mut folded_paths = BTreeSet::new();
    for entry in &plan.entries {
        validate_relative_path(&entry.path)?;
        if !folded_paths.insert(entry.path.to_ascii_lowercase()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate journal path",
            ));
        }
        if !paths.insert(entry.path.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate journal path",
            ));
        }
        if digest(&entry.bytes) != entry.digest {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "journal digest mismatch",
            ));
        }
    }
    if plan.entries.is_empty() || plan_identity(&plan.entries)? != plan.id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "journal plan identity mismatch",
        ));
    }
    Ok(())
}

/// Reject two planned paths where one must be both a file and a directory.
fn validate_no_ancestor_conflicts(path: &str, paths: &BTreeSet<String>) -> io::Result<()> {
    let mut parent_path = path;
    while let Some((parent, _)) = parent_path.rsplit_once('/') {
        if paths.contains(parent) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "planned file is an ancestor of another file",
            ));
        }
        parent_path = parent;
    }
    Ok(())
}

/// Hash length-prefixed paths and contents so concatenation cannot alias another plan.
fn plan_identity(entries: &[PlannedFile]) -> io::Result<String> {
    let mut identity = Vec::new();
    for entry in entries {
        let path_length = u64::try_from(entry.path.len())
            .map_err(|_| io::Error::other("plan path exceeds supported length"))?;
        let bytes_length = u64::try_from(entry.bytes.len())
            .map_err(|_| io::Error::other("planned file exceeds supported size"))?;
        identity.extend_from_slice(&path_length.to_be_bytes());
        identity.extend_from_slice(entry.path.as_bytes());
        identity.extend_from_slice(&bytes_length.to_be_bytes());
        identity.extend_from_slice(&entry.bytes);
    }
    Ok(digest(&identity))
}
