//! Application state and key handling. Rendering lives in `ui`.

use chrono::NaiveDate;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;

use crate::model::{DATE_FORMAT, Priority, Store, Task, Todo, parse_deadline};
use crate::theme::Theme;

/// What a pending delete confirmation applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteTarget {
    Task,
    Todo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoField {
    Title,
    Priority,
    Deadline,
}

impl TodoField {
    const ALL: [Self; 3] = [Self::Title, Self::Priority, Self::Deadline];

    fn step(self, delta: isize) -> Self {
        let i = Self::ALL.iter().position(|&f| f == self).unwrap_or(0) as isize;
        Self::ALL[(i + delta).rem_euclid(Self::ALL.len() as isize) as usize]
    }
}

/// Form for creating (`editing: None`) or renaming a task.
#[derive(Debug)]
pub struct TaskForm {
    pub editing: Option<usize>,
    pub name: String,
}

/// Form for creating (`editing: None`) or editing a todo of the selected task.
#[derive(Debug)]
pub struct TodoForm {
    pub editing: Option<usize>,
    pub title: String,
    pub priority: Priority,
    pub deadline: String,
    pub field: TodoField,
}

#[derive(Debug)]
pub enum Mode {
    Normal,
    TaskForm(TaskForm),
    TodoForm(TodoForm),
    /// Confirm deleting the selected task or todo.
    ConfirmDelete(DeleteTarget),
}

pub struct App {
    pub store: Store,
    pub today: NaiveDate,
    /// Selected task, i.e. the active tab.
    task: Option<usize>,
    pub todos_state: TableState,
    pub mode: Mode,
    /// One-shot message for the status bar; cleared on the next key press.
    pub message: Option<String>,
    pub should_quit: bool,
    pub theme: Theme,
    dirty: bool,
}

impl App {
    pub fn new(store: Store, today: NaiveDate) -> Self {
        let mut app = Self {
            store,
            today,
            task: None,
            todos_state: TableState::default(),
            mode: Mode::Normal,
            message: None,
            should_quit: false,
            theme: Theme::default(),
            dirty: false,
        };
        let first = (!app.store.tasks.is_empty()).then_some(0);
        app.select_task(first);
        app
    }

    /// Returns whether the store changed since the last call.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn selected_task(&self) -> Option<usize> {
        self.task.filter(|&i| i < self.store.tasks.len())
    }

    pub fn selected_todo(&self) -> Option<usize> {
        let task = self.selected_task()?;
        self.todos_state
            .selected()
            .filter(|&i| i < self.store.tasks[task].todos.len())
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        self.message = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match self.mode {
            Mode::Normal => self.handle_normal(key.code),
            Mode::TaskForm(_) => self.handle_task_form(key.code),
            Mode::TodoForm(_) => self.handle_todo_form(key.code),
            Mode::ConfirmDelete(target) => {
                if matches!(key.code, KeyCode::Char('y' | 'Y') | KeyCode::Enter) {
                    self.delete_selected(target);
                }
                self.mode = Mode::Normal;
            }
        }
    }

    fn handle_normal(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('t') => self.theme = self.theme.toggled(),
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => self.cycle_task(1),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => self.cycle_task(-1),
            KeyCode::Char(c @ '1'..='9') => {
                let i = c as usize - '1' as usize;
                if i < self.store.tasks.len() {
                    self.select_task(Some(i));
                }
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_todo(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_todo(-1),
            KeyCode::Char(' ' | 'x') | KeyCode::Enter => self.toggle_done(),
            // With no task to hold a todo, `a` creates the first task instead.
            KeyCode::Char('a') if self.selected_task().is_none() => self.open_task_form(None),
            KeyCode::Char('a') => self.open_todo_form(None),
            KeyCode::Char('e') => {
                if let Some(i) = self.selected_todo() {
                    self.open_todo_form(Some(i));
                }
            }
            KeyCode::Char('d') if self.selected_todo().is_some() => {
                self.mode = Mode::ConfirmDelete(DeleteTarget::Todo);
            }
            KeyCode::Char('A') => self.open_task_form(None),
            KeyCode::Char('E') => {
                if let Some(i) = self.selected_task() {
                    self.open_task_form(Some(i));
                }
            }
            KeyCode::Char('D') if self.selected_task().is_some() => {
                self.mode = Mode::ConfirmDelete(DeleteTarget::Task);
            }
            _ => {}
        }
    }

    /// Moves to the next/previous tab, wrapping around.
    fn cycle_task(&mut self, delta: isize) {
        let len = self.store.tasks.len();
        if let Some(i) = self.selected_task() {
            let next = (i as isize + delta).rem_euclid(len as isize) as usize;
            self.select_task(Some(next));
        }
    }

    fn move_todo(&mut self, delta: isize) {
        let len = self
            .selected_task()
            .map_or(0, |t| self.store.tasks[t].todos.len());
        if len == 0 {
            return;
        }
        let next = self
            .selected_todo()
            .map_or(0, |i| i.saturating_add_signed(delta).min(len - 1));
        self.todos_state.select(Some(next));
    }

    /// Selects a task and resets the todo cursor to its first todo.
    fn select_task(&mut self, index: Option<usize>) {
        self.task = index;
        let has_todos = index.is_some_and(|i| !self.store.tasks[i].todos.is_empty());
        self.todos_state = TableState::default().with_selected(has_todos.then_some(0));
    }

    fn open_task_form(&mut self, editing: Option<usize>) {
        let name = editing.map_or_else(String::new, |i| self.store.tasks[i].name.clone());
        self.mode = Mode::TaskForm(TaskForm { editing, name });
    }

    /// Opens the todo form for the selected task; `editing` is a todo index.
    fn open_todo_form(&mut self, editing: Option<usize>) {
        let Some(t) = self.selected_task() else {
            return;
        };
        let form = match editing {
            None => TodoForm {
                editing,
                title: String::new(),
                priority: Priority::default(),
                deadline: String::new(),
                field: TodoField::Title,
            },
            Some(i) => {
                let todo = &self.store.tasks[t].todos[i];
                TodoForm {
                    editing,
                    title: todo.title.clone(),
                    priority: todo.priority,
                    deadline: todo
                        .deadline
                        .map(|d| d.format(DATE_FORMAT).to_string())
                        .unwrap_or_default(),
                    field: TodoField::Title,
                }
            }
        };
        self.mode = Mode::TodoForm(form);
    }

    fn handle_task_form(&mut self, code: KeyCode) {
        let Mode::TaskForm(form) = &mut self.mode else {
            return;
        };
        match code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => self.submit_task_form(),
            KeyCode::Backspace => {
                form.name.pop();
            }
            KeyCode::Char(c) => form.name.push(c),
            _ => {}
        }
    }

    fn submit_task_form(&mut self) {
        let Mode::TaskForm(form) = &self.mode else {
            return;
        };
        let (editing, name) = (form.editing, form.name.trim().to_string());
        if name.is_empty() {
            self.message = Some("Task name cannot be empty".into());
            return;
        }
        match editing {
            Some(i) => self.store.tasks[i].name = name,
            None => {
                self.store.tasks.push(Task::new(name));
                self.select_task(Some(self.store.tasks.len() - 1));
            }
        }
        self.mode = Mode::Normal;
        self.dirty = true;
    }

    fn handle_todo_form(&mut self, code: KeyCode) {
        let Mode::TodoForm(form) = &mut self.mode else {
            return;
        };
        match (code, form.field) {
            (KeyCode::Esc, _) => self.mode = Mode::Normal,
            (KeyCode::Enter, _) => self.submit_todo_form(),
            (KeyCode::Tab | KeyCode::Down, _) => form.field = form.field.step(1),
            (KeyCode::BackTab | KeyCode::Up, _) => form.field = form.field.step(-1),
            (KeyCode::Left | KeyCode::Char('h'), TodoField::Priority) => {
                form.priority = form.priority.prev()
            }
            (KeyCode::Right | KeyCode::Char('l'), TodoField::Priority) => {
                form.priority = form.priority.next()
            }
            (KeyCode::Backspace, TodoField::Title) => {
                form.title.pop();
            }
            (KeyCode::Backspace, TodoField::Deadline) => {
                form.deadline.pop();
            }
            (KeyCode::Char(c), TodoField::Title) => form.title.push(c),
            (KeyCode::Char(c), TodoField::Deadline) => form.deadline.push(c),
            _ => {}
        }
    }

    fn submit_todo_form(&mut self) {
        let (Mode::TodoForm(form), Some(task)) = (&self.mode, self.selected_task()) else {
            return;
        };
        let title = form.title.trim().to_string();
        let result = if title.is_empty() {
            Err("Todo title cannot be empty".to_string())
        } else {
            parse_deadline(&form.deadline)
        };
        let deadline = match result {
            Ok(deadline) => deadline,
            Err(e) => {
                self.message = Some(e);
                return;
            }
        };
        let (editing, priority) = (form.editing, form.priority);
        let todos = &mut self.store.tasks[task].todos;
        match editing {
            Some(i) => {
                let todo = &mut todos[i];
                (todo.title, todo.priority, todo.deadline) = (title, priority, deadline);
            }
            None => {
                todos.push(Todo {
                    title,
                    priority,
                    deadline,
                    done: false,
                });
                self.todos_state.select(Some(todos.len() - 1));
            }
        }
        self.mode = Mode::Normal;
        self.dirty = true;
    }

    fn toggle_done(&mut self) {
        if let (Some(t), Some(i)) = (self.selected_task(), self.selected_todo()) {
            let todo = &mut self.store.tasks[t].todos[i];
            todo.done = !todo.done;
            self.dirty = true;
        }
    }

    fn delete_selected(&mut self, target: DeleteTarget) {
        match (target, self.selected_task(), self.selected_todo()) {
            (DeleteTarget::Task, Some(t), _) => {
                self.store.tasks.remove(t);
                self.select_task(clamp(t, self.store.tasks.len()));
            }
            (DeleteTarget::Todo, Some(t), Some(i)) => {
                let todos = &mut self.store.tasks[t].todos;
                todos.remove(i);
                self.todos_state.select(clamp(i, todos.len()));
            }
            _ => return,
        }
        self.dirty = true;
    }
}

/// Keeps an index inside a list of `len` items after a removal.
fn clamp(index: usize, len: usize) -> Option<usize> {
    len.checked_sub(1).map(|last| index.min(last))
}
