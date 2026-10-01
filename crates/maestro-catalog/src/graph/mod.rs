//! Internal graph checks, not source admission, routing or execution.

#[cfg(test)]
mod tests;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "C22b supplies the first admitted graph compiler caller"
    )
)]
mod topology;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "C22b resolves topology inputs and evidence from admitted sources"
    )
)]
mod types;
