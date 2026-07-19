//! Help bar widget showing keyboard shortcuts.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::tui::state::AppState;

/// Help bar widget showing keyboard shortcuts.
#[derive(Debug)]
pub struct HelpBar {
    /// Optional title.
    title: String,
}

impl HelpBar {
    /// Creates a new HelpBar widget.
    pub fn new() -> Self {
        Self {
            title: "Help".to_string(),
        }
    }

    /// Renders the help bar.
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, _state: &AppState) {
        let shortcuts = vec![
            ("q", "quit"),
            ("r", "reload config"),
            ("c", "clear history"),
            ("j/k", "navigate"),
            ("↑↓", "navigate"),
            ("/", "filter"),
            ("Tab", "switch panel"),
            ("Enter", "view details"),
            ("?", "toggle help"),
        ];

        let mut lines: Vec<Line> = Vec::new();

        for (key, desc) in shortcuts {
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {: <4}", key),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc, Style::default().fg(Color::White)),
            ]));
        }

        let content = Paragraph::new(lines)
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .block(
                Block::default()
                    .title(self.title.as_str())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White)),
            );

        frame.render_widget(content, area);
    }
}

impl Default for HelpBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_bar_new() {
        let help = HelpBar::new();
        assert_eq!(help.title, "Help");
    }
}
