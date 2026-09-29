//! The default splitter: English and French relation phrases and
//! conditionals, the topic kept in every part, and each reason a question
//! stays whole.

use crate::{
    query::understand,
    search::{QuestionSplit, QuestionSplitter, RelationSplitter, Unsplit},
};

/// The split of `question`.
fn split(question: &str) -> QuestionSplit {
    RelationSplitter.split(&understand(question))
}

/// `question` stays one part, itself, for `reason`.
fn assert_whole(question: &str, reason: Unsplit) {
    assert_eq!(
        split(question),
        QuestionSplit {
            parts: vec![question.to_owned()],
            unsplit: Some(reason),
        },
        "{question}"
    );
}

#[test]
fn the_owner_question_splits_into_its_action_and_its_condition() {
    let question = "how to configure a control-m job to send a not ok message only after the \
                    third failure occurrence";
    assert_eq!(
        split(question),
        QuestionSplit {
            parts: vec![
                "how to configure a control-m job to send a not ok message".to_owned(),
                "control-m job only after the third failure occurrence".to_owned(),
            ],
            unsplit: None,
        }
    );
}

#[test]
fn a_french_question_splits_at_its_relation_phrase_and_its_conditional() {
    assert_eq!(
        split(
            "comment configurer un job control-m pour envoyer un message seulement après le \
             troisième échec"
        )
        .parts,
        [
            "comment configurer un job control-m pour envoyer un message",
            "job control-m seulement après le troisième échec",
        ]
    );
    assert_eq!(
        split("comment relancer un traitement bloqué quand le serveur redémarre la nuit").parts,
        [
            "comment relancer un traitement bloqué",
            "traitement bloqué quand le serveur redémarre la nuit",
        ]
    );
}

#[test]
fn the_topic_leads_every_later_part_once() {
    let parts = split(
        "how to rerun a failed job automatically when the agent restarts after a network outage \
         ends",
    )
    .parts;
    assert_eq!(parts.len(), 3);
    for part in &parts {
        assert_eq!(part.matches("failed job").count(), 1, "{part}");
    }
    let parts = split("notify the job owner when the job owner leaves the team").parts;
    assert_eq!(
        parts,
        ["notify the job owner", "when the job owner leaves the team"]
    );
}

#[test]
fn a_question_without_a_relation_phrase_is_one_part() {
    assert_whole("how do I kill a running job", Unsplit::NoRelation);
    assert_whole("comment supprimer une tâche", Unsplit::NoRelation);
}

#[test]
fn a_part_with_fewer_than_two_words_of_its_own_is_not_split() {
    assert_whole("how to rerun a job from a step", Unsplit::ShortPart);
    assert_whole("send a message if it fails", Unsplit::ShortPart);
    assert_whole("after a failure rerun the job", Unsplit::ShortPart);
}

#[test]
fn a_phrase_inside_a_quote_an_identifier_or_a_name_is_not_split() {
    assert_whole(
        "what does the field \"notify only after the third run\" change in a job",
        Unsplit::Protected,
    );
    assert_whole(
        "what does the Run Only After Failure option change in a job",
        Unsplit::Protected,
    );
    assert_whole(
        "what does `ctm run order --only after` print for a folder",
        Unsplit::Protected,
    );
}

#[test]
fn a_question_splits_into_three_parts_at_most() {
    let result = split(
        "notify the job owner when the job fails after the third retry until the end of the \
         month",
    );
    assert_eq!(
        result.parts,
        [
            "notify the job owner",
            "job owner when the job fails",
            "job owner after the third retry until the end of the month",
        ]
    );
    assert_eq!(result.unsplit, Some(Unsplit::PartCap));
}

#[test]
fn reasons_have_stable_codes() {
    assert_eq!(
        [
            Unsplit::NoRelation,
            Unsplit::ShortPart,
            Unsplit::Protected,
            Unsplit::PartCap,
        ]
        .map(Unsplit::code),
        [
            "no_relation_phrase",
            "part_too_short",
            "protected_phrase",
            "part_cap",
        ]
    );
}
