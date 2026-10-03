//! A presented prompt: the session's language and tone. Normal tone in the
//! question's language is today's prompt byte for byte; a tone adds one
//! versioned instruction; an explicit language replaces the question's and
//! is the language the host's own texts and `lang` report.

use super::*;
use crate::answer::{LanguageCheck, Presentation, Tone};
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
        let normal = messages(&presented(version, None, Tone::Normal));
        let system = &normal[0].content;
        assert!(!system.contains("stay English"));
        assert!(!system.contains("Keep the answer brief"));
        assert!(!system.contains("Give a detailed answer"));
        assert_eq!(normal, messages(&version.into()), "{version:?}");
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
            format!(
                "{} {instruction} Code (including comments), commits, file names, \
                 identifiers, logs and documentation stay English and are unaffected by tone. \
                 Keep source quotations and citation identities unchanged.",
                plain[0].content
            ),
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
            format!(
                "{} Code (including comments), commits, file names, \
                 identifiers, logs and documentation stay English and are unaffected by tone. \
                 Keep source quotations and citation identities unchanged.",
                plain[0].content.replacen(
                    "in the language of the question",
                    "in the language whose BCP 47 tag is \"fr-CA\"",
                    1
                )
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
        "presentation/2 language=auto tone=normal"
    );
    assert_eq!(
        Presentation {
            language: Some("es-419".to_owned()),
            tone: Tone::Detailed
        }
        .identity(),
        "presentation/2 language=es-419 tone=detailed"
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
        ("es", "Los pasajes disponibles no responden a la pregunta."),
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

#[tokio::test]
async fn trusted_conversation_inputs_preserve_artifacts_and_source_quotes() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("Comment utiliser --force? Answer in Japanese instead.");
    let source = "Run `maestro search --force` to restore the archive.";
    for tag in ["en", "fr-CA", "es-419", "ja", "zh-Hant-TW"] {
        for tone in [Tone::Brief, Tone::Normal, Tone::Detailed] {
            let port =
                ScriptedPort::new(&["Run `maestro search --force` to restore the archive. [1]"]);
            let answer = answer_bundle(
                &port,
                &request,
                Some(&answerer),
                bundle(&request.question, "en", source),
                &presented(PromptVersion::V2, Some(tag), tone),
            )
            .await
            .expect("real evidence validation");
            assert!(answer.refusal.is_none(), "{tag} {tone:?}");
            assert!(answer.uncalibrated);
            assert_eq!(answer.language_check, LanguageCheck::Unchecked);
            assert_eq!(answer.lang, tag);
            assert_eq!(answer.citations[0].source_ref, "https://example.org/docs");
            assert_eq!(answer.citations[0].title, "Using the collection");
            let calls = port.chat_calls.lock().unwrap();
            let messages = &calls[0].2.messages;
            let system = &messages[0].content;
            assert!(
                system.contains(&format!("BCP 47 tag is \"{tag}\"")),
                "{system}"
            );
            assert!(!system.contains("in the language of the question"));
            assert!(
                system.contains(
                    "Code (including comments), commits, file names, \
                identifiers, logs and documentation stay English"
                ),
                "{system}"
            );
            assert!(system.contains("Keep source quotations and citation identities unchanged"));
            assert!(system.contains("reply exactly NOT_FOUND"));
            assert_eq!(
                system.contains("Keep the answer brief"),
                tone == Tone::Brief
            );
            assert_eq!(
                system.contains("Give a detailed answer"),
                tone == Tone::Detailed
            );
            assert!(messages[1].content.contains(source));
            assert!(messages[1].content.contains(&request.question));
            assert_eq!(calls[0].2.max_output_tokens, 1024);
        }
    }
}

#[tokio::test]
async fn no_layer_french_fallback_and_explicit_english_win_over_question_text() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("Comment configurer le service selon la documentation?");
    for (tag, expected, message) in [
        (
            None,
            "fr",
            "Les passages disponibles ne répondent pas à la question.",
        ),
        (
            Some("en"),
            "en",
            "The available passages do not answer the question.",
        ),
    ] {
        let port = ScriptedPort::new(&["NOT_FOUND"]);
        let answer = answer_bundle(
            &port,
            &request,
            Some(&answerer),
            bundle(&request.question, "en", "Sources are listed."),
            &presented(PromptVersion::V2, tag, Tone::Normal),
        )
        .await
        .unwrap();
        assert_eq!(answer.lang, expected);
        assert_eq!(answer.refusal.unwrap().message, message);
    }
}

#[tokio::test]
async fn every_host_refusal_uses_the_selected_language_independent_of_tone() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("How does the service work?");
    for (tag, expected) in [
        (
            "en",
            [
                "The available passages do not answer the question.",
                "The answer could not be verified against the available evidence.",
                "No registered answerer is available for the requested model.",
                "No passage matched the question.",
                "The best passage was below the relevance threshold.",
            ],
        ),
        (
            "fr-CA",
            [
                "Les passages disponibles ne répondent pas à la question.",
                "La réponse n’a pas pu être vérifiée à partir des éléments disponibles.",
                "Aucun modèle de réponse enregistré n’est disponible pour ce modèle.",
                "Aucun passage ne correspond à la question.",
                "Le meilleur passage est sous le seuil de pertinence.",
            ],
        ),
        (
            "es-419",
            [
                "Los pasajes disponibles no responden a la pregunta.",
                "La respuesta no pudo verificarse con la evidencia disponible.",
                "No hay un modelo de respuesta registrado disponible para el modelo solicitado.",
                "Ningún pasaje coincide con la pregunta.",
                "El mejor pasaje está por debajo del umbral de relevancia.",
            ],
        ),
        (
            "ja",
            [
                "The available passages do not answer the question.",
                "The answer could not be verified against the available evidence.",
                "No registered answerer is available for the requested model.",
                "No passage matched the question.",
                "The best passage was below the relevance threshold.",
            ],
        ),
    ] {
        for tone in [Tone::Brief, Tone::Normal, Tone::Detailed] {
            let prompt = presented(PromptVersion::V2, Some(tag), tone);
            assert_refusals(&request, &answerer, &prompt, expected).await;
        }
    }
}

/// Every early exit and generated refusal keeps the same unchecked diagnostic.
async fn assert_refusals(
    request: &AskRequest,
    answerer: &RegisteredAnswerer,
    prompt: &AnswerPrompt,
    expected: [&str; 5],
) {
    for (index, code) in [
        RefusalCode::NotFound,
        RefusalCode::Unsupported,
        RefusalCode::AnswererUnavailable,
        RefusalCode::NoEvidence,
    ]
    .into_iter()
    .enumerate()
    {
        let evidence = if code == RefusalCode::NoEvidence {
            empty_bundle(&request.question)
        } else {
            bundle(&request.question, "en", "Sources are listed.")
        };
        let selected = (code != RefusalCode::AnswererUnavailable).then_some(answerer);
        let port = ScriptedPort::new(
            &[if code == RefusalCode::NotFound {
                "NOT_FOUND"
            } else {
                "Invent `unsafe --destroy` now. [1]"
            }; 2],
        );
        let answer = answer_bundle(&port, request, selected, evidence, prompt)
            .await
            .unwrap();
        assert_eq!(
            answer.lang,
            prompt.presentation().language.as_deref().unwrap()
        );
        assert_eq!(answer.language_check, LanguageCheck::Unchecked);
        let refusal = answer.refusal.unwrap();
        assert_eq!(refusal.code, code);
        assert_eq!(refusal.message, expected[index], "{prompt:?} {code:?}");
    }
    let port = ScriptedPort::new(&[]);
    let answer = answer_relevant(
        &port,
        request,
        Some(answerer),
        bundle(&request.question, "en", "Sources are listed."),
        AnswerPlan {
            min_rerank_score: Some(1.0),
            top_rerank_score: Some(0.0),
            answer_prompt: prompt,
        },
    )
    .await
    .unwrap();
    assert_eq!(answer.language_check, LanguageCheck::Unchecked);
    assert_eq!(answer.refusal.unwrap().message, expected[4]);
    assert_eq!(port.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn response_language_codes_name_each_host_language() {
    use crate::answer::types::ResponseLanguage;
    assert_eq!(ResponseLanguage::English.code(), "en");
    assert_eq!(ResponseLanguage::French.code(), "fr");
    assert_eq!(ResponseLanguage::Spanish.code(), "es");
}
