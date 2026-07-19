//! Request details widget showing request/response headers, body preview, and matched rule.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::state::AppState;

/// Maximum body preview size (32 KiB).
const MAX_BODY_PREVIEW: usize = 32 * 1024;

/// Request details widget.
#[derive(Debug)]
pub struct RequestDetails {
    /// Optional title.
    title: String,
}

impl RequestDetails {
    /// Creates a new RequestDetails widget.
    pub fn new() -> Self {
        Self {
            title: "Request Details".to_string(),
        }
    }

    /// Renders the request details panel.
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, state: &AppState) {
        let filtered = state.filtered_requests();
        let selected = match state.selected_request {
            Some(idx) if idx < filtered.len() => filtered[idx],
            _ => {
                let placeholder = Paragraph::new("No request selected")
                    .style(Style::default().fg(Color::Gray))
                    .block(
                        Block::default()
                            .title(self.title.as_str())
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::White)),
                    );
                frame.render_widget(placeholder, area);
                return;
            }
        };

        let mut lines: Vec<Line> = Vec::new();

        // Request section header
        lines.push(Line::from(vec![
            Span::styled("─── Request ───", Style::default().fg(Color::Yellow)),
        ]));

        // Method and path
        lines.push(Line::from(vec![
            Span::styled("Method: ", Style::default().fg(Color::Cyan)),
            Span::raw(&selected.method),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Path: ", Style::default().fg(Color::Cyan)),
            Span::raw(&selected.path),
        ]));

        // Status and latency
        lines.push(Line::from(vec![
            Span::styled("Status: ", Style::default().fg(Color::Cyan)),
            Span::styled(status_str(selected.status), Style::default().fg(status_color(selected.status))),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Latency: ", Style::default().fg(Color::Cyan)),
            Span::raw(format!("{}ms", selected.latency_ms)),
        ]));

        // Decision type
        lines.push(Line::from(vec![
            Span::styled("Decision: ", Style::default().fg(Color::Cyan)),
            Span::styled(
                selected.decision_type.as_str(),
                Style::default().fg(decision_color(&selected.decision_type)),
            ),
        ]));

        // Rule ID
        lines.push(Line::from(vec![
            Span::styled("Rule: ", Style::default().fg(Color::Cyan)),
            Span::raw(selected.rule_id.as_deref().unwrap_or("-")),
        ]));

        // Request ID
        lines.push(Line::from(vec![
            Span::styled("Request ID: ", Style::default().fg(Color::Cyan)),
            Span::raw(&selected.request_id),
        ]));

        // Add some spacing
        lines.push(Line::from(""));

        let content = Text::from(lines);
        let paragraph = Paragraph::new(content)
            .style(Style::default().fg(Color::White).bg(Color::Black))
            .block(
                Block::default()
                    .title(self.title.as_str())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }
}

impl Default for RequestDetails {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the color for a status code.
fn status_color(status: u16) -> Color {
    match status {
        0 => Color::Red,       // Error/failure
        100..=199 => Color::Cyan,
        200..=299 => Color::Green,
        300..=399 => Color::Yellow,
        400..=499 => Color::Red,
        500..=599 => Color::Red,
        _ => Color::White,
    }
}

/// Returns a string representation of the status code.
fn status_str(status: u16) -> String {
    match status {
        0 => "ERROR".to_string(),
        _ => status.to_string(),
    }
}

/// Returns the color for a decision type.
fn decision_color(decision: &mock_http::events::DecisionType) -> Color {
    match decision {
        mock_http::events::DecisionType::Mock => Color::Green,
        mock_http::events::DecisionType::Forward => Color::Blue,
        mock_http::events::DecisionType::Reject => Color::Red,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_details_new() {
        let details = RequestDetails::new();
        assert_eq!(details.title, "Request Details");
    }

    #[test]
    fn test_status_color() {
        assert_eq!(status_color(0), Color::Red);
        assert_eq!(status_color(200), Color::Green);
        assert_eq!(status_color(301), Color::Yellow);
        assert_eq!(status_color(404), Color::Red);
        assert_eq!(status_color(500), Color::Red);
    }

    #[test]
    fn test_status_str() {
        assert_eq!(status_str(0), "ERROR");
        assert_eq!(status_str(200), "200");
    }
}
