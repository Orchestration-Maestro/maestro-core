//! Throwaway S2 G25 probe of the `lbug` crate (`LadybugDB`). Nothing in the
//! product depends on it; the tests beside it are the evidence. They and the
//! example build only with the `engine` feature, so that no default build
//! compiles the C++ engine: `cargo test -p lbug-spike --features engine`.
