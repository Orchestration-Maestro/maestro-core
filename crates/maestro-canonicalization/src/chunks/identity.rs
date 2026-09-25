//! Prepared-input groups: identical prepared inputs share one identity.
use super::batch::PreparedInputGroup;
use super::validation::invalid_chunks;
use crate::dedup::Deduplication;
use crate::document::CanonicalDocument;
use crate::error::Error;
use crate::hashing::digest;
use crate::prepared_inputs::{ChunkContent, PREPARATION_PROFILE};
use crate::source_units::{CHUNKER_VERSION, MappedDocument};
use serde::Serialize;
use std::collections::BTreeMap;

/// The identities of one prepared input, the grouping key of identical inputs.
pub(super) struct PreparedIdentity {
    /// `sha256:` of the profile, the counter's contract and the prepared input.
    pub(super) fingerprint: String,
    /// The serialized record the fingerprint digests.
    pub(super) bytes: Vec<u8>,
    /// The scoped group identifier derived from the fingerprint.
    pub(super) group_id: String,
}

/// A chunk's identifier: its scope, its document revision and parser, the
/// chunker and preparation versions, the counter's contract and its source
/// coordinates.
pub(super) fn chunk_id(
    deduplication: &Deduplication<'_>,
    document: &CanonicalDocument,
    mapped: &MappedDocument,
    content: &ChunkContent,
    tokenizer_contract_id: &str,
) -> Result<String, Error> {
    let coordinates = content
        .fragments
        .iter()
        .map(|fragment| {
            let unit = mapped
                .units
                .get(fragment.contribution.unit_index)
                .ok_or_else(invalid_chunks)?;
            Ok((&unit.unit_id, fragment.contribution.range))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(format!(
        "chunk-{}",
        digest(&record_bytes(&(
            "source-retrieval-chunk/v1",
            deduplication.tenant_id,
            deduplication.workspace_id,
            &document.document_id,
            &document.revision_id,
            &document.schema_version,
            &document.parser_version,
            &document.parser_options,
            CHUNKER_VERSION,
            PREPARATION_PROFILE,
            tokenizer_contract_id,
            coordinates,
        ))?)
    ))
}

/// The fingerprint of a prepared input and its group within the scope.
pub(super) fn prepared_identity(
    deduplication: &Deduplication<'_>,
    tokenizer_contract_id: &str,
    prepared_input: &str,
) -> Result<PreparedIdentity, Error> {
    let bytes = record_bytes(&(PREPARATION_PROFILE, tokenizer_contract_id, prepared_input))?;
    let fingerprint = format!("sha256:{}", digest(&bytes));
    let group_id = format!(
        "prepared-{}",
        digest(&record_bytes(&(
            "prepared-document-input/v1",
            deduplication.tenant_id,
            deduplication.workspace_id,
            &fingerprint,
        ))?)
    );
    Ok(PreparedIdentity {
        fingerprint,
        bytes,
        group_id,
    })
}

/// Prepared-input groups by fingerprint, each with the serialized record its fingerprint digests.
pub(super) type PreparedGroups = BTreeMap<String, (Vec<u8>, PreparedInputGroup)>;

/// Add a chunk to the group of its prepared input; one fingerprint over different bytes is a hash
/// collision and fails.
pub(super) fn insert_prepared_group(
    groups: &mut PreparedGroups,
    hash: String,
    bytes: Vec<u8>,
    group_id: String,
    index: usize,
) -> Result<(), Error> {
    use std::collections::btree_map::Entry;
    match groups.entry(hash.clone()) {
        Entry::Occupied(mut slot) => {
            let (previous, group) = slot.get_mut();
            if previous != &bytes {
                return Err(Error("prepared-input hash collision".into()));
            }
            group.chunk_indices.push(index);
        }
        Entry::Vacant(slot) => {
            slot.insert((
                bytes,
                PreparedInputGroup {
                    group_id,
                    content_hash: hash,
                    chunk_indices: vec![index],
                },
            ));
        }
    }
    Ok(())
}

/// The JSON bytes an identity digests.
fn record_bytes(value: &impl Serialize) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value).map_err(|_| invalid_chunks())
}
