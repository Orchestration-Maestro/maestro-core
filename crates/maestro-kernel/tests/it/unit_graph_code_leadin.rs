//! The kernel accepts code groups without an optional lead-in relation.
use maestro_kernel::unit_graph::{
    ContextKind, DeliveryGraph, GroupKind, MappingLedger, RetrievalMembership,
};

#[test]
fn code_group_without_lead_in_is_valid() {
    let graph_bytes = include_bytes!("../fixtures/unit-graph-v1.json")
        .strip_suffix(b"\n")
        .unwrap();
    let mut graph = DeliveryGraph::from_bytes(graph_bytes).unwrap();
    let ledger = MappingLedger::from_bytes(
        include_bytes!("../fixtures/unit-mapping-v1.json")
            .strip_suffix(b"\n")
            .unwrap(),
    )
    .unwrap();
    let source = include_str!("../fixtures/unit-graph-v1.txt");
    let code = graph
        .groups
        .iter_mut()
        .find(|group| group.kind == GroupKind::Code)
        .unwrap();
    let lead_in = code
        .context_relations
        .iter()
        .find(|relation| relation.kind == ContextKind::LeadIn)
        .unwrap()
        .part_id
        .clone();
    code.context_relations
        .retain(|relation| relation.kind != ContextKind::LeadIn);
    code.part_ids.retain(|part_id| part_id != &lead_in);
    let memberships = &mut graph.retrieval_views[0].memberships;
    let code_position = memberships
        .iter()
        .position(|membership| membership.unit_id == "code-block")
        .unwrap();
    memberships.insert(
        code_position,
        RetrievalMembership {
            unit_id: "lead-in".into(),
            primary_part_ids: vec![lead_in],
        },
    );
    graph.validate(&ledger, source).unwrap();
}
