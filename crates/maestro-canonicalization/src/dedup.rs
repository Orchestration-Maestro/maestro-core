//! Pure, scoped exact grouping; equality never merges identity or grants access.
//! Authorization is asserted by a trusted caller, not inferred from document data.
use crate::{
    BlockAttributes, BlockType, CanonicalDocument, ContentNode, Error, Inline, InlineKind,
    Severity, digest, validate_document,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

/// The grouping policy's version, recorded in every result and group identifier.
const GROUP_VERSION: &str = "scoped-exact-dedup/1";

/// One immutable source revision, independent of its content equality.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct RevisionKey {
    /// Stable source identity.
    pub document_id: String,
    /// Immutable source/provenance revision.
    pub revision_id: String,
}

/// Explicit caller-asserted authorization boundary for one grouping operation.
/// Construct this from a trusted authority, never from document frontmatter.
/// It is not an authentication mechanism or a replacement for source policies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupScope {
    /// Nonblank tenant identifier; not inferred or normalized.
    pub tenant_id: String,
    /// Nonblank workspace identifier within the tenant.
    pub workspace_id: String,
    /// Revisions the caller authorizes for this operation in this scope.
    pub authorized_revisions: BTreeSet<RevisionKey>,
}

/// Explicit treatment of recoverable canonicalization findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningPolicy {
    /// Refuse any document with a warning.
    Reject,
    /// Retain all warnings on the unchanged occurrence; errors still refuse.
    Preserve,
}

/// Exact source bytes paired with their canonical representation.
#[derive(Debug, Clone, Copy)]
pub struct DedupInput<'a> {
    /// Replay-checked before grouping; never mutated.
    pub document: &'a CanonicalDocument,
    /// Entire original UTF-8 Markdown, not retrieval text.
    pub markdown: &'a str,
}

/// Separate equality domains; neither is prepared model-input equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Representation {
    /// Exact original Markdown bytes.
    Original,
    /// Exact versioned structured projection, not semantic similarity.
    Canonical,
}

/// One retained occurrence, including every original policy, span and finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DedupOccurrence<'a> {
    /// Complete unchanged canonical record, not a representative winner.
    pub document: &'a CanonicalDocument,
    /// Complete unchanged reference Markdown.
    pub markdown: &'a str,
    /// SHA-256 of original UTF-8 bytes, matching the canonical source hash.
    pub original_hash: String,
    /// SHA-256 of the versioned canonical comparison bytes.
    pub canonical_hash: String,
}

/// An equality class; a singleton is not a duplicate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DuplicateGroup {
    /// Scope/profile/content-derived identity, not an access grant or source ID.
    pub group_id: String,
    /// Equality domain that was actually verified.
    pub representation: Representation,
    /// Versioned comparison policy.
    pub profile: String,
    /// Candidate hash followed by verified byte equality within this class.
    pub content_hash: String,
    /// Ordered indices into the result's retained occurrences.
    pub occurrence_indices: Vec<usize>,
}

/// Deterministic local grouping under a caller's authorization snapshot.
/// Historical results must be reauthorized before later use. No cache is kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Deduplication<'a> {
    /// Version of the grouping result and identity policy.
    pub version: &'static str,
    /// Explicit tenant boundary.
    pub tenant_id: &'a str,
    /// Explicit workspace boundary.
    pub workspace_id: &'a str,
    /// Warning treatment used for this result.
    pub warning_policy: WarningPolicy,
    /// Source/revision-ordered records, with no source identity merged away.
    pub occurrences: Vec<DedupOccurrence<'a>>,
    /// Representation/hash-ordered classes, including singletons.
    pub groups: Vec<DuplicateGroup>,
}

/// Group exact original and structured canonical equality in an explicit scope.
///
/// The trusted caller supplies authorization; this function cannot prove it.
/// Source policies remain per occurrence. No result authorizes later retrieval.
/// There is no I/O, global lookup, source deletion or input mutation.
///
/// # Errors
/// Refuses blank scope, unauthorized or repeated revisions, failed/tampered
/// canonical records, disallowed warnings, serialization errors or collisions.
pub fn group_exact<'a>(
    scope: &'a DedupScope,
    inputs: &[DedupInput<'a>],
    warning_policy: WarningPolicy,
) -> Result<Deduplication<'a>, Error> {
    if scope.tenant_id.trim().is_empty() || scope.workspace_id.trim().is_empty() {
        return Err(Error("explicit tenant and workspace are required".into()));
    }
    let mut seen = BTreeSet::new();
    for input in inputs {
        let key = RevisionKey {
            document_id: input.document.document_id.clone(),
            revision_id: input.document.revision_id.clone(),
        };
        if !scope.authorized_revisions.contains(&key) {
            return Err(Error("revision is not authorized for this scope".into()));
        }
        if !seen.insert(key) {
            return Err(Error(
                "repeated document revision in deduplication input".into(),
            ));
        }
    }
    let mut ordered = inputs.to_vec();
    ordered.sort_unstable_by(|a, b| {
        (&a.document.document_id, &a.document.revision_id)
            .cmp(&(&b.document.document_id, &b.document.revision_id))
    });
    let mut groups = Groups::new();
    let mut occurrences = Vec::with_capacity(ordered.len());
    // ponytail: replay plus in-memory comparison bytes; use external-memory grouping
    // if measured batches outgrow RAM.
    for input in ordered {
        let findings = validate_document(input.document, input.markdown);
        if findings
            .iter()
            .any(|finding| finding.severity == Severity::Error)
            || (warning_policy == WarningPolicy::Reject && !findings.is_empty())
        {
            return Err(Error(
                "canonical document is not eligible for grouping".into(),
            ));
        }
        let canonical = canonical_bytes(input.document)?;
        let canonical_hash = format!("sha256:{}", digest(&canonical));
        let original_hash = input.document.content_hash.clone();
        let index = occurrences.len();
        insert_group(
            &mut groups,
            scope,
            (Representation::Original, original_hash.clone()),
            input.markdown.as_bytes().to_vec(),
            index,
        )?;
        insert_group(
            &mut groups,
            scope,
            (Representation::Canonical, canonical_hash.clone()),
            canonical,
            index,
        )?;
        occurrences.push(DedupOccurrence {
            document: input.document,
            markdown: input.markdown,
            original_hash,
            canonical_hash,
        });
    }
    Ok(Deduplication {
        version: GROUP_VERSION,
        tenant_id: &scope.tenant_id,
        workspace_id: &scope.workspace_id,
        warning_policy,
        occurrences,
        groups: groups.into_values().map(|(_, group)| group).collect(),
    })
}

impl Representation {
    /// The serialization profile a representation's bytes follow.
    fn profile(self) -> &'static str {
        match self {
            Self::Original => "original-utf8/v1",
            Self::Canonical => "canonical-structured/v1",
        }
    }
}

/// Duplicate groups by representation and content hash, each with the bytes its hash covers.
type Groups = BTreeMap<(Representation, String), (Vec<u8>, DuplicateGroup)>;

/// Add an occurrence to the group of its representation and hash; equal hashes over different bytes
/// are a collision and fail.
fn insert_group(
    groups: &mut Groups,
    scope: &DedupScope,
    key: (Representation, String),
    bytes: Vec<u8>,
    index: usize,
) -> Result<(), Error> {
    let (representation, hash) = key;
    match groups.entry((representation, hash.clone())) {
        Entry::Occupied(mut entry) => {
            let (existing, group) = entry.get_mut();
            if *existing != bytes {
                return Err(Error("deduplication hash collision".into()));
            }
            group.occurrence_indices.push(index);
        }
        Entry::Vacant(entry) => {
            let profile = representation.profile();
            let identity = json_bytes(&(
                GROUP_VERSION,
                &scope.tenant_id,
                &scope.workspace_id,
                representation,
                profile,
                &hash,
            ))?;
            entry.insert((
                bytes,
                DuplicateGroup {
                    group_id: format!("dup-{}", digest(&identity)),
                    representation,
                    profile: profile.into(),
                    content_hash: hash,
                    occurrence_indices: vec![index],
                },
            ));
        }
    }
    Ok(())
}

/// The JSON bytes a comparison or a group identity digests.
fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(value).map_err(|error| Error(error.to_string()))
}

/// A block as canonical comparison sees it: identifiers become ordinals and spans are left out.
#[derive(Serialize)]
struct ComparisonBlock<'a> {
    /// Part of the comparison: blocks of different types never match.
    block_type: &'a BlockType,
    /// Type-specific attributes, such as a heading level or code info, compared as they are.
    attributes: &'a BlockAttributes,
    /// The parent block by ordinal, so that equal structures match across documents.
    parent: Option<usize>,
    /// The enclosing section by ordinal.
    section: Option<usize>,
    /// The heading titles above the block.
    heading_path: &'a [String],
    /// The block's retrieval text.
    text: &'a str,
    /// Child blocks by ordinal and inline content, in order.
    children: Vec<ComparisonNode<'a>>,
}

/// A child as canonical comparison sees it.
#[derive(Serialize)]
enum ComparisonNode<'a> {
    /// A child block, by ordinal.
    Block(usize),
    /// Inline content, without its spans.
    Inline(ComparisonInline<'a>),
}

/// Inline content as canonical comparison sees it: its content and children, spans left out.
#[derive(Serialize)]
struct ComparisonInline<'a> {
    /// The inline content with its values: text, code, link destination or marker.
    kind: &'a InlineKind,
    /// Nested inline content, in order.
    children: Vec<ComparisonInline<'a>>,
}

/// An inline node and its descendants without their spans.
fn comparison_inline(inline: &Inline) -> ComparisonInline<'_> {
    ComparisonInline {
        kind: &inline.content,
        children: inline.children.iter().map(comparison_inline).collect(),
    }
}

/// The canonical representation's bytes: versions, structure, metadata and extractor content, with
/// block identifiers as ordinals and spans left out.
fn canonical_bytes(doc: &CanonicalDocument) -> Result<Vec<u8>, Error> {
    let ordinals: BTreeMap<_, _> = doc
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.block_id.as_str(), index))
        .collect();
    let ordinal = |id: &str| {
        ordinals
            .get(id)
            .copied()
            .ok_or_else(|| Error("unresolved canonical block reference".into()))
    };
    let blocks = doc
        .blocks
        .iter()
        .map(|block| {
            let children = block
                .structured_content
                .children
                .iter()
                .map(|node| match node {
                    ContentNode::Block { block_id } => {
                        Ok(ComparisonNode::Block(ordinal(block_id)?))
                    }
                    ContentNode::Inline { inline } => {
                        Ok(ComparisonNode::Inline(comparison_inline(inline)))
                    }
                })
                .collect::<Result<Vec<_>, Error>>()?;
            Ok(ComparisonBlock {
                block_type: &block.block_type,
                attributes: &block.structured_content.attributes,
                parent: block.parent_block_id.as_deref().map(&ordinal).transpose()?,
                section: block
                    .parent_section_id
                    .as_deref()
                    .map(&ordinal)
                    .transpose()?,
                heading_path: &block.heading_path,
                text: &block.retrieval_text,
                children,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let extractor_content: Vec<_> = doc
        .extractor_blocks
        .iter()
        .map(|block| &block.structured_content)
        .collect();
    json_bytes(&(
        Representation::Canonical.profile(),
        &doc.schema_version,
        &doc.parser_version,
        &doc.parser_options,
        &doc.source_metadata.title,
        &doc.source_metadata.language,
        &doc.source_metadata.extra,
        extractor_content,
        blocks,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_hashes_cannot_merge_unequal_representation_bytes() {
        let scope = DedupScope {
            tenant_id: "tenant".into(),
            workspace_id: "workspace".into(),
            authorized_revisions: BTreeSet::new(),
        };
        let mut groups = Groups::new();
        insert_group(
            &mut groups,
            &scope,
            (Representation::Original, "forced-hash".into()),
            b"left".to_vec(),
            0,
        )
        .unwrap();
        assert!(
            insert_group(
                &mut groups,
                &scope,
                (Representation::Original, "forced-hash".into()),
                b"right".to_vec(),
                1
            )
            .is_err()
        );
        assert_eq!(groups.values().next().unwrap().1.occurrence_indices, [0]);
    }

    #[test]
    fn groups_name_the_serialization_profile_of_their_bytes() {
        let scope = DedupScope {
            tenant_id: "tenant".into(),
            workspace_id: "workspace".into(),
            authorized_revisions: BTreeSet::new(),
        };
        let mut groups = Groups::new();
        for (index, representation) in [Representation::Original, Representation::Canonical]
            .into_iter()
            .enumerate()
        {
            let key = (representation, "hash".into());
            insert_group(&mut groups, &scope, key, b"bytes".to_vec(), index).unwrap();
        }
        let profiles: BTreeSet<_> = groups.values().map(|(_, g)| g.profile.as_str()).collect();
        assert_eq!(
            profiles,
            BTreeSet::from(["canonical-structured/v1", "original-utf8/v1"])
        );
    }
}
