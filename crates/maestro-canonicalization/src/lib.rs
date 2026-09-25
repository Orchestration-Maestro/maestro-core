//! Local, deterministic canonical documents. No networking or inference.
//! Source replay verifies consistency, not the authenticity of caller-supplied metadata.
//! Failed documents are inspectable evidence, never trusted retrieval input.
//!
//! ```
//! use maestro_canonicalization::{canonicalize, CanonicalizeInput};
//! let doc = canonicalize(CanonicalizeInput::new("# Hello\n", "notes/hello.md"))?;
//! assert_eq!(doc.blocks[0].retrieval_text, "Hello");
//! # Ok::<(), maestro_canonicalization::Error>(())
//! ```
#![forbid(unsafe_code)]
mod accounting;
mod assemble;
mod chunk_mapping;
mod chunk_split;
mod chunks;
mod content;
mod dedup;
mod document;
mod error;
mod hashing;
mod metadata;
mod model;
mod parse;
mod pipeline;
mod prepared_inputs;
mod replay;
mod source_units;
mod store;
mod tokenizer;
mod validate;
pub use chunks::*;
pub use content::*;
pub use dedup::*;
pub use document::{CanonicalDocument, MarkdownReference, Section, SourceAccounting, SourceRole};
pub use error::Error;
pub use model::*;
pub use pipeline::{PARSER_VERSION, SCHEMA_VERSION, canonicalize};
pub use prepared_inputs::{
    ChunkContent, Contribution, Fragment, InputPart, InputRole, PREPARATION_PROFILE, SplitKind,
    TableWindow,
};
pub use replay::validate_document;
pub use source_units::{
    AccountingDisposition, CHUNKER_VERSION, InlineEnvelope, MappedDocument, MappingRun, OriginMode,
    SourceDisposition, SourceOrigin, SourceUnit, TextRange, UnitField,
};
pub use store::{load_document, save_document};
pub use tokenizer::NativeTokenizer;
