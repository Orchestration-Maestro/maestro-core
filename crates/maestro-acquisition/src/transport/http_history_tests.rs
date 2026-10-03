//! Hop metadata refuses before the history grows, including empty bodies.
use super::{
    http::retain_hop,
    stream::{Accounting, Failure},
};
use crate::policy::schema::SourcePolicy;
use maestro_kernel::acquisition::{RedirectHop, SafeIdentity};
#[test]
fn n12_zero_body_hops_refuse_before_history_growth() {
    let policy: SourcePolicy =
        serde_json::from_slice(include_bytes!("../../tests/fixtures/policy.json")).unwrap();
    let mut limits = policy.aggregate_limits;
    limits.memory_bytes = 50_000.try_into().unwrap();
    limits.decode.memory_bytes = 50_000.try_into().unwrap();
    let mut accounting = Accounting::new(limits);
    accounting.workspace(49_152).unwrap();
    let identity =
        SafeIdentity::new(&format!("https://garden.example/docs/{}", "x".repeat(400))).unwrap();
    let hop = RedirectHop {
        identity,
        status: 302,
    };
    let mut history = Vec::new();
    retain_hop(&mut history, &mut accounting, hop.clone()).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(
        retain_hop(&mut history, &mut accounting, hop),
        Err(Failure::Memory)
    );
    assert_eq!(
        history.len(),
        1,
        "refused metadata never grows redirect history"
    );
    assert_eq!(accounting.wire_bytes(), 0);
    assert_eq!(accounting.expanded_bytes(), 0);
}
