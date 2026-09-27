//! Constructs the keyed SQLite fingerprint bank from private source snapshots.

use super::{
    bank::{self, Bank},
    normalize::{
        SHINGLE_SIZE, SHORT_UNIT_TAG, hex, key_id, shingle_tag, short_unit_tag, tag, tokens,
    },
};
use rusqlite::{Connection, Statement, Transaction, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process,
    result::Result as StdResult,
    str,
    time::{SystemTime, UNIX_EPOCH},
};

/// Internal error type for bank construction.
type Result<T> = StdResult<T, Box<dyn Error>>;
/// Kind tag for exact source-file fingerprints.
const FILE_TAG: u8 = 1;
/// Kind tag for normalized token-shingle fingerprints.
const SHINGLE_TAG: u8 = 2;

/// Counts emitted after a bank is built successfully.
#[derive(Debug)]
pub(super) struct BuildSummary {
    /// Number of inventoried files.
    pub files: usize,
    /// Total bytes in inventoried files.
    pub bytes: u64,
    /// Distinct stored fingerprints.
    pub fingerprints: u64,
    /// Complete private units of four to seven tokens.
    pub short_units: u64,
    /// Exact units suppressed by the private allowlist.
    pub allowed_units: u64,
}

/// Paths required to build one private bank.
#[derive(Clone, Copy)]
pub(super) struct BuildInputs<'a> {
    /// Mode-600 key file.
    pub key_path: &'a Path,
    /// NUL-delimited source file paths.
    pub input_list: &'a Path,
    /// NUL-delimited selected private prose fields for eight-token shingles.
    pub shingle_text_list: &'a Path,
    /// NUL-delimited selected private question units for short matching.
    pub short_unit_list: &'a Path,
    /// NUL-delimited allowlist unit/reason pairs.
    pub allowlist: &'a Path,
    /// Destination SQLite bank path.
    pub bank_path: &'a Path,
    /// Destination keyed digest path for the completed bank file.
    pub digest_out: &'a Path,
    /// Destination inventory identifier path.
    pub inventory_out: &'a Path,
}

/// Validated in-memory content used to construct a private bank.
struct BuildData {
    /// Secret key used for keyed fingerprints.
    key: [u8; 32],
    /// Canonically ordered source files.
    paths: Vec<PathBuf>,
    /// NUL-delimited selected prose fields for eight-token shingles.
    shingle_text_list: PathBuf,
    /// NUL-delimited selected question fields for short matching.
    short_unit_list: PathBuf,
    /// Exact allowlisted units with reasons validated separately.
    allowed_units: Vec<String>,
    /// Raw private allowlist bytes included in inventory identity.
    allowlist_bytes: Vec<u8>,
}

/// Builds a keyed SQLite bank from private files and selected text units.
pub(super) fn build(inputs: BuildInputs<'_>) -> Result<BuildSummary> {
    let data = load_build_data(inputs)?;
    let (temporary, mut connection) = create_bank(inputs.bank_path)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let (inventory_id, bytes) = populate_bank(&transaction, &data)?;
    let (fingerprints, short_units, allowed_units) =
        store_metadata(&transaction, &data.key, &inventory_id)?;
    transaction.commit()?;
    connection.execute_batch("PRAGMA optimize;")?;
    drop(connection);
    fs::rename(&temporary, inputs.bank_path)?;
    let digest = Bank::file_hmac(inputs.bank_path, &data.key)?;
    write_private_atomic(inputs.digest_out, digest.as_bytes())?;
    write_private_atomic(inputs.inventory_out, inventory_id.as_bytes())?;
    Bank::verify(
        inputs.bank_path,
        inputs.key_path,
        inputs.digest_out,
        &inventory_id,
    )?;
    Ok(BuildSummary {
        files: data.paths.len(),
        bytes,
        fingerprints,
        short_units,
        allowed_units,
    })
}

/// Loads and validates every external input before writing the bank.
fn load_build_data(inputs: BuildInputs<'_>) -> Result<BuildData> {
    let mut paths = input_paths(inputs.input_list)?;
    paths.sort();
    if paths.is_empty() || paths.windows(2).any(|pair| pair.first() == pair.get(1)) {
        return Err(bank::invalid().into());
    }
    bank::check_private_file(inputs.shingle_text_list)?;
    bank::check_private_file(inputs.short_unit_list)?;
    let (allowed_units, allowlist_bytes) = allowlist_units(inputs.allowlist)?;
    Ok(BuildData {
        key: bank::load_key(inputs.key_path)?,
        paths,
        shingle_text_list: inputs.shingle_text_list.to_path_buf(),
        short_unit_list: inputs.short_unit_list.to_path_buf(),
        allowed_units,
        allowlist_bytes,
    })
}

/// Creates the private SQLite schema in a unique sibling file.
fn create_bank(path: &Path) -> Result<(PathBuf, Connection)> {
    let temporary = temporary_path(path)?;
    let file = create_private_file(&temporary)?;
    drop(file);
    let connection = Connection::open(&temporary)?;
    connection.execute_batch(
        "PRAGMA journal_mode=DELETE;
         PRAGMA synchronous=FULL;
         CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
         CREATE TABLE fingerprints (
             kind INTEGER NOT NULL,
             length INTEGER NOT NULL,
             tag BLOB NOT NULL,
             PRIMARY KEY (kind, length, tag)
         ) WITHOUT ROWID;
         CREATE TABLE allowed_units (
             length INTEGER NOT NULL,
             tag BLOB NOT NULL,
             PRIMARY KEY (length, tag)
         ) WITHOUT ROWID;",
    )?;
    Ok((temporary, connection))
}

/// Adds source files, short units and allowlist tags in one transaction.
fn populate_bank(transaction: &Transaction<'_>, data: &BuildData) -> Result<(String, u64)> {
    let mut insert = transaction
        .prepare("INSERT OR IGNORE INTO fingerprints (kind, length, tag) VALUES (?1, ?2, ?3)")?;
    let mut inventory = Sha256::new();
    frame(&mut inventory, b"private-unit-allowlist");
    frame(&mut inventory, &data.allowlist_bytes);
    let mut total_bytes = 0_u64;
    for path in &data.paths {
        let length = insert_file(&mut insert, &mut inventory, &data.key, path)?;
        total_bytes = total_bytes.checked_add(length).ok_or_else(bank::invalid)?;
    }
    for_each_text(&data.shingle_text_list, |text| {
        add_text(&mut insert, &data.key, text)
    })?;
    for_each_text(&data.short_unit_list, |unit| {
        insert_short_unit(&mut insert, &data.key, unit)
    })?;
    drop(insert);
    insert_allowed_units(transaction, &data.key, &data.allowed_units)?;
    Ok((hex(&inventory.finalize()), total_bytes))
}

/// Stores one exact-file tag without indexing source metadata as prose.
fn insert_file(
    insert: &mut Statement<'_>,
    inventory: &mut Sha256,
    key: &[u8; 32],
    path: &Path,
) -> Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(bank::invalid().into());
    }
    let bytes = fs::read(path)?;
    let length = u64::try_from(bytes.len())?;
    frame(inventory, path.as_os_str().as_encoded_bytes());
    frame(inventory, &bytes);
    insert.execute(params![
        i64::from(FILE_TAG),
        i64::try_from(length)?,
        tag(key, FILE_TAG, length, &bytes).as_slice()
    ])?;
    Ok(length)
}

/// Inserts one complete selected question unit of four to seven tokens.
fn insert_short_unit(insert: &mut Statement<'_>, key: &[u8; 32], unit: &str) -> Result<()> {
    let words = tokens(unit);
    if (4..=7).contains(&words.len()) {
        insert.execute(params![
            i64::from(SHORT_UNIT_TAG),
            i64::try_from(words.len())?,
            short_unit_tag(key, &words).as_slice()
        ])?;
    }
    Ok(())
}

/// Stores only keyed tags for exact allowlisted units.
fn insert_allowed_units(
    transaction: &Transaction<'_>,
    key: &[u8; 32],
    units: &[String],
) -> Result<()> {
    let mut insert =
        transaction.prepare("INSERT OR IGNORE INTO allowed_units (length, tag) VALUES (?1, ?2)")?;
    for unit in units {
        let words = tokens(unit);
        insert.execute(params![
            i64::try_from(words.len())?,
            short_unit_tag(key, &words).as_slice()
        ])?;
    }
    Ok(())
}

/// Writes version metadata and keyed integrity before the transaction commits.
fn store_metadata(
    transaction: &Transaction<'_>,
    key: &[u8; 32],
    inventory_id: &str,
) -> Result<(u64, u64, u64)> {
    set_metadata(transaction, "format_version", bank::FORMAT_VERSION)?;
    set_metadata(
        transaction,
        "normalization_version",
        bank::NORMALIZATION_VERSION,
    )?;
    set_metadata(transaction, "key_id", &key_id(key))?;
    set_metadata(transaction, "inventory_id", inventory_id)?;
    let fingerprints: i64 =
        transaction.query_row("SELECT COUNT(*) FROM fingerprints", [], |row| row.get(0))?;
    let short_units: i64 = transaction.query_row(
        "SELECT COUNT(*) FROM fingerprints WHERE kind = ?1",
        [i64::from(SHORT_UNIT_TAG)],
        |row| row.get(0),
    )?;
    let allowed_units: i64 =
        transaction.query_row("SELECT COUNT(*) FROM allowed_units", [], |row| row.get(0))?;
    let bank_integrity = bank::integrity(transaction, key)?;
    set_metadata(transaction, "integrity", &bank_integrity)?;
    Ok((
        u64::try_from(fingerprints)?,
        u64::try_from(short_units)?,
        u64::try_from(allowed_units)?,
    ))
}

/// Inserts every normalized overlapping shingle from one text value.
fn add_text(insert: &mut Statement<'_>, key: &[u8; 32], text: &str) -> Result<()> {
    let words = tokens(text);
    for window in words.windows(SHINGLE_SIZE) {
        insert.execute(params![
            i64::from(SHINGLE_TAG),
            i64::try_from(SHINGLE_SIZE)?,
            shingle_tag(key, window).as_slice()
        ])?;
    }
    Ok(())
}

/// Reads NUL-delimited paths while preserving non-UTF-8 Unix names.
fn input_paths(list: &Path) -> Result<Vec<PathBuf>> {
    bank::check_private_file(list)?;
    let bytes = fs::read(list)?;
    let mut paths = Vec::new();
    for path in bytes
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        #[cfg(unix)]
        let path = {
            use std::{ffi::OsString, os::unix::ffi::OsStringExt};
            PathBuf::from(OsString::from_vec(path.to_vec()))
        };
        #[cfg(not(unix))]
        let path = PathBuf::from(String::from_utf8(path.to_vec())?);
        paths.push(path);
    }
    Ok(paths)
}

/// Streams NUL-delimited private text fields without retaining all contents.
fn for_each_text(path: &Path, mut consume: impl FnMut(&str) -> Result<()>) -> Result<()> {
    bank::check_private_file(path)?;
    let reader = BufReader::new(File::open(path)?);
    for field in reader.split(0) {
        let field = field?;
        if !field.is_empty() {
            consume(str::from_utf8(&field)?)?;
        }
    }
    Ok(())
}

/// Validates NUL-delimited exact-unit allowlist entries and their one-line reasons.
fn allowlist_units(path: &Path) -> Result<(Vec<String>, Vec<u8>)> {
    bank::check_private_file(path)?;
    let bytes = fs::read(path)?;
    let mut fields = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    if fields.len() % 2 != 0 {
        return Err(bank::invalid().into());
    }
    let mut units = Vec::new();
    for pair in fields.chunks(2) {
        let unit = String::from_utf8(pair.first().ok_or_else(bank::invalid)?.to_vec())?;
        let reason = String::from_utf8(pair.get(1).ok_or_else(bank::invalid)?.to_vec())?;
        let word_count = tokens(&unit).len();
        if unit.is_empty()
            || !(4..=7).contains(&word_count)
            || reason.trim().is_empty()
            || reason.contains(['\n', '\r'])
        {
            return Err(bank::invalid().into());
        }
        units.push(unit);
    }
    Ok((units, bytes))
}

/// Creates a new private file without following or replacing existing paths.
fn create_private_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

/// Writes and syncs private output before atomically replacing its destination.
fn write_private_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = temporary_path(path)?;
    let mut file = create_private_file(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, path)?;
    Ok(())
}

/// Generates a unique sibling path for an atomic private-file write.
fn temporary_path(path: &Path) -> Result<PathBuf> {
    let name = path.file_name().ok_or_else(bank::invalid)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(path.with_file_name(format!(
        ".{}-{}-{nonce}.tmp",
        name.to_string_lossy(),
        process::id()
    )))
}

/// Adds length-prefixed bytes to a digest to avoid ambiguous concatenation.
fn frame(hash: &mut Sha256, bytes: &[u8]) {
    hash.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hash.update(bytes);
}

/// Stores one bank metadata value inside the active transaction.
fn set_metadata(transaction: &Transaction<'_>, key: &str, value: &str) -> Result<()> {
    transaction.execute(
        "INSERT INTO metadata (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}
