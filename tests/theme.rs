//! Readability check: every text glyph on screen must meet WCAG AA contrast (4.5:1)
//! against its own background, in both themes and in every UI state.

use chrono::NaiveDate;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Color, Modifier};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use tsu_tui::app::App;
use tsu_tui::model::Store;
use tsu_tui::theme::{Theme, ThemeMode};
use tsu_tui::ui;

fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn keys(app: &mut App, s: &str) {
    s.chars().for_each(|c| press(app, KeyCode::Char(c)));
}

fn render(app: &mut App) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    terminal.draw(|f| ui::draw(f, app)).unwrap();
    terminal.backend().buffer().clone()
}

fn luminance(color: Color) -> f64 {
    let Color::Rgb(r, g, b) = color else {
        panic!("expected an explicit RGB color, got {color:?}")
    };
    let lin = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn contrast(a: Color, b: Color) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Asserts the whole screen is painted with theme colors and all text is readable.
fn assert_readable(buf: &Buffer, state: &str) {
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            let (mut fg, mut bg) = (cell.fg, cell.bg);
            if cell.modifier.contains(Modifier::REVERSED) {
                (fg, bg) = (bg, fg);
            }
            assert!(
                matches!(bg, Color::Rgb(..)),
                "{state}: unpainted background at ({x},{y})"
            );
            let is_text = cell
                .symbol()
                .chars()
                .any(|c| c.is_alphanumeric() || c.is_ascii_punctuation());
            if is_text {
                let ratio = contrast(fg, bg);
                assert!(
                    ratio >= 4.5,
                    "{state}: '{}' at ({x},{y}) has contrast {ratio:.2} ({fg:?} on {bg:?})",
                    cell.symbol()
                );
            }
        }
    }
}

/// Renders every distinct UI state and checks it.
fn check_all_states(theme: Theme) {
    let today = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap();
    let mut app = App::new(Store::default(), today);
    app.theme = theme;
    let name = format!("{:?}", theme.mode);
    assert_readable(&render(&mut app), &format!("{name}/empty"));

    press(&mut app, KeyCode::Char('a'));
    keys(&mut app, "Release");
    assert_readable(&render(&mut app), &format!("{name}/task form"));
    press(&mut app, KeyCode::Enter);
    assert_readable(&render(&mut app), &format!("{name}/task without todos"));

    for (title, deadline) in [
        ("Overdue", "2026-10-01"),
        ("Today", "2026-10-09"),
        ("Later", "2026-11-01"),
        ("None", ""),
    ] {
        press(&mut app, KeyCode::Char('a'));
        keys(&mut app, title);
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Right);
        press(&mut app, KeyCode::Tab);
        assert_readable(&render(&mut app), &format!("{name}/todo form"));
        keys(&mut app, deadline);
        press(&mut app, KeyCode::Enter);
    }
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Char(' ')); // one done todo
    assert_readable(&render(&mut app), &format!("{name}/todos"));

    press(&mut app, KeyCode::Char('d'));
    assert_readable(&render(&mut app), &format!("{name}/confirm todo"));
    press(&mut app, KeyCode::Char('n'));
    press(&mut app, KeyCode::Char('D'));
    assert_readable(&render(&mut app), &format!("{name}/confirm task"));
    press(&mut app, KeyCode::Char('n'));

    press(&mut app, KeyCode::Char('a'));
    press(&mut app, KeyCode::Enter); // empty title -> error message
    assert_readable(&render(&mut app), &format!("{name}/error message"));
    press(&mut app, KeyCode::Esc);

    press(&mut app, KeyCode::Char('A'));
    keys(&mut app, "Second");
    press(&mut app, KeyCode::Enter);
    assert_readable(&render(&mut app), &format!("{name}/second tab selected"));
    press(&mut app, KeyCode::Left);
    assert_readable(&render(&mut app), &format!("{name}/first tab selected"));
}

#[test]
fn dark_theme_is_readable() {
    check_all_states(Theme::DARK);
}

#[test]
fn light_theme_is_readable() {
    check_all_states(Theme::LIGHT);
}

#[test]
fn t_toggles_theme() {
    let mut app = App::new(
        Store::default(),
        NaiveDate::from_ymd_opt(2026, 10, 9).unwrap(),
    );
    app.theme = Theme::DARK;
    keys(&mut app, "t");
    assert_eq!(app.theme.mode, ThemeMode::Light);
    keys(&mut app, "t");
    assert_eq!(app.theme.mode, ThemeMode::Dark);
}
