//! Renderer regressions for corrected fields, transitions and keyboard boundaries.
use super::super::{
    flow::{Answer, FlowPort},
    terminal::Screen,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, buffer::Cell, style::Color};
use std::{collections::VecDeque, fmt::Write as _, fs};

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn text<F>(screen: &Screen<TestBackend, F>) -> String {
    screen
        .terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(Cell::symbol)
        .collect()
}

#[test]
fn catalog_terminal_distinct_prompt_clears_corrected_field() {
    let mut events = VecDeque::from([
        key(KeyCode::Enter),
        key(KeyCode::Char('c')),
        key(KeyCode::Enter),
        key(KeyCode::Enter),
    ]);
    let mut screen = Screen::new(
        Terminal::new(TestBackend::new(80, 24)).unwrap(),
        false,
        || Ok(events.pop_front().unwrap()),
    );
    assert_eq!(
        screen.ask("Catalog: ").unwrap(),
        Answer::Text(String::new())
    );
    screen.show("Error: catalog required").unwrap();
    assert_eq!(screen.ask("Catalog: ").unwrap(), Answer::Text("c".into()));
    assert_eq!(screen.ask("Preset: ").unwrap(), Answer::Text(String::new()));
    assert!(!text(&screen).contains("catalog required"));
}

#[test]
fn catalog_terminal_transition_error_survives_until_draw() {
    for (stage, diagnostic) in [
        ("1/5 Workspace", "workspace preparation failed"),
        ("4/5 All settings", "review preparation failed"),
    ] {
        let mut screen = Screen::new(
            Terminal::new(TestBackend::new(80, 24)).unwrap(),
            false,
            || Ok(key(KeyCode::Enter)),
        );
        screen.show(&format!("Error: {diagnostic}")).unwrap();
        screen.screen(stage).unwrap();
        screen.refresh();
        screen.show("Descriptor refresh").unwrap();
        screen.ask("Retry: ").unwrap();
        assert!(text(&screen).contains(diagnostic), "{}", text(&screen));
        screen.screen("Next stage").unwrap();
        screen.draw("Next: ").unwrap();
        assert!(!text(&screen).contains(diagnostic));
    }
}

#[test]
fn catalog_terminal_scroll_and_plan_content_are_observable() {
    let mut events = VecDeque::from([
        key(KeyCode::PageDown),
        key(KeyCode::Enter),
        key(KeyCode::PageUp),
        key(KeyCode::Enter),
    ]);
    let mut screen = Screen::new(
        Terminal::new(TestBackend::new(80, 24)).unwrap(),
        false,
        || Ok(events.pop_front().unwrap()),
    );
    let mut plan = String::new();
    for index in 0..25 {
        writeln!(plan, "plan row {index:02}").unwrap();
    }
    screen.plan(&plan).unwrap();
    screen.ask("Review: ").unwrap();
    assert!(text(&screen).contains("plan row 10"));
    assert!(!text(&screen).contains("plan row 00"));
    screen.ask("Review: ").unwrap();
    assert!(text(&screen).contains("plan row 00"));
    assert!(!text(&screen).contains("plan row 20"));
}

#[test]
fn catalog_terminal_typed_navigation_and_submit_focus_guard() {
    for (input, answer) in [
        ("back", Answer::Back),
        ("cancel", Answer::Cancel),
        ("exit", Answer::Cancel),
        ("value", Answer::Text("value".into())),
    ] {
        let mut events: VecDeque<_> = input.chars().map(|ch| key(KeyCode::Char(ch))).collect();
        events.extend([
            key(KeyCode::Tab),
            key(KeyCode::Tab),
            key(KeyCode::Tab),
            key(KeyCode::Char('x')),
            key(KeyCode::Backspace),
            key(KeyCode::Enter),
        ]);
        let mut screen = Screen::new(
            Terminal::new(TestBackend::new(80, 24)).unwrap(),
            true,
            || Ok(events.pop_front().unwrap()),
        );
        assert_eq!(screen.ask("Field: ").unwrap(), answer);
        let buffer = screen.terminal.backend().buffer();
        assert_eq!(buffer[(0, 15)].fg, Color::Rgb(94, 115, 131));
    }
}

#[test]
fn catalog_terminal_each_undersized_dimension_blocks_draw_and_answer() {
    for (width, height) in [(79, 24), (80, 23)] {
        let mut events = VecDeque::from([
            key(KeyCode::Char('x')),
            key(KeyCode::Backspace),
            key(KeyCode::Enter),
            key(KeyCode::Esc),
        ]);
        let mut screen = Screen::new(
            Terminal::new(TestBackend::new(width, height)).unwrap(),
            false,
            || Ok(events.pop_front().unwrap()),
        );
        assert_eq!(screen.ask("Field: ").unwrap(), Answer::Cancel);
        assert!(text(&screen).contains("Resize to at least"));
        assert!(!text(&screen).contains("Maestro"));
    }
}

#[test]
fn catalog_terminal_long_review_remains_scrollable() {
    let mut screen = Screen::new(
        Terminal::new(TestBackend::new(80, 24)).unwrap(),
        false,
        || Ok(key(KeyCode::Enter)),
    );
    let review = format!(
        "Review user preferences: language=zh-Hant-TW, tone=detailed, {}updates=propose",
        "synthetic=value, ".repeat(8)
    );
    screen.review_screen(&review).unwrap();
    screen.ask("Confirm [no]: ").unwrap();
    assert!(
        text(&screen).contains("updates=propose"),
        "{}",
        text(&screen)
    );
}

#[test]
fn catalog_terminal_language_fallback_uses_flow_port() {
    use super::super::{
        command,
        flow::{self, Draft},
    };
    use crate::cli::output::Output;
    use maestro_catalog::settings::FilePreferences;
    use maestro_settings::{LayerName, Layers, Registry};
    use maestro_test_scratch::scratch_directory;
    for initial in [true, false] {
        let root = scratch_directory().unwrap();
        let source = FilePreferences::new(&root, &root);
        let mut events: VecDeque<_> = if initial { "" } else { "language=ja" }
            .chars()
            .map(|ch| key(KeyCode::Char(ch)))
            .collect();
        events.extend([key(KeyCode::Enter), key(KeyCode::Enter)]);
        let mut screen = Screen::new(
            Terminal::new(TestBackend::new(80, 24)).unwrap(),
            false,
            || Ok(events.pop_front().unwrap()),
        );
        let mut draft = Draft::new(
            Registry::built_in().unwrap(),
            Layers::default(),
            LayerName::Project,
            &[],
            true,
        )
        .unwrap();
        if initial {
            command::preference_output_to(
                Output::new(false),
                &source,
                &["language=ja".into()],
                |draft, output| draft.language_output_on(output, &mut screen),
            )
            .unwrap();
        }
        screen.screen("4/5 All settings").unwrap();
        flow::editor(&mut screen, &mut draft).unwrap();
        let visible = text(&screen);
        assert!(
            visible.contains("ja") && visible.contains("English"),
            "{visible}"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
