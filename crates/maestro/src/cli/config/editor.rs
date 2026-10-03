//! No-argument config: shared registry-generated editor over S1 operations.
use super::change::{Change, Places, run_in_registry};
use crate::{
    cli::{
        init::{
            flow::{self, Answer, Draft, FlowPort, review},
            terminal::with_port,
        },
        output::Output,
    },
    failure::Failure,
    kernel::Kernel,
    settings::Session,
};
use maestro_kernel::settings::SettingChange;
use maestro_settings::{LayerName, parse_flags};
use std::process::ExitCode;

/// Layer selection is the same as config set; JSON callers use explicit commands.
pub(in crate::cli) fn run(
    output: Output,
    session: &Session,
    layer: LayerName,
    plain: bool,
) -> Result<ExitCode, Failure> {
    if output.is_json() {
        return Err(Failure::refused(
            "the config editor requires plain input; use `maestro config list`, `set` or \
                `unset` with --json",
        ));
    }
    let mut draft = Draft::new(
        session.registry.clone(),
        session.layers.clone(),
        layer,
        &[],
        false,
    )?;
    draft.output = output;
    draft.flags.clone_from(&session.flags);
    let places = Places::current()?;
    // Resolve the same target before prompting; forbidden project discovery never writes.
    super::change::target(layer, &places)?;
    if !with_port(plain, output.color(), |port| collect(port, &mut draft))? {
        return Ok(ExitCode::SUCCESS);
    }
    for flag in parse_flags(&draft.registry, &draft.choices).map_err(Failure::refused)? {
        let text = flag.value.to_string();
        run_in_registry(
            output,
            Change {
                key: &flag.key,
                value: Some(&text),
                layer,
            },
            (&places, &draft.registry),
            Kernel::open,
            journal,
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Preview/cancel never call an edit or journal; Back keeps the draft.
pub(in crate::cli::config) fn collect(
    port: &mut dyn FlowPort,
    draft: &mut Draft,
) -> Result<bool, Failure> {
    loop {
        port.screen("All settings — configuration editor")?;
        if matches!(flow::editor(port, draft)?, Answer::Cancel | Answer::Back) {
            return Ok(false);
        }
        port.review_screen(&format!(
            "Review {} preferences: {}",
            draft.layer.name(),
            draft.choices.join(", ")
        ))?;
        match review(port, true, draft.output)? {
            Answer::Back => {}
            Answer::Text(text) if text == "yes" => return Ok(true),
            _ => return Ok(false),
        }
    }
}

/// Reuse the existing kernel setting-change journal.
fn journal(kernel: &Kernel, record: &SettingChange) -> Result<(), String> {
    kernel
        .database
        .record_setting_change(record)
        .map(drop)
        .map_err(|error| error.to_string())
}
