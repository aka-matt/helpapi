//! Request list widget showing a table of recent requests.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Row, Table},
};

use crate::tui::state::AppState;

/// Maximum width for the path column.
const PATH_WIDTH: u16 = 40;

/// Request list widget.
#[derive(Debug)]
pub struct RequestList {
    /// Optional title.
    title: String,
}

impl RequestList {
    /// Creates a new RequestList widget.
    pub fn new() -> Self {
        Self {
            title: "Requests".to_string(),
        }
    }

    /// Renders the request list table.
    pub fn render(&self, frame: &mut Frame<'_>, area: Rect, state: &AppState) {
        let requests = state.filtered_requests();

        let header_style = Style::default().fg(Color::Yellow).bg(Color::Black);
        let selected_style = Style::default().fg(Color::Black).bg(Color::Blue);
        let normal_style = Style::default().fg(Color::White).bg(Color::Black);

        // Build table rows
        let rows: Vec<Row<'_>> = requests
            .iter()
            .enumerate()
            .map(|(idx, record)| {
                let style = if state.selected_request == Some(idx) {
                    selected_style
                } else {
                    normal_style
                };
                Row::new(vec![
                    Line::raw(record.method.clone()),
                    Line::raw(truncate_path(&record.path)),
                    Line::raw(status_str(record.status)),
                    Line::raw(format!("{}ms", record.latency_ms)),
                    Line::raw(record.rule_id.as_deref().unwrap_or("-").to_string()),
                ])
                .style(style)
            })
            .collect();

        let header = Row::new(vec![
            Line::raw("METHOD"),
            Line::raw("PATH"),
            Line::raw("STATUS"),
            Line::raw("LATENCY"),
            Line::raw("RULE"),
        ])
        .style(header_style);

        let widths = [
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(PATH_WIDTH),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(12),
            ratatui::layout::Constraint::Length(16),
        ];

        let table = Table::new(rows, &widths)
            .header(header)
            .block(
                Block::default()
                    .title(self.title.as_str())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White)),
            )
            .column_spacing(1);

        frame.render_widget(table, area);
    }
}

impl Default for RequestList {
    fn default() -> Self {
        Self::new()
    }
}

/// Truncates the path to fit within PATH_WIDTH.
fn truncate_path(path: &str) -> String {
    if path.len() > PATH_WIDTH as usize {
        let mut result = path[..PATH_WIDTH as usize - 3].to_string();
        result.push_str("...");
        result
    } else {
        path.to_string()
    }
}

/// Returns a string representation of the status code.
fn status_str(status: u16) -> String {
    match status {
        0 => "-".to_string(),
        _ => status.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_list_new() {
        let list = RequestList::new();
        assert_eq!(list.title, "Requests");
    }

    #[test]
    fn test_truncate_path_short() {
        let path = "/short";
        assert_eq!(truncate_path(path), "/short");
    }

    #[test]
    fn test_truncate_path_long() {
        let path = "/this/is/a/very/long/path/that/exceeds/the/maximum/width";
        let result = truncate_path(path);
        assert!(result.len() <= PATH_WIDTH as usize);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_status_str_zero() {
        assert_eq!(status_str(0), "-");
    }

    #[test]
    fn test_status_str_normal() {
        assert_eq!(status_str(200), "200");
    }
}
