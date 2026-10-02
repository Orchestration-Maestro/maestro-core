//! N37 uses N12 durable fixtures and the real N05/N07 contracts.
use super::{
    n07_parse_url_identity_and_denial_precedence::Controls, n09_support::Grants,
    n12_support::Fixture,
};
use maestro_acquisition::{CheckedPolicy, lifecycle::resume::Current};
use maestro_kernel::{
    acquisition::{
        BudgetUsage, DispatchRequest, Frontier, Handle, Item, LeaseRequest, Reason, Receipt,
        ReceiptSchema, Receipts, Status,
    },
    scope::Scope,
};
use std::{sync::LazyLock, time::Duration};

pub(super) fn scope() -> Scope {
    "workspace/default/collection/garden".parse().unwrap()
}
pub(super) fn item(fixture: &Fixture) -> Item {
    Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        100,
    )
    .unwrap()
    .into_iter()
    .find(|item| item.id == fixture.context.item.item)
    .unwrap()
}
pub(super) fn current<'a>(
    fixture: &'a Fixture,
    policy: &'a CheckedPolicy,
    controls: &'a Controls,
    grants: &'a Grants,
) -> Current<'a> {
    Current {
        policy,
        controls,
        authority: grants,
        principal: "reader",
        scope: &SCOPE,
        account: "public",
        authorization: &fixture.envelope.authorization_context,
        now: fixture.context.now,
        inputs: fixture.envelope.inputs,
    }
}
/// One synthetic scope shared by borrowed current contexts.
static SCOPE: LazyLock<Scope> = LazyLock::new(scope);

pub(super) fn redispatch(fixture: &mut Fixture) {
    fixture.context.now += Duration::from_secs(31);
    fixture.context.writer = fixture
        .db
        .lease_source(
            "notes",
            &scope(),
            LeaseRequest {
                holder: "resumed",
                now: fixture.context.now,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    fixture.context.item = fixture
        .db
        .lease(
            &fixture.context.writer,
            fixture.context.item.item,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: "resumed",
                    now: fixture.context.now,
                    term: Duration::from_secs(30),
                },
                max_attempts: 3,
            },
        )
        .unwrap();
}
pub(super) fn receipt(fixture: &mut Fixture) -> Receipt {
    let receipt = Receipt {
        schema: ReceiptSchema::V1,
        run: fixture.envelope.run,
        attempt: Handle::new(),
        inputs: fixture.envelope.inputs,
        tightening: vec![],
        inventories: vec![],
        attempts: 0,
        budget: BudgetUsage {
            elapsed_ms: 0,
            response_bytes: 4,
            staging_bytes: 4,
        },
        downstream: vec![],
        status: Status::Pending,
        reason: Reason::None,
    };
    fixture.db.begin(&scope(), &receipt).unwrap();
    fixture
        .db
        .release_source(&fixture.context.writer, fixture.context.now)
        .unwrap();
    let holder = receipt.attempt.to_string();
    let lease = LeaseRequest {
        holder: &holder,
        now: fixture.context.now,
        term: Duration::from_secs(30),
    };
    fixture.context.writer = fixture.db.lease_source("notes", &scope(), lease).unwrap();
    fixture.context.item = fixture
        .db
        .lease(
            &fixture.context.writer,
            fixture.context.item.item,
            DispatchRequest {
                lease,
                max_attempts: 3,
            },
        )
        .unwrap();
    receipt
}
