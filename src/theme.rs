//! Dark and light palettes. The UI paints its own background with these colors, so
//! contrast never depends on the terminal's color scheme. All text colors meet WCAG AA
//! (4.5:1) against `bg`, `surface` and `selection` (verified by `tests/theme.rs`).

use ratatui::style::{Color, Style};

use crate::model::Priority;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    pub mode: ThemeMode,
    /// Screen background around the card.
    pub bg: Color,
    /// Card and popup background.
    pub surface: Color,
    /// Background of the selected row.
    pub selection: Color,
    pub fg: Color,
    /// Secondary text: hints, counts, done todos.
    pub muted: Color,
    pub accent: Color,
    /// Unfocused borders (decorative, not text).
    pub border: Color,
    /// Unfilled part of progress bars (decorative, not text).
    pub track: Color,
    pub red: Color,
    pub yellow: Color,
    pub green: Color,
}

impl Theme {
    pub const DARK: Self = Self {
        mode: ThemeMode::Dark,
        bg: Color::from_u32(0x181825),
        surface: Color::from_u32(0x1e1e2e),
        selection: Color::from_u32(0x363a4f),
        fg: Color::from_u32(0xcdd6f4),
        muted: Color::from_u32(0xa6adc8),
        accent: Color::from_u32(0x89b4fa),
        border: Color::from_u32(0x585b70),
        track: Color::from_u32(0x45475a),
        red: Color::from_u32(0xf38ba8),
        yellow: Color::from_u32(0xf9e2af),
        green: Color::from_u32(0xa6e3a1),
    };

    pub const LIGHT: Self = Self {
        mode: ThemeMode::Light,
        bg: Color::from_u32(0xeceef3),
        surface: Color::from_u32(0xffffff),
        selection: Color::from_u32(0xe1e4ec),
        fg: Color::from_u32(0x2e3142),
        muted: Color::from_u32(0x5c5f77),
        accent: Color::from_u32(0x1d4ed8),
        border: Color::from_u32(0xa8acbd),
        track: Color::from_u32(0xd4d7e1),
        red: Color::from_u32(0xb4232c),
        yellow: Color::from_u32(0x8a5300),
        green: Color::from_u32(0x2b6e1f),
    };

    pub fn of(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::DARK,
            ThemeMode::Light => Self::LIGHT,
        }
    }

    /// `TSU_THEME=dark|light` wins; otherwise ask the terminal for its background; default dark.
    /// Must run before the terminal enters raw/alternate-screen mode.
    pub fn detect() -> Self {
        let from_env = std::env::var("TSU_THEME").ok().and_then(|v| parse_mode(&v));
        let mode =
            from_env.unwrap_or_else(
                || match terminal_colorsaurus::theme_mode(Default::default()) {
                    Ok(terminal_colorsaurus::ThemeMode::Light) => ThemeMode::Light,
                    _ => ThemeMode::Dark,
                },
            );
        Self::of(mode)
    }

    pub fn toggled(self) -> Self {
        Self::of(match self.mode {
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
        })
    }

    /// Default text on the screen background.
    pub fn base(&self) -> Style {
        Style::new().fg(self.fg).bg(self.bg)
    }

    /// Default text on a card or popup.
    pub fn card(&self) -> Style {
        Style::new().fg(self.fg).bg(self.surface)
    }

    pub fn priority(&self, priority: Priority) -> Color {
        match priority {
            Priority::High => self.red,
            Priority::Medium => self.yellow,
            Priority::Low => self.green,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::DARK
    }
}

fn parse_mode(name: &str) -> Option<ThemeMode> {
    match name.trim().to_ascii_lowercase().as_str() {
        "dark" => Some(ThemeMode::Dark),
        "light" => Some(ThemeMode::Light),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mode_is_case_insensitive_and_rejects_unknown() {
        assert_eq!(parse_mode(" Light "), Some(ThemeMode::Light));
        assert_eq!(parse_mode("DARK"), Some(ThemeMode::Dark));
        assert_eq!(parse_mode("auto"), None);
    }
}
