//! Branded presentation only: all validation, planning and effects stay in the flow.
use super::{
    flow::{Answer, FlowPort},
    plain::Plain,
};
use crate::failure::Failure;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::{
    env,
    error::Error,
    io::{self, IsTerminal as _},
    time::Duration,
};

/// Restore even partial initialization and panic unwinding; never grant authority.
struct Cleanup;
impl Drop for Cleanup {
    fn drop(&mut self) {
        drop(terminal::disable_raw_mode());
        drop(execute!(io::stderr(), Show, LeaveAlternateScreen));
    }
}

/// Select one adapter and release it before any apply/trust handoff.
pub(in crate::cli) fn with_port<R>(
    plain: bool,
    color: bool,
    run: impl FnOnce(&mut dyn FlowPort) -> Result<R, Failure>,
) -> Result<R, Failure> {
    let stdin = io::stdin();
    let stderr = io::stderr();
    let size = terminal::size().unwrap_or((0, 0));
    if !eligible(
        plain,
        stdin.is_terminal(),
        stderr.is_terminal(),
        env::var("TERM").ok().as_deref(),
        size,
    ) {
        return run(&mut Plain {
            input: &mut stdin.lock(),
            output: &mut stderr.lock(),
        });
    }
    let cleanup = Cleanup;
    terminal::enable_raw_mode().map_err(|error| Failure::failed_by(&error))?;
    execute!(io::stderr(), EnterAlternateScreen, Hide)
        .map_err(|error| Failure::failed_by(&error))?;
    let terminal = Terminal::new(CrosstermBackend::new(io::stderr()))
        .map_err(|error| Failure::failed_by(&error))?;
    // Register resize/input readers before the first frame can be observed.
    event::poll(Duration::ZERO).map_err(|error| Failure::failed_by(&error))?;
    let result = run(&mut Screen::new(terminal, color, event::read));
    drop(cleanup);
    result
}

/// D6's plain fallback uses no terminal state and has the same semantic choices.
fn eligible(
    plain: bool,
    input: bool,
    output: bool,
    term: Option<&str>,
    (width, height): (u16, u16),
) -> bool {
    !plain && input && output && term != Some("dumb") && width >= 80 && height >= 24
}

/// An injectable backend and event reader exercise the real flow without a second planner.
pub(in crate::cli::init) struct Screen<B: Backend, F> {
    /// Renderer-owned terminal; tests inspect its frames.
    pub(in crate::cli::init) terminal: Terminal<B>,
    /// Read a backend event, never a setting-specific decision.
    read: F,
    /// Accessible monochrome suppresses all palette styles.
    color: bool,
    /// Shared flow stage title.
    title: String,
    /// Registry and planner text for this stage.
    content: String,
    /// Current editable field, retained on validation failure.
    input: String,
    /// Input, Back, Cancel, Submit in keyboard order.
    focus: usize,
    /// Scroll offset for long registry/plan content.
    scroll: u16,
    /// The flow rejected the last answer; keep its text and focus.
    error: String,
    /// Label of the last submitted field, so only same-field retries retain input.
    label: String,
    /// Whether the diagnostic has appeared in a full-sized frame.
    error_drawn: bool,
    /// Interface notice retained across stage initialization.
    notice: String,
}
impl<B: Backend, F: FnMut() -> io::Result<Event>> Screen<B, F>
where
    B::Error: Error,
{
    /// Presentation state only; no preferences or authority live here.
    pub(in crate::cli::init) fn new(terminal: Terminal<B>, color: bool, read: F) -> Self {
        Self {
            terminal,
            read,
            color,
            title: String::new(),
            content: String::new(),
            input: String::new(),
            focus: 0,
            scroll: 0,
            error: String::new(),
            label: String::new(),
            error_drawn: false,
            notice: String::new(),
        }
    }

    /// Fixed hierarchy with scrollable body, labelled input and explicit navigation.
    pub(in crate::cli::init) fn draw(&mut self, label: &str) -> Result<(), Failure> {
        let color = self.color;
        let body = palette(color, Color::Rgb(242, 232, 220), Color::Rgb(14, 11, 9));
        let border = palette(color, Color::Rgb(94, 115, 131), Color::Rgb(14, 11, 9));
        let accent = palette(color, Color::Rgb(183, 65, 14), Color::Rgb(14, 11, 9));
        let focus = palette(color, Color::Rgb(255, 106, 26), Color::Rgb(42, 31, 26));
        let secondary = palette(color, Color::Rgb(217, 160, 102), Color::Rgb(14, 11, 9));
        let copper = palette(color, Color::Rgb(200, 116, 58), Color::Rgb(14, 11, 9));
        let controls = ["Edit", "Back", "Cancel", "Submit"];
        let navigation = controls
            .iter()
            .enumerate()
            .map(|(index, name)| {
                if index == self.focus {
                    format!("[> {name} <]")
                } else {
                    format!("[ {name} ]")
                }
            })
            .collect::<Vec<_>>()
            .join("  ");
        self.terminal
            .draw(|frame| {
                if frame.area().width < 80 || frame.area().height < 24 {
                    frame.render_widget(
                        Paragraph::new(
                            "Resize to at least 80×24; your draft is kept; Esc or Ctrl-C cancels",
                        )
                        .wrap(Wrap { trim: false }),
                        frame.area(),
                    );
                    return;
                }
                let [heading, content, input, navigation_area] = Layout::vertical([
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(6),
                    Constraint::Length(3),
                ])
                .areas(frame.area());
                frame.render_widget(Block::default().style(body), frame.area());
                let header = Paragraph::new(format!("Maestro — {}", self.title))
                    .style(secondary)
                    .block(Block::default().borders(Borders::ALL).border_style(accent));
                frame.render_widget(header, heading);
                let details = Paragraph::new(format!("{}{}\n{label}", self.notice, self.content))
                    .style(body)
                    .wrap(Wrap { trim: false })
                    .scroll((self.scroll, 0))
                    .block(
                        Block::default()
                            .title(Line::styled("Details — PgUp/PgDn", copper))
                            .borders(Borders::ALL)
                            .border_style(border),
                    );
                frame.render_widget(details, content);
                let field = Paragraph::new(format!(
                    "{}\n{}\n{}",
                    label.lines().last().unwrap_or(label),
                    self.input,
                    self.error
                ))
                .style(body)
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(if self.error.is_empty() {
                            "> Input"
                        } else {
                            "! Correct this field"
                        })
                        .borders(Borders::ALL)
                        .border_style(if self.focus == 0 { focus } else { border }),
                );
                frame.render_widget(field, input);
                let actions = Paragraph::new(format!(
                    "{navigation}\nTab/arrows: focus · Enter: \
                choose · Esc: Back · Ctrl-C/D: cancel · PgUp/Dn: scroll"
                ))
                .style(focus)
                .wrap(Wrap { trim: false });
                frame.render_widget(actions, navigation_area);
            })
            .map_err(|error| Failure::failed_by(&error))?;
        let size = self.terminal.get_frame().area();
        if size.width >= 80 && size.height >= 24 {
            self.error_drawn = true;
        }
        Ok(())
    }

    /// Convert only keyboard/navigation events; the flow validates returned text.
    fn answer(&mut self, event: &Event, (width, height): (u16, u16)) -> Option<Answer> {
        let Event::Key(key) = event else {
            return None;
        };
        if key.kind == KeyEventKind::Release {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('c' | 'd') => Some(Answer::Cancel),
                _ => None,
            };
        }
        if width < 80 || height < 24 {
            return if key.code == KeyCode::Esc {
                Some(Answer::Cancel)
            } else {
                None
            };
        }
        match key.code {
            KeyCode::Esc => Some(Answer::Back),
            KeyCode::Tab | KeyCode::Down | KeyCode::Right => {
                self.focus = (self.focus + 1) % 4;
                None
            }
            KeyCode::BackTab | KeyCode::Up | KeyCode::Left => {
                self.focus = (self.focus + 3) % 4;
                None
            }
            KeyCode::PageDown => {
                self.scroll = self.scroll.saturating_add(10);
                None
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
                None
            }
            KeyCode::Enter => Some(match self.focus {
                1 => Answer::Back,
                2 => Answer::Cancel,
                _ => match self.input.trim().to_ascii_lowercase().as_str() {
                    "back" => Answer::Back,
                    "cancel" | "exit" => Answer::Cancel,
                    _ => Answer::Text(self.input.trim().to_owned()),
                },
            }),
            KeyCode::Backspace if self.focus == 0 => {
                self.input.pop();
                None
            }
            KeyCode::Char(ch) if self.focus == 0 => {
                self.input.push(ch);
                None
            }
            _ => None,
        }
    }
}
impl<B: Backend, F: FnMut() -> io::Result<Event>> FlowPort for Screen<B, F>
where
    B::Error: Error,
{
    fn screen(&mut self, title: &str) -> Result<(), Failure> {
        let (heading, content) = title.split_once('\n').unwrap_or((title, ""));
        heading.clone_into(&mut self.title);
        content.clone_into(&mut self.content);
        self.scroll = 0;
        if self.error_drawn {
            self.error.clear();
        }
        Ok(())
    }
    fn review_screen(&mut self, text: &str) -> Result<(), Failure> {
        self.screen(text)?;
        text.clone_into(&mut self.content);
        Ok(())
    }
    fn notice(&mut self, text: &str) -> Result<(), Failure> {
        self.notice = format!("{text}\n");
        Ok(())
    }
    fn refresh(&mut self) {
        self.content.clear();
        self.scroll = 0;
    }
    fn show(&mut self, text: &str) -> Result<(), Failure> {
        if text.starts_with("Error:") {
            text.clone_into(&mut self.error);
            self.error_drawn = false;
            self.focus = 0;
        } else {
            if self.error_drawn {
                self.error.clear();
            }
            self.content.push_str(text);
            self.content.push('\n');
        }
        Ok(())
    }
    fn plan(&mut self, text: &str) -> Result<(), Failure> {
        self.show(text)
    }
    fn ask(&mut self, label: &str) -> Result<Answer, Failure> {
        if self.error.is_empty() || self.label != label {
            self.input.clear();
            if self.error_drawn {
                self.error.clear();
            }
        }
        label.clone_into(&mut self.label);
        self.focus = 0;
        loop {
            self.draw(label)?;
            let event = match (self.read)() {
                Ok(event) => event,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::Interrupted | io::ErrorKind::UnexpectedEof
                    ) =>
                {
                    return Ok(Answer::Cancel);
                }
                Err(error) => return Err(Failure::failed_by(&error)),
            };
            let size = self.terminal.size().map_err(|error| {
                Failure::failed(format!(
                    "cannot read terminal size; no answer submitted: {error}"
                ))
            })?;
            let Some(answer) = self.answer(&event, (size.width, size.height)) else {
                continue;
            };
            if let Answer::Text(_) = &answer {
                self.error.clear();
            }
            if self.error_drawn {
                self.notice.clear();
            }
            return Ok(answer);
        }
    }
}

/// Monochrome uses labels and markers, with no foreground/background palette.
fn palette(enabled: bool, foreground: Color, background: Color) -> Style {
    if enabled {
        Style::default().fg(foreground).bg(background)
    } else {
        Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::eligible;

    #[test]
    fn catalog_terminal_selection_obeys_plain_capability_and_geometry() {
        for (plain, input, output, term, size, expected) in [
            (false, true, true, Some("xterm"), (80, 24), true),
            (true, true, true, Some("xterm"), (80, 24), false),
            (false, false, true, Some("xterm"), (80, 24), false),
            (false, true, false, Some("xterm"), (80, 24), false),
            (false, true, true, Some("dumb"), (80, 24), false),
            (false, true, true, Some("xterm"), (79, 24), false),
            (false, true, true, Some("xterm"), (80, 23), false),
            (false, true, true, None, (80, 24), true),
        ] {
            assert_eq!(eligible(plain, input, output, term, size), expected);
        }
    }
}
