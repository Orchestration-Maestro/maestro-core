//! `ask` searches with the configuration its caller gives.

use super::configured_search::{clean, context, published};
use maestro_knowledge::{
    answer::{
        AnswerContext, AskBudget, AskRequest, DEFAULT_MODEL, RefusalCode, ask, ask_configured,
    },
    search::SearchConfiguration,
};

#[tokio::test]
async fn ask_runs_its_search_with_the_given_configuration() {
    let fixture = published().await;
    let answer_context = AnswerContext {
        search: context(&fixture, None),
        port: &fixture.port,
        answerer: None,
    };
    let request = AskRequest {
        collection: fixture.kernel.collection.clone(),
        question: "scheduler".to_owned(),
        model: DEFAULT_MODEL.to_owned(),
        version: None,
        budget: AskBudget {
            search_deadline_ms: 5000,
            ..AskBudget::default()
        },
    };
    let lexical_only = SearchConfiguration {
        dense_enabled: false,
        identifier_enabled: false,
        structured_enabled: false,
        rerank_enabled: false,
        ..SearchConfiguration::default()
    };

    let before = fixture.port.calls().len();
    let answer = Box::pin(ask_configured(&answer_context, &request, lexical_only))
        .await
        .unwrap();
    assert_eq!(fixture.port.calls().len(), before);
    assert_eq!(
        answer.refusal.map(|refusal| refusal.code),
        Some(RefusalCode::AnswererUnavailable)
    );

    let answer = Box::pin(ask(&answer_context, &request)).await.unwrap();
    assert!(fixture.port.calls().len() > before);
    assert_eq!(
        answer.refusal.map(|refusal| refusal.code),
        Some(RefusalCode::AnswererUnavailable)
    );

    clean(&fixture).await;
}
