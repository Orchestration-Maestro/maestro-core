//! Tests of the document records: the documents migration, collections,
//! sources and documents, the guards that keep a document's identity,
//! revisions and their quality dispositions, and a collection's counts of
//! them, with the same outcomes on Linux, macOS and Windows.

mod counts;
mod dispositions;
mod duplicates;
mod errors;
mod guards;
mod listing;
mod parents;
mod revisions;
mod schema;
mod support;
