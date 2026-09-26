//! The import of a collection's corpus manifests (T019): what it refuses,
//! what it holds, what a second import does, how a document is identified,
//! what it reports, what declaring a collection records and what an
//! import's observer sees (T022), the public synthetic collection end to
//! end, and, on demand, its rate.
#![cfg(test)]

mod declared_collections;
mod document_identity;
mod held_pairs;
mod import_rate;
mod import_report;
mod observed_imports;
mod refused_entries;
mod repeated_imports;
mod support;
mod synthetic_corpus;
