//! Application state for the TUI.

use std::collections::VecDeque;

use mock_http::events::{DecisionType, RuntimeEvent};

use crate::tui::widgets::Panel;

/// Maximum number of request records to keep in history.
pub const MAX_REQUESTS: usize = 1000;

/// A single request record displayed in the request list.
#[derive(Debug, Clone)]
pub struct RequestRecord {
    /// HTTP method.
    pub method: String,
    /// Request path.
    pub path: String,
    /// Request headers (lowercase keys).
    pub request_headers: Vec<(String, String)>,
    /// Request body (raw bytes).
    pub request_body: Vec<u8>,
    /// Response status code.
    pub status: u16,
    /// Response headers.
    pub response_headers: Vec<(String, String)>,
    /// Response body (raw bytes).
    pub response_body: Vec<u8>,
    /// Latency in milliseconds.
    pub latency_ms: u64,
    /// ID of the matched rule, if any.
    pub rule_id: Option<String>,
    /// Type of decision that was made.
    pub decision_type: DecisionType,
    /// Unique request identifier.
    pub request_id: String,
}

impl RequestRecord {
    /// Creates a new RequestRecord from a request started event.
    pub fn from_started(request_id: &str, summary: &mock_http::events::RequestSummary) -> Self {
        Self {
            method: summary.method.clone(),
            path: summary.path.clone(),
            request_headers: summary.headers.clone(),
            request_body: summary.body.clone(),
            status: 0, // Will be updated on completion
            response_headers: Vec::new(),
            response_body: Vec::new(),
            latency_ms: 0,
            rule_id: summary.rule_id.clone(),
            decision_type: DecisionType::Mock, // Default until updated
            request_id: request_id.to_string(),
        }
    }

    /// Updates the record from a completed request event.
    pub fn update_from_completed(&mut self, result: &mock_http::events::RequestResult) {
        self.status = result.status;
        self.latency_ms = result.elapsed_ms;
        self.response_headers = result.headers.clone();
        self.response_body = result.body.clone();
        self.decision_type = result.decision_type;
    }
}

/// The main application state for the TUI.
#[derive(Debug)]
pub struct AppState {
    /// Current runtime status.
    pub runtime_status: mock_runtime::RuntimeStatus,
    /// Server address when running.
    pub server_address: Option<String>,
    /// Number of rules in the current configuration.
    pub rule_count: usize,
    /// Number of runtime events skipped due to broadcast lag. Surfaced in the TUI.
    pub lag_count: u64,
    /// Request history.
    pub requests: VecDeque<RequestRecord>,
    /// Index of the currently selected request.
    pub selected_request: Option<usize>,
    /// The active panel.
    pub active_panel: Panel,
    /// Filter string for the request list.
    pub filter: String,
    /// Whether filter input mode is active.
    pub filter_input_mode: bool,
    /// Configuration error message, if any.
    pub config_error: Option<String>,
    /// Whether the TUI should quit.
    pub should_quit: bool,
}

impl AppState {
    /// Creates a new AppState.
    pub fn new() -> Self {
        Self {
            runtime_status: mock_runtime::RuntimeStatus::Stopped,
            server_address: None,
            rule_count: 0,
            lag_count: 0,
            requests: VecDeque::with_capacity(MAX_REQUESTS),
            selected_request: None,
            active_panel: Panel::RequestList,
            filter: String::new(),
            filter_input_mode: false,
            config_error: None,
            should_quit: false,
        }
    }

    /// Updates the state with a runtime event.
    pub fn handle_event(&mut self, event: &RuntimeEvent) {
        match event {
            RuntimeEvent::ServerStarted { address } => {
                self.server_address = Some(address.clone());
                self.runtime_status = mock_runtime::RuntimeStatus::Running;
            }
            RuntimeEvent::ServerStopped => {
                self.runtime_status = mock_runtime::RuntimeStatus::Stopped;
                self.server_address = None;
            }
            RuntimeEvent::ConfigReloaded { rule_count } => {
                self.rule_count = *rule_count;
                self.config_error = None;
            }
            RuntimeEvent::ConfigReloadFailed { message } => {
                self.config_error = Some(message.clone());
            }
            RuntimeEvent::RequestStarted {
                request_id,
                summary,
            } => {
                // Request started events are informational; we wait for completion
                let record = RequestRecord::from_started(request_id, summary);
                if self.requests.len() >= MAX_REQUESTS {
                    self.requests.pop_front();
                }
                self.requests.push_back(record);
            }
            RuntimeEvent::RequestCompleted { request_id, result } => {
                // Find and update the matching request record
                for record in self.requests.iter_mut() {
                    if record.request_id == *request_id {
                        record.update_from_completed(result);
                        break;
                    }
                }
            }
            RuntimeEvent::RequestFailed {
                request_id,
                message: _,
            } => {
                // Find and update the matching request record with an error status
                for record in self.requests.iter_mut() {
                    if record.request_id == *request_id {
                        record.status = 0; // Indicates failure
                        break;
                    }
                }
            }
        }
    }

    /// Clears the request history.
    pub fn clear_history(&mut self) {
        self.requests.clear();
        self.selected_request = None;
    }

    /// Moves the selection up in the request list.
    pub fn select_previous(&mut self) {
        let len = self.filtered_requests().len();
        if len == 0 {
            self.selected_request = None;
            return;
        }
        match self.selected_request {
            None => self.selected_request = Some(len - 1),
            Some(i) => {
                if i > 0 {
                    self.selected_request = Some(i - 1);
                }
            }
        }
    }

    /// Moves the selection down in the request list.
    pub fn select_next(&mut self) {
        let len = self.filtered_requests().len();
        if len == 0 {
            // Don't set selection on empty list
            return;
        }
        match self.selected_request {
            None => self.selected_request = Some(0),
            Some(i) => {
                if i < len - 1 {
                    self.selected_request = Some(i + 1);
                }
            }
        }
    }

    /// Returns the filtered list of requests.
    pub fn filtered_requests(&self) -> Vec<&RequestRecord> {
        if self.filter.is_empty() {
            return self.requests.iter().collect();
        }
        let filter_lower = self.filter.to_lowercase();
        self.requests
            .iter()
            .filter(|r| {
                r.path.to_lowercase().contains(&filter_lower)
                    || r.method.to_lowercase().contains(&filter_lower)
                    || r.status.to_string().contains(&filter_lower)
                    || r.rule_id
                        .as_ref()
                        .map(|id| id.to_lowercase().contains(&filter_lower))
                        .unwrap_or(false)
            })
            .collect()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_new() {
        let state = AppState::new();
        assert_eq!(state.runtime_status, mock_runtime::RuntimeStatus::Stopped);
        assert!(state.server_address.is_none());
        assert_eq!(state.rule_count, 0);
        assert_eq!(state.lag_count, 0);
        assert!(state.requests.is_empty());
        assert!(state.selected_request.is_none());
        assert_eq!(state.active_panel, Panel::RequestList);
        assert!(state.filter.is_empty());
        assert!(!state.filter_input_mode);
        assert!(state.config_error.is_none());
        assert!(!state.should_quit);
    }

    #[test]
    fn test_select_previous_empty() {
        let mut state = AppState::new();
        state.select_previous();
        assert!(state.selected_request.is_none());
    }

    #[test]
    fn test_select_next_empty() {
        let mut state = AppState::new();
        state.select_next();
        // On empty list, selection should remain None
        assert!(state.selected_request.is_none());
    }

    #[test]
    fn test_select_previous_wraps() {
        let mut state = AppState::new();
        state.requests.push_back(RequestRecord {
            method: "GET".to_string(),
            path: "/test".to_string(),
            request_headers: Vec::new(),
            request_body: Vec::new(),
            status: 200,
            response_headers: Vec::new(),
            response_body: Vec::new(),
            latency_ms: 10,
            rule_id: None,
            decision_type: DecisionType::Mock,
            request_id: "1".to_string(),
        });
        state.selected_request = Some(0);
        state.select_previous();
        assert_eq!(state.selected_request, Some(0));
    }

    #[test]
    fn test_clear_history() {
        let mut state = AppState::new();
        state.requests.push_back(RequestRecord {
            method: "GET".to_string(),
            path: "/test".to_string(),
            request_headers: Vec::new(),
            request_body: Vec::new(),
            status: 200,
            response_headers: Vec::new(),
            response_body: Vec::new(),
            latency_ms: 10,
            rule_id: None,
            decision_type: DecisionType::Mock,
            request_id: "1".to_string(),
        });
        state.selected_request = Some(0);
        state.clear_history();
        assert!(state.requests.is_empty());
        assert!(state.selected_request.is_none());
    }

    #[test]
    fn test_filtered_requests_empty_filter() {
        let mut state = AppState::new();
        state.requests.push_back(RequestRecord {
            method: "GET".to_string(),
            path: "/test".to_string(),
            request_headers: Vec::new(),
            request_body: Vec::new(),
            status: 200,
            response_headers: Vec::new(),
            response_body: Vec::new(),
            latency_ms: 10,
            rule_id: None,
            decision_type: DecisionType::Mock,
            request_id: "1".to_string(),
        });
        let filtered = state.filtered_requests();
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_filtered_requests_with_filter() {
        let mut state = AppState::new();
        state.requests.push_back(RequestRecord {
            method: "GET".to_string(),
            path: "/test".to_string(),
            request_headers: Vec::new(),
            request_body: Vec::new(),
            status: 200,
            response_headers: Vec::new(),
            response_body: Vec::new(),
            latency_ms: 10,
            rule_id: None,
            decision_type: DecisionType::Mock,
            request_id: "1".to_string(),
        });
        state.requests.push_back(RequestRecord {
            method: "POST".to_string(),
            path: "/other".to_string(),
            request_headers: Vec::new(),
            request_body: Vec::new(),
            status: 201,
            response_headers: Vec::new(),
            response_body: Vec::new(),
            latency_ms: 20,
            rule_id: None,
            decision_type: DecisionType::Mock,
            request_id: "2".to_string(),
        });
        state.filter = "get".to_string();
        let filtered = state.filtered_requests();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].path, "/test");
    }
}
