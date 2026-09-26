//! Reciprocal rank fusion for search routes.

mod fusion;
#[cfg(test)]
mod tests;

pub use fusion::{Fused, Hit, Route, RouteList, fuse};
