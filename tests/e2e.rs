//! End-to-end tests: drive the app with key events, render to a test backend,
//! and persist through the real storage layer.

use chrono::NaiveDate;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use tsu_tui::app::App;
use tsu_tui::model::{Priority, Store};
use tsu_tui::{storage, ui};

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 9).unwrap()
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn type_str(app: &mut App, s: &str) {
    s.chars().for_each(|c| press(app, KeyCode::Char(c)));
}

fn screen(app: &mut App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).unwrap();
    terminal.draw(|f| ui::draw(f, app)).unwrap();
    let buf = terminal.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
                + "\n"
        })
        .collect()
}

/// Adds a task named `name` (`A` works whether or not tasks exist).
fn add_task(app: &mut App, name: &str) {
    press(app, KeyCode::Char('A'));
    type_str(app, name);
    press(app, KeyCode::Enter);
}

/// Adds a todo to the selected task; `prio_steps` presses → on the priority field.
fn add_todo(app: &mut App, title: &str, prio_steps: usize, deadline: &str) {
    press(app, KeyCode::Char('a'));
    type_str(app, title);
    press(app, KeyCode::Tab);
    (0..prio_steps).for_each(|_| press(app, KeyCode::Right));
    press(app, KeyCode::Tab);
    type_str(app, deadline);
    press(app, KeyCode::Enter);
}

fn app_with_todos() -> App {
    let mut app = App::new(Store::default(), today());
    // With no tasks yet, `a` opens the new-task form.
    press(&mut app, KeyCode::Char('a'));
    type_str(&mut app, "Release v1");
    press(&mut app, KeyCode::Enter);
    add_todo(&mut app, "Write docs", 1, "2026-10-08"); // Medium -> High, overdue
    add_todo(&mut app, "Tag release", 0, "");
    app
}

#[test]
fn add_task_and_todos_shows_progress() {
    let mut app = app_with_todos();
    let task = &app.store.tasks[0];
    assert_eq!(task.name, "Release v1");
    assert_eq!(task.todos.len(), 2);
    assert_eq!(task.todos[0].priority, Priority::High);
    assert_eq!(
        task.todos[0].deadline,
        Some(NaiveDate::from_ymd_opt(2026, 10, 8).unwrap())
    );
    assert_eq!(task.todos[1].deadline, None);

    let s = screen(&mut app);
    assert!(s.contains("Release v1"), "{s}");
    assert!(
        s.contains("Write docs") && s.contains("High") && s.contains("2026-10-08"),
        "{s}"
    );
    assert!(s.contains("0/2"), "{s}");

    // Toggle the first todo -> 50%.
    press(&mut app, KeyCode::Up);
    press(&mut app, KeyCode::Char(' '));
    assert!(app.store.tasks[0].todos[0].done);
    let s = screen(&mut app);
    assert!(s.contains("1/2") && s.contains("50%"), "{s}");
}

#[test]
fn invalid_deadline_keeps_form_open_with_error() {
    let mut app = App::new(Store::default(), today());
    add_task(&mut app, "T");
    add_todo(&mut app, "bad date", 0, "2026-13-40");
    assert!(app.store.tasks[0].todos.is_empty());
    assert!(screen(&mut app).contains("Invalid date"));
    press(&mut app, KeyCode::Esc);
    assert!(app.store.tasks[0].todos.is_empty());
}

#[test]
fn empty_names_are_rejected() {
    let mut app = App::new(Store::default(), today());
    add_task(&mut app, "   ");
    assert!(app.store.tasks.is_empty());
}

#[test]
fn edit_task_and_todo() {
    let mut app = app_with_todos();
    // Edit selected todo ("Tag release"): replace title, lower priority, set deadline.
    press(&mut app, KeyCode::Char('e'));
    (0.."Tag release".len()).for_each(|_| press(&mut app, KeyCode::Backspace));
    type_str(&mut app, "Publish");
    press(&mut app, KeyCode::Tab);
    press(&mut app, KeyCode::Left); // Medium -> Low
    press(&mut app, KeyCode::Tab);
    type_str(&mut app, "2026-10-20");
    press(&mut app, KeyCode::Enter);
    let todo = &app.store.tasks[0].todos[1];
    assert_eq!(todo.title, "Publish");
    assert_eq!(todo.priority, Priority::Low);
    assert_eq!(
        todo.deadline,
        Some(NaiveDate::from_ymd_opt(2026, 10, 20).unwrap())
    );

    // `E` renames the current task.
    press(&mut app, KeyCode::Char('E'));
    type_str(&mut app, "!");
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.store.tasks[0].name, "Release v1!");
}

#[test]
fn delete_requires_confirmation() {
    let mut app = app_with_todos();
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('n'));
    assert_eq!(app.store.tasks[0].todos.len(), 2);

    press(&mut app, KeyCode::Char('d'));
    assert!(screen(&mut app).contains("Delete"));
    press(&mut app, KeyCode::Char('y'));
    assert_eq!(app.store.tasks[0].todos.len(), 1);

    // `D` deletes the whole task.
    press(&mut app, KeyCode::Char('D'));
    assert!(screen(&mut app).contains("Delete task"));
    press(&mut app, KeyCode::Char('y'));
    assert!(app.store.tasks.is_empty());
    // Actions on an empty list must not panic.
    for c in ['e', 'd', ' ', 'E', 'D', 'j', 'k', '1'] {
        press(&mut app, KeyCode::Char(c));
    }
    for code in [KeyCode::Tab, KeyCode::Left, KeyCode::Right, KeyCode::Enter] {
        press(&mut app, code);
    }
    screen(&mut app);
}

#[test]
fn quit_and_persist_round_trip() {
    let path = std::env::temp_dir().join(format!("tsu-tui-e2e-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    assert!(
        storage::load(&path).unwrap().tasks.is_empty(),
        "missing file loads empty"
    );

    let mut app = app_with_todos();
    assert!(app.take_dirty());
    storage::save(&path, &app.store).unwrap();
    assert_eq!(storage::load(&path).unwrap(), app.store);

    press(&mut app, KeyCode::Char('q'));
    assert!(app.should_quit);

    std::fs::write(&path, "not json").unwrap();
    assert!(
        storage::load(&path).is_err(),
        "corrupt file must not be silently replaced"
    );
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn tabs_switch_between_tasks() {
    let mut app = app_with_todos();
    add_task(&mut app, "Groceries");
    add_todo(&mut app, "Buy milk", 0, "");
    assert_eq!(app.selected_task(), Some(1), "new task gets selected");

    // Both tasks appear as tabs; only the selected task's todos are listed.
    let s = screen(&mut app);
    assert!(s.contains("Release v1") && s.contains("Groceries"), "{s}");
    assert!(s.contains("Buy milk") && !s.contains("Write docs"), "{s}");

    press(&mut app, KeyCode::Left);
    let s = screen(&mut app);
    assert!(s.contains("Write docs") && !s.contains("Buy milk"), "{s}");

    // Tab / → wrap around; number keys jump directly.
    press(&mut app, KeyCode::Tab);
    assert_eq!(app.selected_task(), Some(1));
    press(&mut app, KeyCode::Right);
    assert_eq!(app.selected_task(), Some(0));
    press(&mut app, KeyCode::Char('2'));
    assert_eq!(app.selected_task(), Some(1));
    press(&mut app, KeyCode::Char('9')); // out of range: ignored
    assert_eq!(app.selected_task(), Some(1));

    // Space and Enter both toggle the selected todo.
    press(&mut app, KeyCode::Enter);
    assert!(app.store.tasks[1].todos[0].done);
    press(&mut app, KeyCode::Char(' '));
    assert!(!app.store.tasks[1].todos[0].done);
}

#[test]
fn layout_is_a_compact_left_aligned_card() {
    let mut app = app_with_todos();
    let s = screen(&mut app);
    let lines: Vec<&str> = s.lines().collect();
    // Card starts near the top-left corner, not at the screen edge.
    assert_eq!(lines[1].chars().nth(2), Some('╭'), "{s}");
    // It doesn't stretch to the right edge...
    assert!(
        lines.iter().all(|l| l.chars().skip(85).all(|c| c == ' ')),
        "{s}"
    );
    // ...nor to the bottom: with two todos the card ends well above the last rows.
    let bottom = lines.iter().rposition(|l| l.contains('╰')).unwrap();
    assert!(bottom < 16, "card bottom at row {bottom}\n{s}");
    // Key hints sit right under the card.
    assert!(
        lines[bottom + 1..].iter().any(|l| l.contains("quit")),
        "{s}"
    );
}

#[test]
fn deadlines_show_relative_due_labels() {
    let mut app = app_with_todos();
    add_todo(&mut app, "Today one", 0, "2026-10-09");
    add_todo(&mut app, "Later one", 0, "2026-10-14");
    let s = screen(&mut app);
    assert!(s.contains("1d late"), "{s}");
    assert!(s.contains("today"), "{s}");
    assert!(s.contains("in 5d"), "{s}");
}
