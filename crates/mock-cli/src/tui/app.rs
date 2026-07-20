//! Main TUI application.

use std::sync::Arc;

use crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use mock_runtime::Runtime;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::RwLock;

/// Interval for polling runtime events (in milliseconds).
const EVENT_POLL_INTERVAL_MS: u64 = 100;

/// TUI application runner.
pub struct App {
    /// Runtime reference for reload operations.
    runtime: Arc<RwLock<Runtime>>,
    /// Path to the config file for reloads.
    config_path: std::path::PathBuf,
}

impl App {
    /// Creates a new App.
    pub fn new(runtime: Arc<RwLock<Runtime>>, config_path: std::path::PathBuf) -> Self {
        Self {
            runtime,
            config_path,
        }
    }

    /// Runs the TUI application.
    pub async fn run(&self) -> anyhow::Result<()> {
        static PANIC_HOOK_INSTALLED: std::sync::Once = std::sync::Once::new();
        PANIC_HOOK_INSTALLED.call_once(|| {
            let original = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = crossterm::execute!(std::io::stderr(), DisableBracketedPaste, LeaveAlternateScreen);
                original(info);
            }));
        });

        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(std::io::stderr(), EnterAlternateScreen)?;
        crossterm::execute!(std::io::stderr(), EnableBracketedPaste)?;

        let backend = CrosstermBackend::new(std::io::stderr());
        let mut terminal = Terminal::new(backend)?;

        let mut state = crate::tui::state::AppState::new();

        let mut event_receiver = {
            let runtime = self.runtime.read().await;
            state.runtime_status = runtime.status();
            runtime.subscribe()
        };

        let res = self.run_loop(&mut terminal, &mut state, &mut event_receiver).await;

        let _ = crossterm::execute!(std::io::stderr(), DisableBracketedPaste);
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stderr(), LeaveAlternateScreen);

        res
    }

    /// Main render loop.
    async fn run_loop(
        &self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stderr>>,
        state: &mut crate::tui::state::AppState,
        event_receiver: &mut mock_runtime::EventReceiver,
    ) -> anyhow::Result<()> {
        loop {
            if state.should_quit {
                break;
            }

            // Poll for runtime events
            let (events, lag) = event_receiver
                .drain_all_until_empty(std::time::Duration::from_millis(EVENT_POLL_INTERVAL_MS))
                .await;
            state.lag_count = state.lag_count.saturating_add(lag);
            for event in events {
                state.handle_event(&event);
            }

            // Render the UI
            terminal.draw(|frame| {
                self.render_frame(frame, state);
            })?;

            // Handle keyboard input (non-blocking)
            if let Ok(true) = crossterm::event::poll(std::time::Duration::from_millis(10)) {
                if let Event::Key(key) = event::read()? {
                    // Only handle key press events, not release
                    if key.kind == KeyEventKind::Press {
                        self.handle_key_event(key.code, state);
                    }
                }
            }
        }

        Ok(())
    }

    /// Renders a single frame.
    fn render_frame(&self, frame: &mut ratatui::Frame<'_>, state: &crate::tui::state::AppState) {
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::style::Color;
        use ratatui::style::Style;
        use ratatui::text::{Line, Span};

        let size = frame.area();

        // Create the main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Status bar
                Constraint::Min(0),    // Main content
                Constraint::Length(1), // Help bar (toggled)
            ])
            .split(size);

        // Render status bar
        let status_bar = crate::tui::widgets::StatusBar::new();
        status_bar.render(frame, chunks[0], state);

        // Render main content based on active panel
        match state.active_panel {
            crate::tui::widgets::Panel::RequestList => {
                let request_list = crate::tui::widgets::RequestList::new();
                request_list.render(frame, chunks[1], state);
            }
            crate::tui::widgets::Panel::Details => {
                let request_details = crate::tui::widgets::RequestDetails::new();
                request_details.render(frame, chunks[1], state);
            }
            crate::tui::widgets::Panel::Help => {
                let help_bar = crate::tui::widgets::HelpBar::new();
                help_bar.render(frame, chunks[1], state);
            }
        }

        // Render help bar at the bottom if not viewing the help panel
        if state.active_panel != crate::tui::widgets::Panel::Help {
            let help_text = Line::from(vec![
                Span::raw(" "),
                Span::styled("q", Style::default().fg(Color::Yellow)),
                Span::raw(" quit "),
                Span::styled("r", Style::default().fg(Color::Yellow)),
                Span::raw(" reload "),
                Span::styled("c", Style::default().fg(Color::Yellow)),
                Span::raw(" clear "),
                Span::styled("j/k", Style::default().fg(Color::Yellow)),
                Span::raw(" navigate "),
                Span::styled("/", Style::default().fg(Color::Yellow)),
                Span::raw(" filter "),
                Span::styled("Tab", Style::default().fg(Color::Yellow)),
                Span::raw(" switch "),
                Span::styled("?", Style::default().fg(Color::Yellow)),
                Span::raw(" help"),
            ]);
            let help_widget = ratatui::widgets::Paragraph::new(help_text)
                .style(Style::default().bg(Color::DarkGray));
            frame.render_widget(help_widget, chunks[2]);
        }
    }

    /// Handles a key event.
    fn handle_key_event(&self, key_code: KeyCode, state: &mut crate::tui::state::AppState) {
        // Handle filter input mode separately
        if state.filter_input_mode {
            self.handle_filter_input(key_code, state);
            return;
        }

        match key_code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                state.should_quit = true;
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                // Trigger config reload
                let runtime = self.runtime.clone();
                let config_path = self.config_path.clone();
                tokio::spawn(async move {
                    if let Ok(content) = std::fs::read_to_string(&config_path) {
                        let mut runtime = runtime.write().await;
                        if let Err(e) = runtime.reload(&content).await {
                            tracing::error!("reload failed: {}", e);
                        }
                    }
                });
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                state.clear_history();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                state.select_next();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                state.select_previous();
            }
            KeyCode::Char('/') => {
                // Enter filter input mode
                state.filter_input_mode = true;
                state.filter.clear();
            }
            KeyCode::Tab => {
                // Cycle through panels
                state.active_panel = match state.active_panel {
                    crate::tui::widgets::Panel::RequestList => crate::tui::widgets::Panel::Details,
                    crate::tui::widgets::Panel::Details => crate::tui::widgets::Panel::RequestList,
                    crate::tui::widgets::Panel::Help => crate::tui::widgets::Panel::RequestList,
                };
            }
            KeyCode::Enter => {
                // Switch to details panel when Enter is pressed on a request
                if state.selected_request.is_some() {
                    state.active_panel = crate::tui::widgets::Panel::Details;
                }
            }
            KeyCode::Char('?') => {
                // Toggle help panel
                if state.active_panel == crate::tui::widgets::Panel::Help {
                    state.active_panel = crate::tui::widgets::Panel::RequestList;
                } else {
                    state.active_panel = crate::tui::widgets::Panel::Help;
                }
            }
            _ => {}
        }
    }

    /// Handles key events in filter input mode.
    fn handle_filter_input(&self, key_code: KeyCode, state: &mut crate::tui::state::AppState) {
        match key_code {
            KeyCode::Enter => {
                // Confirm filter and exit filter mode
                state.filter_input_mode = false;
            }
            KeyCode::Esc => {
                // Cancel filter input and exit filter mode
                state.filter.clear();
                state.filter_input_mode = false;
            }
            KeyCode::Backspace => {
                // Remove last character from filter
                state.filter.pop();
            }
            KeyCode::Char(c) => {
                // Add character to filter
                state.filter.push(c);
            }
            _ => {
                // Any other key exits filter mode
                state.filter_input_mode = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_poll_interval() {
        assert_eq!(EVENT_POLL_INTERVAL_MS, 100);
    }
}
