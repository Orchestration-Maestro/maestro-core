//! Parses privacy CLI arguments and returns location-only output.

use super::{
    bank,
    bank_build::{self, BuildInputs},
    git::{self, RefSpec},
};
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fmt::Write as _,
    io,
    path::PathBuf,
    result::Result as StdResult,
};

/// Internal error type for privacy command argument and I/O failures.
type Result<T> = StdResult<T, io::Error>;

/// Parses arguments and returns an exit code with location-only output.
///
/// # Errors
/// Invalid arguments or any unavailable, incomplete or invalid privacy data.
pub fn run(arguments: &[OsString]) -> StdResult<(u8, String), io::Error> {
    let Some((command, rest)) = arguments.split_first() else {
        return Err(invalid());
    };
    match command.to_str() {
        Some("build-bank") => build_bank(rest),
        Some("scan-git") => scan_git(rest),
        Some("verify-bank") => verify_bank(rest),
        _ => Err(invalid()),
    }
}

/// Builds a bank and returns only aggregate counts.
fn build_bank(arguments: &[OsString]) -> Result<(u8, String)> {
    let options = options(arguments, &[])?;
    reject_unknown(
        &options,
        &[
            "--key-file",
            "--input-list",
            "--shingle-text-list",
            "--short-unit-list",
            "--allowlist",
            "--bank",
            "--digest-out",
            "--inventory-out",
        ],
    )?;
    let key_path = required_path(&options, "--key-file")?;
    let input_list = required_path(&options, "--input-list")?;
    let shingle_text_list = required_path(&options, "--shingle-text-list")?;
    let short_unit_list = required_path(&options, "--short-unit-list")?;
    let allowlist = required_path(&options, "--allowlist")?;
    let bank_path = required_path(&options, "--bank")?;
    let digest_out = required_path(&options, "--digest-out")?;
    let inventory_out = required_path(&options, "--inventory-out")?;
    let summary = bank_build::build(BuildInputs {
        key_path: &key_path,
        input_list: &input_list,
        shingle_text_list: &shingle_text_list,
        short_unit_list: &short_unit_list,
        allowlist: &allowlist,
        bank_path: &bank_path,
        digest_out: &digest_out,
        inventory_out: &inventory_out,
    })
    .map_err(|_| invalid())?;
    let mut output = String::new();
    writeln!(
        &mut output,
        "bank built files={} bytes={} fingerprints={} short_units={} allowed_units={}",
        summary.files,
        summary.bytes,
        summary.fingerprints,
        summary.short_units,
        summary.allowed_units
    )
    .map_err(io::Error::other)?;
    Ok((0, output))
}

/// Fully verifies one bank's SQLite structure and logical-row integrity.
fn verify_bank(arguments: &[OsString]) -> Result<(u8, String)> {
    let options = options(arguments, &[])?;
    reject_unknown(
        &options,
        &[
            "--bank",
            "--bank-digest-file",
            "--key-file",
            "--inventory-id",
        ],
    )?;
    bank::Bank::verify(
        &required_path(&options, "--bank")?,
        &required_path(&options, "--key-file")?,
        &required_path(&options, "--bank-digest-file")?,
        &required_string(&options, "--inventory-id")?,
    )
    .map_err(|_| invalid())?;
    Ok((0, "bank verified\n".to_owned()))
}

/// Scans explicit outgoing refs and returns location-only findings.
fn scan_git(arguments: &[OsString]) -> Result<(u8, String)> {
    let options = options(arguments, &["--ref"])?;
    reject_unknown(
        &options,
        &[
            "--repo",
            "--remote",
            "--bank",
            "--bank-digest-file",
            "--key-file",
            "--inventory-id",
            "--ref",
        ],
    )?;
    let repository = required_path(&options, "--repo")?;
    let remote = required_string(&options, "--remote")?;
    let bank_path = required_path(&options, "--bank")?;
    let digest_path = required_path(&options, "--bank-digest-file")?;
    let key_path = required_path(&options, "--key-file")?;
    let inventory_id = required_string(&options, "--inventory-id")?;
    let references = options
        .get("--ref")
        .ok_or_else(invalid)?
        .iter()
        .map(|value| {
            let value = value.to_str().ok_or_else(invalid)?;
            RefSpec::parse(value, &remote).map_err(|_| invalid())
        })
        .collect::<Result<Vec<_>>>()?;
    let report = git::scan(
        &repository,
        &references,
        git::BankFiles {
            bank: &bank_path,
            digest: &digest_path,
            key: &key_path,
            inventory: &inventory_id,
        },
    )
    .map_err(|_| invalid())?;
    let mut output = String::new();
    for finding in &report.details {
        writeln!(
            &mut output,
            "ref={} object={} unit={} line={} bytes={}-{} hits=1",
            finding.reference,
            finding.object,
            finding.unit,
            finding.line,
            finding.start,
            finding.end
        )
        .map_err(io::Error::other)?;
    }
    writeln!(
        &mut output,
        "scan complete refs={} objects={} hits={} details={}",
        report.refs,
        report.objects,
        report.hits,
        report.details.len()
    )
    .map_err(io::Error::other)?;
    Ok((u8::from(report.hits != 0), output))
}

/// Parses option/value pairs, allowing repetition only for named options.
fn options(arguments: &[OsString], repeated: &[&str]) -> Result<BTreeMap<String, Vec<OsString>>> {
    let mut result: BTreeMap<String, Vec<OsString>> = BTreeMap::new();
    let (pairs, remainder) = arguments.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(invalid());
    }
    for pair in pairs {
        let name = pair
            .first()
            .map(OsString::as_os_str)
            .and_then(OsStr::to_str)
            .ok_or_else(invalid)?;
        let value = pair.get(1).ok_or_else(invalid)?.clone();
        if !name.starts_with("--") {
            return Err(invalid());
        }
        let values = result.entry(name.to_owned()).or_default();
        if !values.is_empty() && !repeated.contains(&name) {
            return Err(invalid());
        }
        values.push(value);
    }
    Ok(result)
}

/// Rejects options not explicitly accepted by the current subcommand.
fn reject_unknown(options: &BTreeMap<String, Vec<OsString>>, allowed: &[&str]) -> Result<()> {
    if options.keys().any(|name| !allowed.contains(&name.as_str())) {
        return Err(invalid());
    }
    Ok(())
}

/// Converts a required option value into a filesystem path.
fn required_path(options: &BTreeMap<String, Vec<OsString>>, name: &str) -> Result<PathBuf> {
    let value = options
        .get(name)
        .and_then(|values| values.first())
        .ok_or_else(invalid)?;
    Ok(PathBuf::from(value))
}

/// Converts a required option value into UTF-8 text.
fn required_string(options: &BTreeMap<String, Vec<OsString>>, name: &str) -> Result<String> {
    options
        .get(name)
        .and_then(|values| values.first())
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .ok_or_else(invalid)
}

/// Constructs the generic refusal error for malformed arguments.
fn invalid() -> io::Error {
    io::Error::other("invalid privacy arguments")
}
