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

        // Request headers
        lines.push(Line::from(vec![
            Span::styled("Headers:", Style::default().fg(Color::Cyan)),
        ]));
        for (key, value) in &selected.request_headers {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{}: ", key), Style::default().fg(Color::Magenta)),
                Span::raw(truncate_string(value, 60)),
            ]));
        }

        // Request body
        lines.push(Line::from(vec![
            Span::styled("Body:", Style::default().fg(Color::Cyan)),
        ]));
        let body_str = decode_body(&selected.request_body);
        for line in body_str.lines().take(20) {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::raw(truncate_string(line, 80)),
            ]));
        }
        if body_str.lines().count() > 20 {
            lines.push(Line::from(vec![
                Span::styled("  ...", Style::default().fg(Color::Gray)),
            ]));
        }

        // Add some spacing
        lines.push(Line::from(""));

        // Response section header
        lines.push(Line::from(vec![
            Span::styled("─── Response ───", Style::default().fg(Color::Yellow)),
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

        // Response headers
        lines.push(Line::from(vec![
            Span::styled("Headers:", Style::default().fg(Color::Cyan)),
        ]));
        if selected.response_headers.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  -"),
            ]));
        } else {
            for (key, value) in &selected.response_headers {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{}: ", key), Style::default().fg(Color::Magenta)),
                    Span::raw(truncate_string(value, 60)),
                ]));
            }
        }

        // Response body
        lines.push(Line::from(vec![
            Span::styled("Body:", Style::default().fg(Color::Cyan)),
        ]));
        let resp_body_str = decode_body(&selected.response_body);
        if resp_body_str.is_empty() {
            lines.push(Line::from(vec![
                Span::raw("  -"),
            ]));
        } else {
            for line in resp_body_str.lines().take(20) {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::raw(truncate_string(line, 80)),
                ]));
            }
            if resp_body_str.lines().count() > 20 {
                lines.push(Line::from(vec![
                    Span::styled("  ...", Style::default().fg(Color::Gray)),
                ]));
            }
        }

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

/// Decodes body bytes to a string for display.
fn decode_body(body: &[u8]) -> String {
    if body.is_empty() {
        return String::new();
    }

    // First, try to parse as JSON and pretty-print
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(body) {
        if let Ok(formatted) = serde_json::to_string_pretty(&json) {
            return formatted;
        }
    }

    // Try as UTF-8 string
    String::from_utf8_lossy(body).to_string()
}

/// Truncates a string to max_len, adding "..." if truncated.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
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

    #[test]
    fn test_truncate_string_short() {
        assert_eq!(truncate_string("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_string_long() {
        let result = truncate_string("this is a very long string", 10);
        assert!(result.len() <= 10);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_decode_body_empty() {
        assert!(decode_body(&[]).is_empty());
    }

    #[test]
    fn test_decode_body_json() {
        let body = br#"{"key":"value"}"#;
        let result = decode_body(body);
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    #[test]
    fn test_decode_body_text() {
        let body = b"plain text";
        assert_eq!(decode_body(body), "plain text");
    }
}
