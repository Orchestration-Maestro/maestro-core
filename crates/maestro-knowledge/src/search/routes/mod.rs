//! Independent route diagnostics.

pub mod dense;
pub mod error;
pub mod identifier;
mod identifier_cursor;
mod identifier_payload;
pub mod lexical;
pub mod outcome;
pub mod results;
pub mod structured;

#[cfg(test)]
mod tests;
