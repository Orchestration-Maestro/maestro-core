//! Terminal frames and semantic keys use the same draft as the plain flow.
use super::super::{
    flow::{Answer, FlowPort},
    terminal::Screen,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Cell, style::Color};
use std::collections::VecDeque;

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn catalog_terminal_five_screens_and_navigation() {
    let mut events = VecDeque::from([
        key(KeyCode::Char('x')),
        key(KeyCode::Tab),
        key(KeyCode::BackTab),
        key(KeyCode::Enter),
        key(KeyCode::Esc),
        key(KeyCode::Down),
        key(KeyCode::Enter),
        key(KeyCode::Char('d')),
        Event::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
    ]);
    let terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut screen = Screen::new(terminal, false, || Ok(events.pop_front().unwrap()));
    screen.screen("1/5 Workspace").unwrap();
    screen
        .show("Root: synthetic; trust defaults to no")
        .unwrap();
    assert_eq!(screen.ask("Catalog: ").unwrap(), Answer::Text("x".into()));
    screen.screen("2/5 Language").unwrap();
    assert_eq!(screen.ask("language: ").unwrap(), Answer::Back);
    screen.screen("3/5 Tone").unwrap();
    assert_eq!(screen.ask("tone: ").unwrap(), Answer::Back);
    screen.screen("4/5 All settings").unwrap();
    screen
        .show("synthetic = 3; accepts: 0..9; source: default; description")
        .unwrap();
    assert_eq!(screen.ask("KEY=VALUE: ").unwrap(), Answer::Cancel);
    screen.screen("5/5 Review").unwrap();
    screen.show("No unconfirmed writes").unwrap();
    screen.draw("Confirm [no]: ").unwrap();
    let frame = screen.terminal.backend().buffer();
    let text: String = frame.content.iter().map(Cell::symbol).collect();
    for label in [
        "Maestro",
        "5/5 Review",
        "No unconfirmed writes",
        "Back",
        "Cancel",
    ] {
        assert!(text.contains(label), "{text}");
    }
    for cell in &frame.content {
        assert_eq!(cell.fg, Color::Reset);
        assert_eq!(cell.bg, Color::Reset);
    }
}

/// Keep exact five-stage geometry and hierarchy visible in reviewable text snapshots.
#[test]
fn catalog_terminal_five_frame_snapshots() {
    for (name, title, body, label) in [
        (
            "workspace",
            "1/5 Workspace",
            "Root: synthetic\nMode: authoring convenience; not a verified install\nTrust: no",
            "Catalog: ",
        ),
        (
            "language",
            "2/5 Language",
            "language = en; source: default\nEnglish en; French fr; Spanish es; another \
            supported BCP 47 tag",
            "language [Enter keeps current]: ",
        ),
        (
            "tone",
            "3/5 Tone",
            "tone = normal; source: default\nbrief: Done. normal: The change is ready. \
            detailed: Here is the evidence.",
            "tone [Enter keeps current]: ",
        ),
        (
            "settings",
            "4/5 All settings",
            "synthetic_setting = 3\naccepts: 0..9; source: default; Synthetic \
            description\nEditable in project preferences.\nLocked: no file or \
            flag may edit.\nTrust is authority, not a preference.",
            "KEY=VALUE [Enter finishes]: ",
        ),
        (
            "review",
            "5/5 Review",
            "Root: synthetic\nConfig: language=en, tone=normal\nFile: \
            .maestro/config.toml — create\nFile: README.md — collision; no unconfirmed writes",
            "Confirm [no]: ",
        ),
    ] {
        let terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut screen = Screen::new(terminal, false, || Ok(key(KeyCode::Enter)));
        screen.screen(title).unwrap();
        screen.show(body).unwrap();
        screen.draw(label).unwrap();
        let mut text = String::new();
        for row in screen.terminal.backend().buffer().content.chunks(80) {
            text.extend(row.iter().map(Cell::symbol));
            text.push('\n');
        }
        let expected = match name {
            "workspace" => include_str!("../../../../../../tests/fixtures/terminal/workspace.json"),
            "language" => include_str!("../../../../../../tests/fixtures/terminal/language.json"),
            "tone" => include_str!("../../../../../../tests/fixtures/terminal/tone.json"),
            "settings" => include_str!("../../../../../../tests/fixtures/terminal/settings.json"),
            "review" => include_str!("../../../../../../tests/fixtures/terminal/review.json"),
            _ => panic!("unknown snapshot {name}"),
        };
        let rows: Vec<String> = serde_json::from_str(expected).unwrap();
        assert_eq!(text, rows.join("\n") + "\n", "{name}");
    }
}

#[test]
fn catalog_terminal_registry_refresh_invalid_focus_and_plain_parity() {
    use super::super::{
        flow::{self, Draft},
        plain::Plain,
    };
    use super::flow::registry;
    use maestro_settings::{LayerName, Layers};
    let input = "synthetic_setting=10\nsynthetic_setting=8\n\
        raw_prompt_logging=true\nsynthetic_standard=4\ntrust=true\n\n";
    for init in [false, true] {
        let mut plain =
            Draft::new(registry(), Layers::default(), LayerName::Project, &[], init).unwrap();
        let mut output = Vec::new();
        flow::editor(
            &mut Plain {
                input: &mut input.as_bytes(),
                output: &mut output,
            },
            &mut plain,
        )
        .unwrap();
        let mut events = VecDeque::new();
        for line in input.lines() {
            events.extend((0..64).map(|_| key(KeyCode::Backspace)));
            events.extend(line.chars().map(|ch| key(KeyCode::Char(ch))));
            events.push_back(key(KeyCode::Enter));
        }
        let mut draft =
            Draft::new(registry(), Layers::default(), LayerName::Project, &[], init).unwrap();
        let terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut screen = Screen::new(terminal, false, || Ok(events.pop_front().unwrap()));
        screen.screen("4/5 All settings").unwrap();
        flow::editor(&mut screen, &mut draft).unwrap();
        assert_eq!(draft.choices, plain.choices);
        assert_eq!(draft.choices, ["synthetic_setting=8"]);
        let text: String = screen
            .terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(Cell::symbol)
            .collect();
        assert!(
            text.contains("! Correct this field") && text.contains("unknown key"),
            "{text}"
        );
        screen.screen("4/5 All settings").unwrap();
        draft.show(&mut screen).unwrap();
        // Move the synthetic descriptor into view without adding a setting-specific screen.
        let terminal = Terminal::new(TestBackend::new(100, 400)).unwrap();
        let mut visible = Screen::new(terminal, false, || Ok(key(KeyCode::Enter)));
        draft.show(&mut visible).unwrap();
        visible.draw("KEY=VALUE: ").unwrap();
        let text: String = visible
            .terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(Cell::symbol)
            .collect();
        for field in [
            "synthetic_setting = 8",
            "accepts: a whole number",
            "Synthetic descriptor",
            "source: workspace",
            "Standard-only:",
            "Locked:",
        ] {
            assert!(text.contains(field), "missing {field}: {text}");
        }
    }
}

#[test]
fn catalog_terminal_error_retains_field_and_navigation_handles_all_events() {
    use crossterm::event::KeyEventKind;
    use std::io::{self, ErrorKind};
    let mut events = VecDeque::from([
        Ok(key(KeyCode::Char('é'))),
        Ok(key(KeyCode::Enter)),
        Ok(Event::Resize(100, 30)),
        Ok(Event::FocusLost),
        Ok(Event::FocusGained),
        Ok(Event::Key(KeyEvent {
            kind: KeyEventKind::Release,
            ..KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)
        })),
        Ok(key(KeyCode::Char('x'))),
        Ok(key(KeyCode::Enter)),
        Ok(key(KeyCode::Left)),
        Ok(key(KeyCode::Enter)),
        Ok(key(KeyCode::Up)),
        Ok(key(KeyCode::Right)),
        Ok(key(KeyCode::Tab)),
        Ok(key(KeyCode::Enter)),
        Err(io::Error::from(ErrorKind::UnexpectedEof)),
        Err(io::Error::from(ErrorKind::Interrupted)),
        Err(io::Error::from(ErrorKind::PermissionDenied)),
    ]);
    let terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut screen = Screen::new(terminal, false, || events.pop_front().unwrap());
    assert_eq!(screen.ask("field: ").unwrap(), Answer::Text("é".into()));
    screen.show("Error: invalid field").unwrap();
    screen.terminal.backend_mut().resize(100, 30);
    assert_eq!(screen.ask("field: ").unwrap(), Answer::Text("éx".into()));
    assert_eq!(screen.terminal.backend().buffer().area.width, 100);
    screen.show("Updated").unwrap();
    assert_eq!(screen.ask("field: ").unwrap(), Answer::Text(String::new()));
    assert_eq!(screen.ask("field: ").unwrap(), Answer::Back);
    assert_eq!(screen.ask("field: ").unwrap(), Answer::Cancel);
    assert_eq!(screen.ask("field: ").unwrap(), Answer::Cancel);
    assert!(screen.ask("field: ").is_err());
}

#[test]
fn catalog_terminal_palette_contrast_meets_d6() {
    fn luminance(rgb: [u8; 3]) -> f64 {
        let [red, green, blue] = rgb.map(|value| {
            let value = f64::from(value) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * red + 0.7152 * green + 0.0722 * blue
    }
    for (name, foreground, background, minimum) in [
        ("cream body", [242, 232, 220], [14, 11, 9], 4.5),
        ("sand headings", [217, 160, 102], [14, 11, 9], 4.5),
        ("copper labels", [200, 116, 58], [14, 11, 9], 4.5),
        ("ember focus", [255, 106, 26], [42, 31, 26], 4.5),
        ("slate boundary", [94, 115, 131], [14, 11, 9], 3.0),
        ("rust brand boundary", [183, 65, 14], [14, 11, 9], 3.0),
    ] {
        let contrast = (luminance(foreground) + 0.05) / (luminance(background) + 0.05);
        println!("{name}: {contrast:.2}:1 (minimum {minimum}:1)");
        assert!(contrast >= minimum, "{name}: {contrast}");
    }
}

#[test]
fn catalog_terminal_colored_frame_matches_measured_palette() {
    let terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut screen = Screen::new(terminal, true, || Ok(key(KeyCode::Enter)));
    screen.screen("Palette").unwrap();
    screen.show("Body").unwrap();
    screen.draw("Field: ").unwrap();
    let cells = &screen.terminal.backend().buffer().content;
    for color in [
        Color::Rgb(242, 232, 220),
        Color::Rgb(94, 115, 131),
        Color::Rgb(183, 65, 14),
        Color::Rgb(255, 106, 26),
        Color::Rgb(217, 160, 102),
        Color::Rgb(200, 116, 58),
    ] {
        assert!(
            cells.iter().any(|cell| cell.fg == color),
            "missing {color:?}"
        );
    }
    assert!(
        cells
            .iter()
            .all(|cell| [Color::Rgb(14, 11, 9), Color::Rgb(42, 31, 26)].contains(&cell.bg))
    );
}

#[test]
fn catalog_terminal_focus_controls_do_not_edit_text_and_error_refocuses() {
    let mut events = VecDeque::from([
        key(KeyCode::Right),
        key(KeyCode::Right),
        key(KeyCode::Char('x')),
        key(KeyCode::Backspace),
        key(KeyCode::Left),
        key(KeyCode::Enter),
        key(KeyCode::Right),
        key(KeyCode::Right),
        key(KeyCode::Enter),
        key(KeyCode::Char('y')),
        key(KeyCode::Right),
        key(KeyCode::Right),
        key(KeyCode::Right),
        key(KeyCode::Enter),
        key(KeyCode::PageDown),
        key(KeyCode::PageUp),
        key(KeyCode::Enter),
    ]);
    let terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut screen = Screen::new(terminal, false, || Ok(events.pop_front().unwrap()));
    assert_eq!(screen.ask("Field: ").unwrap(), Answer::Back);
    let text: String = screen
        .terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(Cell::symbol)
        .collect();
    assert!(!text.contains('x'));
    assert_eq!(screen.ask("Field: ").unwrap(), Answer::Cancel);
    assert_eq!(screen.ask("Field: ").unwrap(), Answer::Text("y".into()));
    screen.show("Error: invalid").unwrap();
    screen.draw("Field: ").unwrap();
    let text: String = screen
        .terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(Cell::symbol)
        .collect();
    assert!(
        text.contains("[> Edit <]") && text.contains("Error: invalid"),
        "{text}"
    );
    assert_eq!(screen.ask("Field: ").unwrap(), Answer::Text("y".into()));
}

#[test]
fn catalog_terminal_live_shrink_pauses_submission_and_restores_typed_field() {
    let mut events = VecDeque::from([
        key(KeyCode::Char('é')),
        key(KeyCode::Enter),
        key(KeyCode::Char('x')),
        key(KeyCode::Enter),
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        key(KeyCode::Enter),
        key(KeyCode::Esc),
    ]);
    let terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut screen = Screen::new(terminal, false, || Ok(events.pop_front().unwrap()));
    assert_eq!(screen.ask("Field: ").unwrap(), Answer::Text("é".into()));
    screen.show("Error: retain entered field").unwrap();
    screen.terminal.backend_mut().resize(40, 12);
    assert_eq!(screen.ask("Field: ").unwrap(), Answer::Cancel);
    let text: String = screen
        .terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(Cell::symbol)
        .collect();
    assert!(
        text.contains("Resize to at least 80×24") && text.contains("your draft is kept"),
        "{text}"
    );
    screen.terminal.backend_mut().resize(80, 24);
    assert_eq!(
        screen.ask("Field: ").unwrap(),
        Answer::Text("é".into()),
        "undersized input must not change text or submit"
    );
    screen.terminal.backend_mut().resize(40, 12);
    assert_eq!(
        screen.ask("Field: ").unwrap(),
        Answer::Cancel,
        "Esc cancels while undersized"
    );
}

#[test]
fn catalog_terminal_size_query_failure_cannot_submit() {
    use ratatui::{
        backend::{Backend, ClearType, WindowSize},
        layout::{Position, Size},
    };
    use std::{cell::Cell as Switch, io, rc::Rc};
    struct FailedSize(Rc<Switch<bool>>);
    impl Backend for FailedSize {
        type Error = io::Error;
        fn draw<'a, I>(&mut self, _: I) -> io::Result<()>
        where
            I: Iterator<Item = (u16, u16, &'a Cell)>,
        {
            Ok(())
        }
        fn hide_cursor(&mut self) -> io::Result<()> {
            Ok(())
        }
        fn show_cursor(&mut self) -> io::Result<()> {
            Ok(())
        }
        fn get_cursor_position(&mut self) -> io::Result<Position> {
            Ok(Position::ORIGIN)
        }
        fn set_cursor_position<P: Into<Position>>(&mut self, _: P) -> io::Result<()> {
            Ok(())
        }
        fn clear(&mut self) -> io::Result<()> {
            Ok(())
        }
        fn clear_region(&mut self, _: ClearType) -> io::Result<()> {
            Ok(())
        }
        fn size(&self) -> io::Result<Size> {
            if self.0.get() {
                Err(io::Error::other("injected size query failure"))
            } else {
                Ok(Size {
                    width: 80,
                    height: 24,
                })
            }
        }
        fn window_size(&mut self) -> io::Result<WindowSize> {
            Ok(WindowSize {
                columns_rows: self.size()?,
                pixels: Size::ZERO,
            })
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let failure = Rc::new(Switch::new(false));
    let terminal = Terminal::new(FailedSize(Rc::clone(&failure))).unwrap();
    let mut screen = Screen::new(terminal, false, || {
        failure.set(true); // Fail only after drawing, exactly at the submission boundary.
        Ok(key(KeyCode::Enter))
    });
    let error = screen.ask("Confirm [no]: ").unwrap_err().to_string();
    assert!(
        error.contains("cannot read terminal size; no answer submitted")
            && error.contains("injected size query failure"),
        "{error}"
    );
}

#[test]
fn catalog_terminal_editor_refresh_replaces_stale_registry_values() {
    use super::super::flow::{self, Draft};
    use super::flow::registry;
    use maestro_settings::{LayerName, Layers};
    let mut events: VecDeque<_> = "synthetic_setting=8"
        .chars()
        .map(|ch| key(KeyCode::Char(ch)))
        .collect();
    events.extend([key(KeyCode::Enter), key(KeyCode::Enter)]);
    let mut draft =
        Draft::new(registry(), Layers::default(), LayerName::Project, &[], true).unwrap();
    let terminal = Terminal::new(TestBackend::new(100, 400)).unwrap();
    let mut screen = Screen::new(terminal, false, || Ok(events.pop_front().unwrap()));
    flow::editor(&mut screen, &mut draft).unwrap();
    let text: String = screen
        .terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(Cell::symbol)
        .collect();
    assert!(
        text.contains("synthetic_setting = 8") && !text.contains("synthetic_setting = 3"),
        "stale registry frame"
    );
}
