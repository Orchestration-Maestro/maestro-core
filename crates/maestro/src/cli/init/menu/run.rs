//! Init orchestration over the shared draft, plain port and unchanged planner.
use super::super::{
    command::{self, ApplyChoices},
    flow::{self, Answer, Draft, FlowPort, review},
    plain::terminal,
    terminal::with_port,
};
use crate::{
    cli::{output::Output, session, trust},
    failure::Failure,
    presentation::messages::MessageKey,
};
use maestro_catalog::{
    limits::Limits,
    settings::{PreferencesDraft, WorkspacePreferences},
};
use maestro_settings::LayerName;
use std::{
    env,
    io::{self, IsTerminal as _},
    path::{Path, PathBuf},
    process::ExitCode,
};

/// Explicit script/rendering choices, not authority.
#[derive(Clone, Copy)]
pub(in crate::cli) struct Request<'a> {
    /// Explicit catalog location, or an interactive choice.
    pub(in crate::cli) catalog: Option<&'a Path>,
    /// Explicit composed preset names.
    pub(in crate::cli) presets: &'a [String],
    /// Force the sequential plain adapter.
    pub(in crate::cli) plain: bool,
    /// Accept scripted choices without prompting.
    pub(in crate::cli) yes: bool,
    /// Explicit apply/preferences-only choices.
    pub(in crate::cli) effects: ApplyChoices<'a>,
}

/// C47a's init entry never opens a runtime session or admits a stale lock.
pub(in crate::cli) fn run(
    output: Output,
    request: Request<'_>,
    choices: &[String],
) -> Result<ExitCode, Failure> {
    let stdin = io::stdin();
    let terminal = terminal(stdin.is_terminal(), io::stdout().is_terminal());
    if scripted_mode(output, &request, terminal) {
        if request.effects.apply && !request.yes && !request.effects.preferences_only {
            return Err(Failure::refused(
                "non-interactive apply requires --yes; it never grants trust",
            ));
        }
        return command::scripted(
            output,
            request.catalog,
            request.presets,
            request.effects,
            choices,
        );
    }
    let source = session::init_preferences()?;
    let root = env::current_dir()
        .and_then(|root| root.canonicalize())
        .map_err(|error| Failure::failed_by(&error))?;
    let reviewed = with_port(request.plain, output.color(), |port| {
        interactive(output, &request, choices, port, (&source, &root))
    })?;
    let Some(reviewed) = reviewed else {
        return Ok(ExitCode::SUCCESS);
    };
    // Release renderer IO locks before the existing administration adapter takes them.
    let prepared = match reviewed.plan {
        ReviewedPlan::Preferences(draft, choices) => {
            return command::apply_preferences(
                reviewed.output,
                &root,
                &request.effects,
                (&draft, &choices),
            );
        }
        ReviewedPlan::Authoring(prepared) => prepared,
    };
    if command::require_trust(reviewed.output, &root).is_err() {
        trust::run(
            reviewed.output,
            &trust::TrustCommand::Add {
                directory: root,
                confirm_path: None,
            },
        )?;
    }
    prepared.apply(reviewed.output)?;
    Ok(ExitCode::SUCCESS)
}

/// A confirmed review carries pinned bytes but no trust or persistence capability.
pub(super) struct Reviewed {
    /// Pinned reviewed bytes, without an authorization capability.
    plan: ReviewedPlan,
    /// Localized result presentation.
    output: Output,
}

/// Declining trust never reviews or applies template/projection files.
enum ReviewedPlan {
    /// Full C05 owned-file plan.
    Authoring(Box<command::Prepared>),
    /// Only the separately confirmed root-local preferences record.
    Preferences(PreferencesDraft, Vec<String>),
}

/// Five sequential stages; Back retains all draft choices, errors retain the stage.
pub(super) fn interactive(
    output: Output,
    request: &Request<'_>,
    choices: &[String],
    port: &mut dyn FlowPort,
    (source, root): (&dyn WorkspacePreferences, &Path),
) -> Result<Option<Reviewed>, Failure> {
    let mut output = command::preference_output(output, source, choices)?;
    let mut catalog = request.catalog.map(Path::to_path_buf);
    let mut presets = request.presets.to_vec();
    let mut trust_choice = false;
    let mut draft: Option<Draft> = None;
    let mut stage = 0_u8;
    loop {
        let answer = match stage {
            0 => initialize(
                port,
                (root, output),
                (&mut catalog, &mut presets, &mut trust_choice),
                (source, choices),
                &mut draft,
            )?,
            1 => {
                port.screen(&output.wording(MessageKey::FlowLanguage, &[])?)?;
                flow::preference(
                    port,
                    draft_mut(&mut draft)?,
                    "language",
                    "English en; French fr; Spanish es; or another supported BCP 47 tag. \
                        Artifacts/logs stay English.",
                )?
            }
            2 => {
                port.screen(&output.wording(MessageKey::FlowTone, &[])?)?;
                flow::preference(
                    port,
                    draft_mut(&mut draft)?,
                    "tone",
                    "brief: 'Done.'; normal: 'The change is ready.'; detailed: 'The change is \
                        ready; here is the evidence.' Artifacts/logs stay English.",
                )?
            }
            3 => {
                port.screen(&output.wording(MessageKey::FlowSettings, &[])?)?;
                flow::editor(port, draft_mut(&mut draft)?)?
            }
            _ => {
                port.screen(&output.wording(MessageKey::FlowReview, &[])?)?;
                let selected = catalog
                    .as_deref()
                    .ok_or_else(|| Failure::failed("missing catalog draft"))?;
                let choices = &draft_mut(&mut draft)?.choices;
                let plan = match review_plan(
                    trust_choice,
                    (selected, &presets),
                    (source, root),
                    choices,
                    port,
                ) {
                    Ok(plan) => plan,
                    Err(error) => {
                        port.show(&format!("Error: {error}"))?;
                        stage = 3;
                        continue;
                    }
                };
                match review(port, request.effects.apply, output)? {
                    Answer::Text(action) if action == "yes" => {
                        return Ok(Some(Reviewed { plan, output }));
                    }
                    Answer::Text(_) | Answer::Cancel => return Ok(None),
                    Answer::Back => Answer::Back,
                }
            }
        };
        match answer {
            Answer::Cancel => return Ok(None),
            Answer::Back if stage == 0 => return Ok(None),
            Answer::Back => stage -= 1,
            Answer::Text(text) => {
                let draft = draft_mut(&mut draft)?;
                if stage == 0 || (stage == 1 && !text.is_empty()) {
                    draft.output = draft.language_output(output)?;
                }
                output = draft.output;
                stage += 1;
            }
        }
    }
}

/// Review the exact effect chosen at the trust stage, never hypothetical template writes.
fn review_plan(
    trusted: bool,
    (catalog, presets): (&Path, &[String]),
    (source, root): (&dyn WorkspacePreferences, &Path),
    choices: &[String],
    port: &mut dyn FlowPort,
) -> Result<ReviewedPlan, Failure> {
    if trusted {
        let prepared = command::prepare(catalog, presets, source, choices)?;
        port.show(&prepared.review_values()?)?;
        port.plan(&prepared.text(false)?)?;
        Ok(ReviewedPlan::Authoring(Box::new(prepared)))
    } else {
        let draft = command::prepare_preferences(root, source, choices)?;
        port.show(&command::review_values(root, Some(&draft))?)?;
        port.plan("Trust declined: preferences-only review; no template or projection writes.")?;
        port.plan(
            &serde_json::to_string_pretty(&draft).map_err(|error| Failure::failed_by(&error))?,
        )?;
        Ok(ReviewedPlan::Preferences(draft, choices.to_vec()))
    }
}

/// Keep typed choices while selecting a catalog with a different admitted defaults slot.
fn initialize(
    port: &mut dyn FlowPort,
    (root, output): (&Path, Output),
    (catalog, presets, trust_choice): (&mut Option<PathBuf>, &mut Vec<String>, &mut bool),
    (source, choices): (&dyn WorkspacePreferences, &[String]),
    draft: &mut Option<Draft>,
) -> Result<Answer, Failure> {
    loop {
        let answer = workspace(port, (root, output), catalog, presets, trust_choice)?;
        if !matches!(answer, Answer::Text(_)) {
            return Ok(answer);
        }
        let selected = catalog
            .as_deref()
            .ok_or_else(|| Failure::failed("missing catalog draft"))?;
        let registry = if *trust_choice {
            match command::prepare(selected, presets, source, choices) {
                Ok(prepared) => prepared.registry,
                Err(error) => {
                    port.show(&format!("Error: {error}"))?;
                    continue;
                }
            }
        } else {
            source.registry().map_err(Failure::refused)?
        };
        let layers = source
            .layers(&registry, &Limits::PRODUCTION)
            .map_err(Failure::refused)?;
        let retained = draft.as_ref().map_or(choices, |draft| &draft.choices);
        *draft = Some(Draft::new(
            registry,
            layers,
            LayerName::Project,
            retained,
            true,
        )?);
        return Ok(answer);
    }
}

/// Navigation reaches settings only after a validated workspace stage.
fn draft_mut(draft: &mut Option<Draft>) -> Result<&mut Draft, Failure> {
    draft
        .as_mut()
        .ok_or_else(|| Failure::failed("missing settings draft"))
}

/// Explicit location and preset have no invented defaults.
fn workspace(
    port: &mut dyn FlowPort,
    (root, output): (&Path, Output),
    catalog: &mut Option<PathBuf>,
    presets: &mut Vec<String>,
    trust_choice: &mut bool,
) -> Result<Answer, Failure> {
    port.screen(&format!(
        "{}\nRoot: {}\nMode: authoring convenience; not a verified \
        install\nPresets: {}\nExisting approval: {}",
        output.wording(MessageKey::FlowWorkspace, &[])?,
        root.display(),
        presets.join(", "),
        command::require_trust(Output::new(false), root).is_ok()
    ))?;
    loop {
        let current = catalog
            .as_ref()
            .map_or(String::new(), |path| path.display().to_string());
        match port.ask(&format!("Reviewed catalog directory [{current}]: "))? {
            Answer::Text(text) => {
                if !text.is_empty() {
                    *catalog = Some(PathBuf::from(text));
                }
                if catalog.is_some() {
                    break;
                }
                port.show("Error: select a reviewed catalog directory; there is no default.")?;
            }
            answer => return Ok(answer),
        }
    }
    loop {
        match port.ask(&format!(
            "Preset name(s), comma-separated [{}]: ",
            presets.join(", ")
        ))? {
            Answer::Text(text) => {
                if !text.is_empty() {
                    *presets = text
                        .split(',')
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(str::to_owned)
                        .collect();
                }
                if !presets.is_empty() {
                    break;
                }
                port.show("Error: select at least one explicit preset.")?;
            }
            answer => return Ok(answer),
        }
    }
    loop {
        match port.ask(
            "Trust this exact workspace for confirmed apply? [y/N] (decline permits \
            only separately confirmed preferences): ",
        )? {
            Answer::Text(text)
                if matches!(
                    text.to_ascii_lowercase().as_str(),
                    "" | "n" | "no" | "y" | "yes"
                ) =>
            {
                *trust_choice = text.eq_ignore_ascii_case("y") || text.eq_ignore_ascii_case("yes");
                return Ok(Answer::Text(text));
            }
            Answer::Text(_) => port.show("Error: answer yes or no; trust defaults to no.")?,
            answer => return Ok(answer),
        }
    }
}

/// Scripted selection is independent of terminal rendering and never grants authority.
fn scripted_mode(output: Output, request: &Request<'_>, terminal: bool) -> bool {
    request.yes
        || output.is_json()
        || (!request.plain && !terminal)
        || request.effects.preferences_only
}

#[cfg(test)]
mod tests {
    use super::{Request, scripted_mode};
    use crate::cli::{init::ApplyChoices, output::Output};

    #[test]
    fn catalog_init_menu_scripted_choices_are_independent_of_terminal_detection() {
        let mut request = Request {
            catalog: None,
            presets: &[],
            plain: false,
            yes: false,
            effects: ApplyChoices {
                apply: false,
                preferences_only: false,
                non_interactive: false,
                confirm_path: None,
            },
        };
        assert!(!scripted_mode(Output::new(false), &request, true));
        assert!(scripted_mode(Output::new(false), &request, false));
        request.plain = true;
        assert!(!scripted_mode(Output::new(false), &request, false));
        request.plain = false;
        request.yes = true;
        assert!(scripted_mode(Output::new(false), &request, true));
        request.yes = false;
        assert!(scripted_mode(Output::new(true), &request, true));
        request.effects.preferences_only = true;
        assert!(scripted_mode(Output::new(false), &request, true));
    }
}
