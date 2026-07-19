//! Status bar widget showing runtime status, address, and rule count.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::tui::state::AppState;

/// Status bar widget.
#[derive(Debug)]
pub struct StatusBar;

impl StatusBar {
    /// Creates a new StatusBar widget.
    pub fn new() -> Self {
        Self
    }

    /// Renders the status bar.
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, state: &AppState) {
        let status_text = match state.runtime_status {
            mock_runtime::RuntimeStatus::Stopped => "Stopped",
            mock_runtime::RuntimeStatus::Starting => "Starting...",
            mock_runtime::RuntimeStatus::Running => "Running",
            mock_runtime::RuntimeStatus::Reloading => "Reloading...",
            mock_runtime::RuntimeStatus::Failed => "Failed",
        };

        let status_color = match state.runtime_status {
            mock_runtime::RuntimeStatus::Stopped => Color::Gray,
            mock_runtime::RuntimeStatus::Starting => Color::Yellow,
            mock_runtime::RuntimeStatus::Running => Color::Green,
            mock_runtime::RuntimeStatus::Reloading => Color::Yellow,
            mock_runtime::RuntimeStatus::Failed => Color::Red,
        };

        let address_text = state
            .server_address
            .as_deref()
            .unwrap_or("not bound");

        let rule_count_text = format!("{} rules", state.rule_count);

        let spans = vec![
            Span::styled(" status: ", Style::default().fg(Color::Gray)),
            Span::styled(status_text, Style::default().fg(status_color)),
            Span::styled(" | address: ", Style::default().fg(Color::Gray)),
            Span::styled(address_text, Style::default().fg(Color::White)),
            Span::styled(" | ", Style::default().fg(Color::Gray)),
            Span::styled(&rule_count_text, Style::default().fg(Color::Cyan)),
        ];

        let line = Line::from(spans);

        let widget = Paragraph::new(line)
            .style(Style::default().bg(Color::DarkGray))
            .alignment(Alignment::Left);

        frame.render_widget(widget, area);
    }
}

impl Default for StatusBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_new() {
        let bar = StatusBar::new();
        assert!(true); // Just verify it constructs
    }
}
