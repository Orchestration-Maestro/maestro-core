//! Local CLI for Markdown canonicalization. No network clients or model calls.
//! Exit codes: 0 usable, 2 retained but invalid, 1 execution or input refusal.
#![forbid(unsafe_code)]
use maestro_canonicalization::{
    AssetStatus, CanonicalDocument, CanonicalizeInput, Error, ExtractorBlock, ParserOptions,
    SourceMetadata, ValidationStatus, canonicalize, save_document,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    process::ExitCode,
};

/// The command line the tool accepts, printed for `--help` and on a usage error.
const USAGE: &str = concat!(
    "usage: maestro-canonicalization INPUT.md --output DIRECTORY [--metadata SIDECAR.json] ",
    "[--document-id ID] [--identity-key KEY]"
);

/// The optional metadata sidecar: JSON with only these keys, each optional.
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Sidecar {
    /// A stable document identifier; it must agree with `--document-id` when both are given.
    document_id: Option<String>,
    /// Provenance, title, language and policy supplied for the source.
    source_metadata: SourceMetadata,
    /// Run details kept with the document and left out of every identity.
    operational_metadata: BTreeMap<String, serde_json::Value>,
    /// Structure an upstream extractor produced, anchored to Markdown spans.
    extractor_blocks: Vec<ExtractorBlock>,
    /// The parser extensions to enable.
    parser_options: ParserOptions,
}

/// The parsed command line.
struct Args {
    /// The Markdown file to canonicalize.
    input: PathBuf,
    /// The directory that receives the immutable snapshot.
    output: PathBuf,
    /// The metadata sidecar, if one is named.
    metadata: Option<PathBuf>,
    /// A stable document identifier, if one is given.
    document_id: Option<String>,
    /// What a document identifier derives from when none is given: the input path unless
    /// `--identity-key` names another key.
    identity_key: String,
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    match parse_args(&args).and_then(|args| run(&args)) {
        Ok(status) => {
            if status == ValidationStatus::Failed {
                ExitCode::from(2)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Read the input path and the flag pairs; an unknown, repeated or empty flag is a usage error.
fn parse_args(args: &[String]) -> Result<Args, Error> {
    let Some(input) = args.first().filter(|s| !s.starts_with('-')) else {
        return Err(Error(USAGE.into()));
    };
    let mut values = BTreeMap::new();
    let mut pairs = args[1..].chunks_exact(2);
    for pair in &mut pairs {
        if !["--output", "--metadata", "--document-id", "--identity-key"]
            .contains(&pair[0].as_str())
            || pair[1].is_empty()
            || values.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err(Error(USAGE.into()));
        }
    }
    if !pairs.remainder().is_empty() {
        return Err(Error(USAGE.into()));
    }
    let output = values.get("--output").ok_or_else(|| Error(USAGE.into()))?;
    Ok(Args {
        input: input.into(),
        output: output.into(),
        metadata: values.get("--metadata").map(PathBuf::from),
        document_id: values.get("--document-id").map(|s| (*s).into()),
        identity_key: values
            .get("--identity-key")
            .copied()
            .unwrap_or(input)
            .into(),
    })
}

/// Canonicalize the input with its sidecar, resolve its local assets, save the snapshot and print a
/// JSON summary.
fn run(args: &Args) -> Result<ValidationStatus, Error> {
    let markdown = fs::read_to_string(&args.input)
        .map_err(|e| Error(format!("cannot read UTF-8 Markdown: {e}")))?;
    let sidecar = match &args.metadata {
        Some(path) => {
            let bytes =
                fs::read(path).map_err(|e| Error(format!("cannot read metadata sidecar: {e}")))?;
            // Deserialize JSON with the existing strict mapping visitor first:
            // serde_json::Value alone silently overwrites duplicate policy keys.
            serde_json::from_slice::<serde_yaml_ng::Value>(&bytes)
                .map_err(|e| Error(format!("invalid metadata sidecar: {e}")))?;
            serde_json::from_slice::<Sidecar>(&bytes)
                .map_err(|e| Error(format!("invalid metadata sidecar: {e}")))?
        }
        None => Sidecar::default(),
    };
    if args
        .document_id
        .as_ref()
        .zip(sidecar.document_id.as_ref())
        .is_some_and(|(a, b)| a != b)
    {
        return Err(Error("conflicting CLI and sidecar document IDs".into()));
    }
    let mut input = CanonicalizeInput::new(&markdown, &args.identity_key);
    input.document_id = args
        .document_id
        .as_deref()
        .or(sidecar.document_id.as_deref());
    input.metadata = sidecar.source_metadata;
    input.operational_metadata = sidecar.operational_metadata;
    input.extractor_blocks = sidecar.extractor_blocks;
    input.parser_options = sidecar.parser_options;
    let parsed = canonicalize(input.clone())?;
    input.assets = asset_inventory(
        &parsed,
        args.input
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    let doc = canonicalize(input)?;
    let path = save_document(&doc, &markdown, &args.output)?;
    let summary = serde_json::json!({
        "canonical_json": path,
        "validation_status": doc.validation_status,
        "blocks": doc.blocks.len(),
        "findings": doc.warnings.len()
    });
    println!(
        "{}",
        serde_json::to_string(&summary).map_err(|e| Error(e.to_string()))?
    );
    Ok(doc.validation_status)
}

/// The status of each unchecked asset destination, resolved under the input's directory.
fn asset_inventory(
    doc: &CanonicalDocument,
    root: &Path,
) -> Result<BTreeMap<String, AssetStatus>, Error> {
    let root =
        fs::canonicalize(root).map_err(|e| Error(format!("cannot resolve asset root: {e}")))?;
    let destinations: BTreeSet<_> = doc
        .blocks
        .iter()
        .flat_map(|b| &b.asset_references)
        .filter(|a| a.status == AssetStatus::Unchecked)
        .map(|a| a.destination.clone())
        .collect();
    Ok(destinations
        .into_iter()
        .map(|url| {
            let status = local_asset_status(&url, &root);
            (url, status)
        })
        .collect())
}

/// Whether a destination names a regular file under the root: available, missing, outside the root,
/// or unchecked when it cannot be decoded or resolved.
fn local_asset_status(destination: &str, root: &Path) -> AssetStatus {
    let without_suffix = destination.split(['?', '#']).next().unwrap_or("");
    let Some(decoded) = percent_decode(without_suffix) else {
        return AssetStatus::Unchecked;
    };
    if decoded.is_empty() {
        return AssetStatus::Unchecked;
    }
    let mut relative = PathBuf::new();
    for component in Path::new(&decoded).components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir if relative.pop() => {}
            _ => return AssetStatus::OutsideRoot,
        }
    }
    match fs::canonicalize(root.join(relative)) {
        Ok(path) if !path.starts_with(root) => AssetStatus::OutsideRoot,
        Ok(path) if path.is_file() => AssetStatus::Available,
        Ok(_) => AssetStatus::Missing,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => AssetStatus::Missing,
        Err(_) => AssetStatus::Unchecked,
    }
}

/// Decode percent escapes into UTF-8 text; a malformed escape, invalid UTF-8 or a NUL gives
/// nothing.
fn percent_decode(text: &str) -> Option<String> {
    let mut bytes = text.bytes();
    let mut decoded = Vec::with_capacity(text.len());
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = char::from(bytes.next()?).to_digit(16)?;
            let low = char::from(bytes.next()?).to_digit(16)?;
            decoded.push(u8::try_from(high * 16 + low).ok()?);
        } else {
            decoded.push(byte);
        }
    }
    String::from_utf8(decoded)
        .ok()
        .filter(|s| !s.contains('\0'))
}
