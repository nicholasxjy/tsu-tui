//! Domain types: a `Store` holds tasks, each task holds todos.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub const DATE_FORMAT: &str = "%Y-%m-%d";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Low,
    #[default]
    Medium,
    High,
}

impl Priority {
    pub fn next(self) -> Self {
        match self {
            Self::Low => Self::Medium,
            Self::Medium | Self::High => Self::High,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::High => Self::Medium,
            Self::Medium | Self::Low => Self::Low,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Todo {
    pub title: String,
    pub priority: Priority,
    pub deadline: Option<NaiveDate>,
    pub done: bool,
}

impl Todo {
    pub fn is_overdue(&self, today: NaiveDate) -> bool {
        !self.done && self.deadline.is_some_and(|d| d < today)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub name: String,
    pub todos: Vec<Todo>,
}

impl Task {
    pub fn new(name: String) -> Self {
        Self {
            name,
            todos: Vec::new(),
        }
    }

    pub fn done_count(&self) -> usize {
        self.todos.iter().filter(|t| t.done).count()
    }

    /// Completion ratio in `0.0..=1.0`; a task without todos is 0%.
    pub fn progress(&self) -> f64 {
        if self.todos.is_empty() {
            0.0
        } else {
            self.done_count() as f64 / self.todos.len() as f64
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Store {
    pub tasks: Vec<Task>,
}

/// Parses a `YYYY-MM-DD` deadline; blank input means "no deadline".
pub fn parse_deadline(input: &str) -> Result<Option<NaiveDate>, String> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(None);
    }
    NaiveDate::parse_from_str(input, DATE_FORMAT)
        .map(Some)
        .map_err(|_| format!("Invalid date '{input}', expected YYYY-MM-DD"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn todo(done: bool, deadline: Option<NaiveDate>) -> Todo {
        Todo {
            title: "t".into(),
            priority: Priority::Medium,
            deadline,
            done,
        }
    }

    #[test]
    fn progress_counts_done_todos() {
        let mut task = Task::new("x".into());
        assert_eq!(task.progress(), 0.0);
        task.todos = vec![
            todo(true, None),
            todo(false, None),
            todo(true, None),
            todo(false, None),
        ];
        assert_eq!(task.done_count(), 2);
        assert_eq!(task.progress(), 0.5);
    }

    #[test]
    fn priority_cycles_are_clamped() {
        assert_eq!(Priority::Low.prev(), Priority::Low);
        assert_eq!(Priority::Low.next(), Priority::Medium);
        assert_eq!(Priority::High.next(), Priority::High);
        assert_eq!(Priority::High.prev(), Priority::Medium);
    }

    #[test]
    fn overdue_only_when_open_and_past_deadline() {
        let today = date(2026, 10, 9);
        assert!(todo(false, Some(date(2026, 10, 8))).is_overdue(today));
        assert!(!todo(false, Some(today)).is_overdue(today));
        assert!(!todo(true, Some(date(2026, 10, 8))).is_overdue(today));
        assert!(!todo(false, None).is_overdue(today));
    }

    #[test]
    fn parse_deadline_accepts_blank_and_iso_dates() {
        assert_eq!(parse_deadline("  "), Ok(None));
        assert_eq!(parse_deadline(" 2026-10-09 "), Ok(Some(date(2026, 10, 9))));
        assert!(parse_deadline("2026-02-30").is_err());
        assert!(parse_deadline("10/09/2026").is_err());
    }
}
