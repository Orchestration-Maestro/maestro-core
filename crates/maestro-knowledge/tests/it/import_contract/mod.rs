//! The import of a collection's corpus manifests (T019): what it refuses,
//! what it holds, what a second import does, how a document is identified,
//! what it reports, the public synthetic collection end to end, and, on
//! demand, its rate.
#![cfg(test)]

mod document_identity;
mod held_pairs;
mod import_rate;
mod import_report;
mod refused_entries;
mod repeated_imports;
mod support;
mod synthetic_corpus;
