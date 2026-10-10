# tsu-tui
A simple todo TUI built with [ratatui](https://github.com/ratatui/ratatui).

Tasks contain todos; each todo has a priority (Low/Medium/High) and an optional deadline.
Tasks are shown as tabs in a compact card; each tab shows the task's progress and todos.

```sh
cargo run --release            # data in ~/.tsu-tui.json
cargo run --release -- my.json # custom data file
```

## Keys

| Key | Action |
| --- | --- |
| `←`/`→`, `h`/`l`, `Tab`/`Shift-Tab` | Switch task tab (wraps) |
| `1`–`9` | Jump to task tab |
| `j`/`k`, `↑`/`↓` | Move todo selection |
| `Space` / `x` / `Enter` | Toggle todo done |
| `a` / `e` / `d` | Add / edit / delete todo (delete asks `y/n`; `a` adds a task when there is none) |
| `A` / `E` / `D` | Add / rename / delete task |
| `t` | Toggle dark / light theme |
| `q`, `Ctrl-C` | Quit |

In the todo form: `Tab`/`↑`/`↓` switch field, `←`/`→` change priority, deadline is `YYYY-MM-DD` (blank = none),
`Enter` saves, `Esc` cancels. Overdue deadlines are shown in red with "Nd late", today's in yellow.

## Theme

Dark or light is picked from the terminal background at startup (OSC 11 query);
override with `TSU_THEME=dark|light`, or press `t` at runtime. The app paints its own
background, so contrast doesn't depend on the terminal's color scheme. All text meets
WCAG AA (4.5:1), checked by `tests/theme.rs`. Requires a truecolor terminal.

## Layout

```
src/model.rs    Task / Todo / Priority, progress & deadline logic
src/storage.rs  JSON load/save (atomic write)
src/app.rs      App state + key handling (no rendering)
src/ui.rs       Rendering only
src/theme.rs    Dark / light palettes + detection
src/main.rs     Terminal setup + event loop
tests/e2e.rs    Drives App with key events, renders via TestBackend
```
