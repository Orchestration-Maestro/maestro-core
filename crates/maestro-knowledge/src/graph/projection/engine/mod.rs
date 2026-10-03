//! Native projection operations; only feature-enabled builds compile this door.

mod open;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "E08a consumes canonical native rows")
)]
mod rows;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "E08a consumes the native schema")
)]
mod schema;
#[cfg(test)]
mod tests;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "E08a consumes native transactions")
)]
mod transaction;

#[cfg(test)]
#[cfg(not(windows))]
mod rollback_repro;

#[cfg(test)]
mod codec_tests;
#[cfg(test)]
#[cfg(not(windows))]
mod validation_tests;
