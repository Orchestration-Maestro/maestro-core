//! The command line: its usage, its flags, the metadata sidecar and the run that canonicalizes,
//! saves and summarizes one Markdown file.
use crate::local_assets::asset_inventory;
use maestro_canonicalization::{
    CanonicalizeInput, Error, ExtractorBlock, ParserOptions, SourceMetadata, ValidationStatus,
    canonicalize, save_document,
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// The command line the tool accepts, printed for `--help` and on a usage error.
pub(crate) const USAGE: &str = concat!(
    "usage: maestro-canonicalization INPUT.md --output DIRECTORY [--metadata SIDECAR.json] ",
    "[--document-id ID] [--identity-key KEY]"
);

/// The flags that take a value; each may appear once.
const FLAGS: [&str; 4] = ["--output", "--metadata", "--document-id", "--identity-key"];

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
pub(crate) struct Args {
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

/// Read the input path and the flag pairs; an unknown, repeated or empty flag is a usage error.
pub(crate) fn parse_args(args: &[String]) -> Result<Args, Error> {
    let Some((input, flags)) = args
        .split_first()
        .filter(|(input, _)| !input.starts_with('-'))
    else {
        return Err(Error(USAGE.into()));
    };
    let mut values = BTreeMap::new();
    let (pairs, remainder) = flags.as_chunks::<2>();
    for [flag, value] in pairs {
        if !FLAGS.contains(&flag.as_str())
            || value.is_empty()
            || values.insert(flag.as_str(), value.as_str()).is_some()
        {
            return Err(Error(USAGE.into()));
        }
    }
    if !remainder.is_empty() {
        return Err(Error(USAGE.into()));
    }
    let output = values.get("--output").ok_or_else(|| Error(USAGE.into()))?;
    Ok(Args {
        input: input.into(),
        output: output.into(),
        metadata: values.get("--metadata").map(PathBuf::from),
        document_id: values.get("--document-id").map(|&id| id.into()),
        identity_key: values
            .get("--identity-key")
            .copied()
            .unwrap_or(input)
            .into(),
    })
}

/// Canonicalize the input with its sidecar, resolve its local assets, save the snapshot and print a
/// JSON summary.
pub(crate) fn run(args: &Args) -> Result<ValidationStatus, Error> {
    let markdown = fs::read_to_string(&args.input)
        .map_err(|error| Error(format!("cannot read UTF-8 Markdown: {error}")))?;
    let sidecar = match &args.metadata {
        Some(path) => read_sidecar(path)?,
        None => Sidecar::default(),
    };
    if args
        .document_id
        .as_ref()
        .zip(sidecar.document_id.as_ref())
        .is_some_and(|(given, supplied)| given != supplied)
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
            .filter(|parent| !parent.as_os_str().is_empty())
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
        serde_json::to_string(&summary).map_err(|error| Error(error.to_string()))?
    );
    Ok(doc.validation_status)
}

/// The metadata sidecar at `path`, read strictly.
fn read_sidecar(path: &Path) -> Result<Sidecar, Error> {
    let bytes =
        fs::read(path).map_err(|error| Error(format!("cannot read metadata sidecar: {error}")))?;
    // Deserialize JSON with the existing strict mapping visitor first:
    // serde_json::Value alone silently overwrites duplicate policy keys.
    serde_json::from_slice::<serde_yaml_ng::Value>(&bytes)
        .map_err(|error| Error(format!("invalid metadata sidecar: {error}")))?;
    serde_json::from_slice::<Sidecar>(&bytes)
        .map_err(|error| Error(format!("invalid metadata sidecar: {error}")))
}
