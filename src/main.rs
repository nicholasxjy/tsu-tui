use anyhow::Result;
use chrono::Local;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use std::path::Path;
use tsu_tui::{app::App, storage, theme::Theme, ui};

fn main() -> Result<()> {
    let path = storage::data_path();
    let mut app = App::new(storage::load(&path)?, Local::now().date_naive());
    app.theme = Theme::detect(); // queries the terminal, so before raw mode
    ratatui::run(|terminal| run(terminal, &mut app, &path))
}

fn run(terminal: &mut DefaultTerminal, app: &mut App, path: &Path) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.handle_key(key);
        }
        if app.take_dirty()
            && let Err(e) = storage::save(path, &app.store)
        {
            app.message = Some(format!("Save failed: {e:#}"));
        }
    }
    Ok(())
}
