//! Rendering. Pure function of `App` state (plus table scroll offsets).
//! Every color comes from `app.theme`; nothing relies on terminal defaults.
//!
//! Layout: a single card anchored top-left, at most `CARD_WIDTH` wide and only as
//! tall as its content, with key hints underneath. Tasks are tabs across the top of
//! the card; below them sit the selected task's progress and todos.

use chrono::NaiveDate;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, HighlightSpacing, Padding, Paragraph, Row, Table, Wrap,
};

use crate::app::{App, DeleteTarget, Mode, TaskForm, TodoField, TodoForm};
use crate::model::{DATE_FORMAT, Task, Todo};
use crate::theme::Theme;

const LABEL_WIDTH: u16 = 12;
const CARD_WIDTH: u16 = 76;
/// Rows above the todo list: tabs, tab rule, gap, progress, gap.
const HEADER_ROWS: usize = 5;
/// Card rows outside its content: two borders plus top and bottom padding.
const CARD_CHROME: usize = 4;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let t = app.theme;
    frame.render_widget(Block::new().style(t.base()), frame.area());

    let outer = frame.area().inner(Margin::new(2, 1));
    let card = card_area(app, outer);
    draw_card(frame, app, card);
    let status = Rect {
        x: outer.x + 1,
        y: card.bottom() + 1,
        width: outer.width.saturating_sub(1),
        height: 1,
    };
    draw_status(frame, app, status.intersection(frame.area()));

    match &app.mode {
        Mode::Normal => {}
        Mode::TaskForm(form) => draw_task_form(frame, &t, card, form),
        Mode::TodoForm(form) => draw_todo_form(frame, &t, card, form),
        Mode::ConfirmDelete(target) => draw_confirm(frame, app, card, *target),
    }
}

/// Top-left card sized to its content, leaving two rows below for the key hints.
fn card_area(app: &App, outer: Rect) -> Rect {
    let content = app
        .selected_task()
        .map_or(1, |i| HEADER_ROWS + app.store.tasks[i].todos.len().max(1));
    let max_height = usize::from(outer.height.saturating_sub(2));
    Rect {
        width: outer.width.min(CARD_WIDTH),
        height: (content + CARD_CHROME).min(max_height) as u16,
        ..outer
    }
}

fn highlight(t: &Theme) -> Style {
    Style::new().bg(t.selection).bold()
}

fn hint<'a>(t: &Theme, text: &'a str) -> Paragraph<'a> {
    Paragraph::new(text.fg(t.muted))
}

fn draw_card(frame: &mut Frame, app: &mut App, area: Rect) {
    let t = app.theme;
    let date = format!(" {} ", app.today.format("%a %b %-d"));
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(t.border)
        .style(t.card())
        .padding(Padding::new(2, 2, 1, 1))
        .title_top(Line::from(" tsu ".fg(t.accent).bold()))
        .title_top(Line::from(date.fg(t.muted)).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(i) = app.selected_task() else {
        frame.render_widget(hint(&t, "No tasks yet — press a to add one"), inner);
        return;
    };
    let [tabs, rule, _, progress, _, list] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(inner);
    draw_tabs(frame, &t, &app.store.tasks, i, tabs, rule);
    draw_progress(frame, &t, &app.store.tasks[i], app.today, progress);

    let task = &app.store.tasks[i];
    if task.todos.is_empty() {
        frame.render_widget(hint(&t, "No todos yet — press a to add one"), list);
        return;
    }
    // On narrow cards drop the relative-due column, then the date, so titles stay readable.
    let columns = match list.width {
        60.. => 5,
        46.. => 4,
        _ => 3,
    };
    let rows = task.todos.iter().map(|todo| {
        let cells = todo_cells(&t, todo, app.today);
        let row = Row::new(cells.into_iter().take(columns));
        // Done rows recede: muted text, struck through. Cell colors are kept so
        // priority remains distinguishable but secondary.
        if todo.done {
            row.fg(t.muted).crossed_out()
        } else {
            row
        }
    });
    let widths = [
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(8),
    ];
    let table = Table::new(rows, &widths[..columns])
        .column_spacing(2)
        .row_highlight_style(highlight(&t))
        .highlight_symbol("▌ ".fg(t.accent))
        .highlight_spacing(HighlightSpacing::Always);
    frame.render_stateful_widget(table, list, &mut app.todos_state);
}

/// Tab strip plus the rule beneath it; the selected tab is accent-colored and
/// underlined. Scrolls horizontally so the selected tab is always visible.
fn draw_tabs(
    frame: &mut Frame,
    t: &Theme,
    tasks: &[Task],
    selected: usize,
    area: Rect,
    rule: Rect,
) {
    let tabs: Vec<Line> = tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let name = if i == selected {
                task.name.as_str().fg(t.accent).bold()
            } else {
                task.name.as_str().fg(t.muted)
            };
            let complete = !task.todos.is_empty() && task.done_count() == task.todos.len();
            let check = if complete {
                " ✔".fg(t.green)
            } else {
                "".into()
            };
            Line::from(vec![" ".into(), name, check, " ".into()])
        })
        .collect();
    let widths: Vec<usize> = tabs.iter().map(Line::width).collect();
    let (first, last) = visible_tabs(&widths, selected, usize::from(area.width));

    let mut labels = Vec::new();
    let mut marks = Vec::new();
    if first > 0 {
        labels.push("‹ ".fg(t.muted));
        marks.push("──".fg(t.border));
    }
    for i in first..last {
        labels.extend(tabs[i].spans.iter().cloned());
        marks.push(if i == selected {
            "━".repeat(widths[i]).fg(t.accent)
        } else {
            "─".repeat(widths[i]).fg(t.border)
        });
    }
    if last < tabs.len() {
        labels.push(" ›".fg(t.muted));
    }
    let used: usize = marks.iter().map(Span::width).sum();
    marks.push(
        "─"
            .repeat(usize::from(rule.width).saturating_sub(used))
            .fg(t.border),
    );
    frame.render_widget(Line::from(labels), area);
    frame.render_widget(Line::from(marks), rule);
}

/// Range `first..last` of tabs that fits in `width`, scrolled just far enough to
/// include `selected`. On overflow, two columns per side are kept for arrows.
fn visible_tabs(widths: &[usize], selected: usize, width: usize) -> (usize, usize) {
    if widths.iter().sum::<usize>() <= width {
        return (0, widths.len());
    }
    let room = width.saturating_sub(4);
    let mut first = 0;
    while first < selected && widths[first..=selected].iter().sum::<usize>() > room {
        first += 1;
    }
    let (mut last, mut used) = (first, 0);
    while last < widths.len() && (last == first || used + widths[last] <= room) {
        used += widths[last];
        last += 1;
    }
    (first, last)
}

fn draw_progress(frame: &mut Frame, t: &Theme, task: &Task, today: NaiveDate, area: Rect) {
    let mut stats = vec![
        format!("{:.0}%", task.progress() * 100.0).fg(t.fg).bold(),
        format!("  {}/{} done", task.done_count(), task.todos.len()).fg(t.muted),
    ];
    let overdue = task.todos.iter().filter(|x| x.is_overdue(today)).count();
    if overdue > 0 {
        stats.push(format!("  {overdue} overdue").fg(t.red).bold());
    }
    let stats = Line::from(stats);
    let [bar, _, label] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(2),
        Constraint::Length(stats.width() as u16),
    ])
    .areas(area);

    let width = usize::from(bar.width);
    let filled = (task.progress() * width as f64).round() as usize;
    let color = if task.progress() >= 1.0 {
        t.green
    } else {
        t.accent
    };
    let bar_line = Line::from(vec![
        "━".repeat(filled).fg(color),
        "━".repeat(width - filled).fg(t.track),
    ]);
    frame.render_widget(bar_line, bar);
    frame.render_widget(stats, label);
}

/// Check, title, priority, deadline date, relative due label.
fn todo_cells<'a>(t: &Theme, todo: &'a Todo, today: NaiveDate) -> [Cell<'a>; 5] {
    let check = if todo.done {
        "✔".fg(t.green)
    } else {
        "○".fg(t.muted)
    };
    let color = t.priority(todo.priority);
    let priority = Line::from(vec!["● ".fg(color), todo.priority.label().fg(color)]);

    let (date, due) = match todo.deadline {
        None => (Cell::from("—").fg(t.muted), Cell::from("")),
        Some(d) => {
            let date = d.format(DATE_FORMAT).to_string();
            let days = (d - today).num_days();
            match () {
                _ if todo.done => (Cell::from(date).fg(t.muted), Cell::from("")),
                _ if days < 0 => (
                    Cell::from(date).fg(t.red).bold(),
                    Cell::from(format!("{}d late", -days)).fg(t.red).bold(),
                ),
                _ if days == 0 => (
                    Cell::from(date).fg(t.yellow).bold(),
                    Cell::from("today").fg(t.yellow).bold(),
                ),
                _ if days == 1 => (Cell::from(date), Cell::from("tomorrow")),
                _ => (
                    Cell::from(date),
                    Cell::from(format!("in {days}d")).fg(t.muted),
                ),
            }
        }
    };
    [
        Cell::from(check),
        Cell::from(todo.title.as_str()),
        Cell::from(priority),
        date,
        due,
    ]
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    if let Some(message) = &app.message {
        frame.render_widget(Line::from(message.as_str().fg(t.red).bold()), area);
        return;
    }
    let hints: &[(&str, &str)] = match (&app.mode, app.selected_task()) {
        (Mode::Normal, None) => &[("a", "new task"), ("t", "theme"), ("q", "quit")],
        (Mode::Normal, Some(_)) => &[
            ("←/→", "switch"),
            ("j/k", "move"),
            ("space", "done"),
            ("a/e/d", "todo"),
            ("A/E/D", "task"),
            ("t", "theme"),
            ("q", "quit"),
        ],
        (Mode::TaskForm(_), _) => &[("enter", "save"), ("esc", "cancel")],
        (Mode::TodoForm(_), _) => &[
            ("tab", "next field"),
            ("←/→", "priority"),
            ("enter", "save"),
            ("esc", "cancel"),
        ],
        (Mode::ConfirmDelete(_), _) => &[("y", "delete"), ("n", "cancel")],
    };
    let spans = hints.iter().flat_map(|(key, desc)| {
        [
            format!("{key} ").fg(t.accent).bold(),
            format!("{desc}   ").fg(t.muted),
        ]
    });
    frame.render_widget(Line::from_iter(spans), area);
}

/// Clears and frames a popup centered on the card, returning its inner area.
fn popup(frame: &mut Frame, t: &Theme, card: Rect, title: &str, width: u16, height: u16) -> Rect {
    let anchor = Rect {
        height: card.height.max(height),
        ..card
    }
    .intersection(frame.area());
    let area = anchor.centered(Constraint::Length(width), Constraint::Length(height));
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(t.accent)
        .title(format!(" {title} ").fg(t.accent).bold())
        .style(t.card());
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    inner
}

/// One `label  value` form line; the value starts at column `LABEL_WIDTH`.
fn field_line<'a>(t: &Theme, label: &'a str, value: Vec<Span<'a>>, active: bool) -> Line<'a> {
    let label = format!(
        "{} {:<width$}",
        if active { "›" } else { " " },
        label,
        width = LABEL_WIDTH as usize - 2
    );
    let label = if active {
        label.fg(t.accent).bold()
    } else {
        label.fg(t.muted)
    };
    Line::from_iter(std::iter::once(label).chain(value))
}

fn draw_task_form(frame: &mut Frame, t: &Theme, card: Rect, form: &TaskForm) {
    let title = if form.editing.is_some() {
        "Rename Task"
    } else {
        "New Task"
    };
    let inner = popup(frame, t, card, title, 50, 3);
    frame.render_widget(
        field_line(t, "Name", vec![form.name.as_str().into()], true),
        inner,
    );
    frame.set_cursor_position((
        inner.x + LABEL_WIDTH + Span::raw(&form.name).width() as u16,
        inner.y,
    ));
}

fn draw_todo_form(frame: &mut Frame, t: &Theme, card: Rect, form: &TodoForm) {
    let title = if form.editing.is_some() {
        "Edit Todo"
    } else {
        "New Todo"
    };
    let inner = popup(frame, t, card, title, 56, 6);
    let is = |field| form.field == field;
    let priority = vec![
        "◀ ".fg(t.muted),
        form.priority.label().fg(t.priority(form.priority)).bold(),
        " ▶".fg(t.muted),
    ];
    let deadline = if form.deadline.is_empty() && !is(TodoField::Deadline) {
        vec!["none".fg(t.muted)]
    } else {
        vec![form.deadline.as_str().into()]
    };
    let lines = vec![
        field_line(
            t,
            "Title",
            vec![form.title.as_str().into()],
            is(TodoField::Title),
        ),
        field_line(t, "Priority", priority, is(TodoField::Priority)),
        field_line(t, "Deadline", deadline, is(TodoField::Deadline)),
        Line::from(
            format!(
                "{:width$}YYYY-MM-DD, blank = none",
                "",
                width = LABEL_WIDTH as usize
            )
            .fg(t.muted),
        ),
    ];
    frame.render_widget(Paragraph::new(lines), inner);

    let cursor = match form.field {
        TodoField::Title => Some((&form.title, 0)),
        TodoField::Deadline => Some((&form.deadline, 2)),
        TodoField::Priority => None,
    };
    if let Some((text, row)) = cursor {
        frame.set_cursor_position((
            inner.x + LABEL_WIDTH + Span::raw(text).width() as u16,
            inner.y + row,
        ));
    }
}

fn draw_confirm(frame: &mut Frame, app: &App, card: Rect, target: DeleteTarget) {
    let t = app.theme;
    let Some(i) = app.selected_task() else { return };
    let task = &app.store.tasks[i];
    let text = match (target, app.selected_todo()) {
        (DeleteTarget::Todo, Some(j)) => format!("Delete todo \"{}\"?", task.todos[j].title),
        _ => format!(
            "Delete task \"{}\" and its {} todo(s)?",
            task.name,
            task.todos.len()
        ),
    };
    let inner = popup(frame, &t, card, "Confirm", 50, 5);
    let body = vec![
        Line::from(text),
        Line::from(""),
        Line::from(vec![
            "y".fg(t.red).bold(),
            " delete · ".fg(t.muted),
            "n".fg(t.accent).bold(),
            " cancel".fg(t.muted),
        ]),
    ];
    frame.render_widget(Paragraph::new(body).wrap(Wrap { trim: true }), inner);
}

#[cfg(test)]
mod tests {
    use super::visible_tabs;

    #[test]
    fn visible_tabs_scroll_to_keep_selection_in_view() {
        assert_eq!(visible_tabs(&[5, 5, 5], 2, 20), (0, 3), "all fit");
        // Width 14 leaves room 10 on overflow: two tabs at a time.
        assert_eq!(visible_tabs(&[5, 5, 5, 5], 0, 14), (0, 2));
        assert_eq!(visible_tabs(&[5, 5, 5, 5], 3, 14), (2, 4));
        // A tab wider than the room is still shown alone.
        assert_eq!(visible_tabs(&[5, 30, 5], 1, 14), (1, 2));
    }
}
