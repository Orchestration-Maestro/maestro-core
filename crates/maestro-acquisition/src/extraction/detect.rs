//! Bounded deterministic evidence matching; no parser, network or model starts.
pub(crate) use crate::policy::shape::{valid_id as id_valid, valid_text as text};
use crate::{policy::acquisition::DomPath, refusal::Refusal};
use maestro_knowledge::collection::PolicyReference as Ref;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::str;

/// Exact JSON path step, never an executable query.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum JsonStep {
    /// Literal field name.
    Field {
        /// Bounded nonempty name.
        name: String,
    },
    /// Fixed unsigned array index.
    Index {
        /// Position in an observed array.
        value: u32,
    },
}
/// Atomic bounded structure observation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Observation {
    /// Literal DOM tag/attribute path.
    Dom {
        /// Nonempty direct-child path.
        path: DomPath,
    },
    /// Literal JSON field/index path.
    Json {
        /// Nonempty bounded path.
        path: Vec<JsonStep>,
    },
    /// Existing structural block kind.
    Block {
        /// Exact block kind ID.
        id: String,
    },
    /// Literal content prefix.
    Prefix {
        /// Bounded nonempty prefix.
        text: String,
    },
}

/// Bounded content detection rule; filename is intentionally not a detector.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Detector {
    /// Fixed signature in the retained bounded sample.
    Magic {
        /// Byte offset within the 64 KiB sample.
        offset: usize,
        /// Nonempty lowercase hexadecimal bytes, at most 256 bytes.
        hex_bytes: String,
    },
    /// Exact observed bounded container member.
    ContainerMember {
        /// Literal member name; never opened as a filesystem path.
        name: String,
    },
    /// Declared-media corroboration, insufficient by itself.
    DeclaredMedia {
        /// Exact bounded media string.
        media: String,
    },
    /// Installed qualified parser capability.
    ParserCapability {
        /// Exact capability identity.
        id: String,
    },
}
impl Detector {
    /// Verify a declarative detector before matching.
    pub(crate) fn valid(&self) -> bool {
        match self {
            Self::Magic { offset, hex_bytes } => {
                *offset < 65_536
                    && !hex_bytes.is_empty()
                    && hex_bytes.len() <= 512
                    && hex_bytes.len().is_multiple_of(2)
                    && hex_bytes
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            }
            Self::ContainerMember { name } => text(name) && !name.is_empty(),
            Self::DeclaredMedia { media } => text(media) && !media.is_empty(),
            Self::ParserCapability { id } => id_valid(id),
        }
    }
    /// Match only retained evidence, without inspecting external resources.
    pub(crate) fn detects(&self, evidence: &DetectionEvidence) -> bool {
        let input = &evidence.0;
        match self {
            Self::Magic { offset, hex_bytes } => {
                let bytes: Option<Vec<_>> = hex_bytes
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| {
                        let hex = str::from_utf8(pair).ok()?;
                        u8::from_str_radix(hex, 16).ok()
                    })
                    .collect();
                bytes.is_some_and(|bytes| {
                    input
                        .sample
                        .get(*offset..offset.saturating_add(bytes.len()))
                        == Some(bytes.as_slice())
                })
            }
            Self::ContainerMember { name } => input.members.contains(name),
            Self::DeclaredMedia { media } => input.declared_media.contains(media),
            Self::ParserCapability { id } => input.capabilities.contains(id),
        }
    }
    /// A hint alone is not content-backed detection.
    #[expect(
        clippy::match_like_matches_macro,
        reason = "keep the content-only guard visible to mutation testing"
    )]
    pub(crate) fn substantive(&self) -> bool {
        match self {
            Self::DeclaredMedia { .. } | Self::ParserCapability { .. } => false,
            _ => true,
        }
    }
}

/// Bounded declarative structure AST; no arbitrary executable selectors.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Structure {
    /// Exact atomic structure observation.
    Observed {
        /// One bounded atomic selector.
        observation: Observation,
    },
    /// Every child must match.
    All {
        /// Nonempty bounded children.
        children: Vec<Structure>,
    },
    /// At least one child must match.
    Any {
        /// Nonempty bounded children.
        children: Vec<Structure>,
    },
}
impl Structure {
    /// Count all AST nodes before matching; depth and nodes are independent caps.
    pub(crate) fn validate(&self, depth: usize, nodes: &mut usize) -> bool {
        *nodes += 1;
        if depth > 32 || *nodes > 1000 {
            return false;
        }
        match self {
            Self::Observed { observation } => observation_valid(observation),
            Self::All { children } | Self::Any { children } => {
                !children.is_empty()
                    && children
                        .iter()
                        .all(|child| child.validate(depth + 1, nodes))
            }
        }
    }
    /// Match the already-bounded AST over observed structures only.
    pub(crate) fn detects(&self, evidence: &DetectionEvidence) -> bool {
        match self {
            Self::Observed { observation } => match observation {
                Observation::Prefix { text } => evidence.0.sample.starts_with(text.as_bytes()),
                _ => evidence.0.structures.contains(observation),
            },
            Self::All { children } => children.iter().all(|child| child.detects(evidence)),
            Self::Any { children } => children.iter().any(|child| child.detects(evidence)),
        }
    }
}

/// Safe obtainable content; held output never means searchable acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafePartial {
    /// Bounded original text layer, without synthesis.
    pub text: String,
    /// Bounded metadata observations.
    pub metadata: Vec<String>,
    /// Bounded immutable retained asset records.
    pub assets: Vec<Ref>,
}

/// Untrusted detection input; pass through `DetectionEvidence::new` before use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceInput {
    /// Original bounded capture sample.
    pub sample: Vec<u8>,
    /// Untrusted declared-media hints.
    pub declared_media: Vec<String>,
    /// Observed container member signatures; no decompression in selection.
    pub members: Vec<String>,
    /// Observed atomic structures.
    pub structures: Vec<Observation>,
    /// Installed capability observations, rechecked against artifact admission.
    pub capabilities: Vec<String>,
    /// Encrypted content is explicitly held.
    pub encrypted: bool,
    /// Malformed content is explicitly held.
    pub malformed: bool,
    /// Safely obtainable original text.
    pub text: String,
    /// Safely obtainable metadata.
    pub metadata: Vec<String>,
    /// Immutable bounded asset inventory.
    pub assets: Vec<Ref>,
    /// Exact scoped detection-receipt references supplied by the caller.
    pub evidence: Vec<Ref>,
}

/// Immutable bounded evidence handle; constructing it performs no effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectionEvidence(EvidenceInput, SafePartial);
impl DetectionEvidence {
    /// Validate every input size and atomic observation before matching.
    ///
    /// # Errors
    /// Oversized samples, lists, text, paths or invalid logical refs refuse.
    pub fn new(input: EvidenceInput) -> Result<Self, Refusal> {
        if input.sample.len() > 65_536 || !text(&input.text) {
            return Err(Refusal::Invalid);
        }
        for values in [
            &input.declared_media,
            &input.members,
            &input.capabilities,
            &input.metadata,
        ] {
            if values.len() > 1000 || !values.iter().all(|value| text(value)) {
                return Err(Refusal::Invalid);
            }
        }
        if input.structures.len() > 1000 || !input.structures.iter().all(observation_valid) {
            return Err(Refusal::Invalid);
        }
        for refs in [&input.assets, &input.evidence] {
            if refs.len() > 1000 || !refs.iter().all(|reference| id_valid(&reference.id)) {
                return Err(Refusal::Invalid);
            }
        }
        let partial = SafePartial {
            text: input.text.clone(),
            metadata: input.metadata.clone(),
            assets: input.assets.clone(),
        };
        Ok(Self(input, partial))
    }
    /// Original immutable bounded observations.
    #[must_use]
    pub fn input(&self) -> &EvidenceInput {
        &self.0
    }
    /// Safe retained content to carry into an explicit held outcome.
    #[must_use]
    pub fn partial(&self) -> &SafePartial {
        &self.1
    }
}

/// Bound atomic selector sizes independently of the global JSON parser.
pub(crate) fn observation_valid(observation: &Observation) -> bool {
    match observation {
        Observation::Dom { path } => {
            !path.is_empty()
                && path.len() <= 32
                && path.iter().all(|step| {
                    id_valid(&step.tag)
                        && step.attributes.len() <= 8
                        && step
                            .attributes
                            .iter()
                            .all(|attribute| id_valid(&attribute.name) && text(&attribute.value))
                })
        }
        Observation::Json { path } => {
            !path.is_empty()
                && path.len() <= 32
                && path.iter().all(|step| match step {
                    JsonStep::Field { name } => text(name) && !name.is_empty(),
                    JsonStep::Index { .. } => true,
                })
        }
        Observation::Block { id } => id_valid(id),
        Observation::Prefix { text: prefix } => text(prefix) && !prefix.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Detector, Observation, Structure};

    /// Depth is tested directly: the JSON parser must not mask this AST guard.
    #[test]
    fn n15_ast_depth_has_an_unmasked_boundary() {
        let mut selector = Structure::Observed {
            observation: Observation::Block {
                id: "heading".into(),
            },
        };
        for _ in 1..32 {
            selector = Structure::All {
                children: vec![selector],
            };
        }
        assert!(selector.validate(1, &mut 0));
        selector = Structure::All {
            children: vec![selector],
        };
        assert!(!selector.validate(1, &mut 0));
    }

    /// Every detector arm is bounded independently of JSON decoding.
    #[test]
    fn n15_detector_validity_has_unmasked_neighbours() {
        for detector in [
            Detector::Magic {
                offset: 65_536,
                hex_bytes: "23".into(),
            },
            Detector::Magic {
                offset: 0,
                hex_bytes: String::new(),
            },
            Detector::ContainerMember {
                name: String::new(),
            },
            Detector::DeclaredMedia {
                media: String::new(),
            },
            Detector::ParserCapability {
                id: "../bad".into(),
            },
        ] {
            assert!(!detector.valid());
        }
        assert!(
            Detector::Magic {
                offset: 65_535,
                hex_bytes: "23".into()
            }
            .valid()
        );
        assert!(
            Detector::ContainerMember {
                name: "word/document.xml".into()
            }
            .valid()
        );
        assert!(
            Detector::DeclaredMedia {
                media: "image/png".into()
            }
            .valid()
        );
        assert!(
            Detector::ParserCapability {
                id: "synthetic-rust".into()
            }
            .valid()
        );
    }
}
