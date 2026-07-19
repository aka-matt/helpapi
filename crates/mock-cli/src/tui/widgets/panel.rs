//! UI panel types.

/// Represents the active panel in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// Request list panel.
    RequestList,
    /// Request details panel.
    Details,
    /// Help panel.
    Help,
}

impl Default for Panel {
    fn default() -> Self {
        Panel::RequestList
    }
}
