//! A presented prompt: the session's language and tone. Normal tone in the
//! question's language is today's prompt byte for byte; a tone adds one
//! versioned instruction; an explicit language replaces the question's and
//! is the language the host's own texts and `lang` report.

use super::*;
use crate::answer::{Presentation, Tone};
use maestro_kernel::gateway::Message;

/// The prompt of `answer_prompt` for one English question over one passage.
fn messages(answer_prompt: &AnswerPrompt) -> Vec<Message> {
    let request = request("How do I list the registered sources?");
    let evidence = bundle(&request.question, "en", "Sources are listed.");
    prompt(&request, &evidence, answer_prompt).expect("bounded prompt")
}

/// The presented prompt of `version`, `language` and `tone`.
fn presented(version: PromptVersion, language: Option<&str>, tone: Tone) -> AnswerPrompt {
    AnswerPrompt::Presented {
        version,
        presentation: Presentation {
            language: language.map(str::to_owned),
            tone,
        },
    }
}

#[test]
fn normal_tone_in_the_question_language_is_the_version_prompt_byte_for_byte() {
    for version in [
        PromptVersion::V1,
        PromptVersion::V2,
        PromptVersion::ProcedureFirst,
    ] {
        assert_eq!(
            messages(&presented(version, None, Tone::Normal)),
            messages(&version.into()),
            "{version:?}"
        );
    }
    assert_eq!(Tone::default(), Tone::Normal);
    assert_eq!(Presentation::default().language, None);
}

#[test]
fn a_tone_adds_its_versioned_instruction_to_the_system_text_only() {
    let plain = messages(&PromptVersion::V2.into());
    for (tone, instruction) in [
        (
            Tone::Brief,
            "Keep the answer brief: the fewest full sentences that answer the question \
             completely, each still ending with its marker.",
        ),
        (
            Tone::Detailed,
            "Give a detailed answer: explain each step, condition and prerequisite the \
             passages state, each sentence still ending with its marker.",
        ),
    ] {
        let toned = messages(&presented(PromptVersion::V2, None, tone));
        assert_eq!(
            toned[0].content,
            format!("{} {instruction}", plain[0].content),
            "{tone:?}"
        );
        assert_eq!(toned[1], plain[1], "{tone:?}");
    }
}

#[test]
fn an_explicit_language_replaces_the_question_language_in_every_version() {
    for version in [
        PromptVersion::V1,
        PromptVersion::V2,
        PromptVersion::ProcedureFirst,
    ] {
        let plain = messages(&version.into());
        let french = messages(&presented(version, Some("fr-CA"), Tone::Normal));
        assert_eq!(
            french[0].content,
            plain[0].content.replacen(
                "in the language of the question",
                "in the language whose BCP 47 tag is \"fr-CA\"",
                1
            ),
            "{version:?}"
        );
        assert_ne!(french[0].content, plain[0].content, "{version:?}");
        assert_eq!(french[1], plain[1], "{version:?}");
    }
    let quoted = messages(&presented(PromptVersion::V2, Some("x\"y"), Tone::Brief));
    assert!(
        quoted[0]
            .content
            .starts_with("Answer in the language whose BCP 47 tag is \"x\\\"y\". "),
        "{}",
        quoted[0].content
    );
}

#[test]
fn a_rung_prompt_text_is_never_presented() {
    let text = PromptText::new("System.".to_owned(), "{data}".to_owned()).expect("one slot");
    let prompt = AnswerPrompt::Text(text);
    assert_eq!(prompt.presentation(), &Presentation::default());
    assert_eq!(messages(&prompt)[0].content, "System.");
}

#[test]
fn presentation_identity_names_its_version_language_and_tone() {
    assert_eq!(
        Presentation::default().identity(),
        "presentation/1 language=auto tone=normal"
    );
    assert_eq!(
        Presentation {
            language: Some("es-419".to_owned()),
            tone: Tone::Detailed
        }
        .identity(),
        "presentation/1 language=es-419 tone=detailed"
    );
    assert_eq!(Tone::Brief.name(), "brief");
}

#[tokio::test]
async fn an_explicit_language_is_the_answer_lang_and_the_host_text_language() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("How do I list the registered sources?");
    for (tag, message) in [
        (
            "fr-CA",
            "Les passages disponibles ne répondent pas à la question.",
        ),
        ("es", "The available passages do not answer the question."),
    ] {
        let evidence = bundle(&request.question, "en", "Sources are listed.");
        let port = ScriptedPort::new(&["NOT_FOUND"]);
        let answer = answer_bundle(
            &port,
            &request,
            Some(&answerer),
            evidence,
            &presented(PromptVersion::V2, Some(tag), Tone::Normal),
        )
        .await
        .expect("refusal");
        assert_eq!(answer.lang, tag);
        assert_eq!(answer.refusal.expect("not found").message, message, "{tag}");
    }
    let evidence = bundle(&request.question, "en", "Sources are listed.");
    let port = ScriptedPort::new(&["Sources are listed. [1]"]);
    let answer = answer_bundle(
        &port,
        &request,
        Some(&answerer),
        evidence,
        &presented(PromptVersion::V2, None, Tone::Brief),
    )
    .await
    .expect("answer");
    assert_eq!(answer.lang, "en");
}

#[tokio::test]
async fn a_threshold_refusal_speaks_the_explicit_language() {
    let request = request("How do I list the registered sources?");
    let evidence = bundle(&request.question, "en", "Sources are listed.");
    let port = ScriptedPort::new(&[]);
    let prompt = presented(PromptVersion::V2, Some("fr"), Tone::Normal);
    let answer = answer_relevant(
        &port,
        &request,
        None,
        evidence,
        AnswerPlan {
            min_rerank_score: Some(1.0),
            top_rerank_score: Some(0.0),
            answer_prompt: &prompt,
        },
    )
    .await
    .expect("refusal");
    assert_eq!(answer.lang, "fr");
    assert_eq!(
        answer.refusal.expect("below threshold").message,
        "Le meilleur passage est sous le seuil de pertinence."
    );
}
