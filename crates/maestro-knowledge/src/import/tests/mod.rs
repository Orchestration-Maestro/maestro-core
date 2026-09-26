//! Tests of the import that reach inside it: a manifest read one numbered
//! line at a time, and the import's streaming, proven with an in-memory
//! corpus that checks every read.

mod lines;
mod streaming;
