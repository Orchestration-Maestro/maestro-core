//! Private ladder routing and prompt paths refuse before inference.
use super::{
    graph_eval::{ladder, manifest},
    support::Home,
};
use serde_json::{Value, json};
use std::fs;

#[test]
fn graph_eval_ladder_requires_an_explicit_local_router() {
    for router in [None, Some("http://192.0.2.1:18000")] {
        let home = Home::bare();
        let graph = manifest(&home);
        let path = ladder(&home, &graph, "none");
        let mut command = home.command(&["eval", "ladder", "--manifest", path.to_str().unwrap()]);
        command.env("MAESTRO_QDRANT_URL", "http://localhost:16334");
        if let Some(router) = router {
            command.env("MAESTRO_ROUTER_URL", router);
        } else {
            command.env_remove("MAESTRO_ROUTER_URL");
        }
        let result = command.output().unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert_eq!(
            String::from_utf8_lossy(&result.stderr).trim(),
            "graph_scratch_refused"
        );
    }
}

#[test]
fn graph_eval_ladder_prompt_must_be_inside_the_approved_private_root() {
    let home = Home::bare();
    let graph = manifest(&home);
    let path = ladder(&home, &graph, "none");
    let outside = home.root().join("outside-prompt.txt");
    fs::write(&outside, "private prompt").unwrap();
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    value["rungs"][0]["ask"] = json!({"prompt":{"file":outside}});
    fs::write(&path, value.to_string()).unwrap();
    let result = home
        .command(&["eval", "ladder", "--manifest", path.to_str().unwrap()])
        .env("MAESTRO_QDRANT_URL", "http://localhost:16334")
        .env("MAESTRO_ROUTER_URL", "http://localhost:18000")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr).trim(),
        "graph_scratch_refused"
    );
}
