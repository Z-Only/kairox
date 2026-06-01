//! Shared types for the browser automation tool.

use serde::{Deserialize, Serialize};

/// Actions the browser tool can perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BrowserAction {
    /// Navigate to a URL.
    Navigate { url: String },
    /// Click an element by CSS selector or ref.
    Click { selector: String },
    /// Type text into a focused or selected element.
    Type { selector: String, text: String },
    /// Scroll the page (direction: up/down/left/right, amount in pixels).
    Scroll {
        direction: String,
        amount: Option<u32>,
    },
    /// Hover over an element.
    Hover { selector: String },
    /// Take a screenshot (returns base64 PNG).
    Screenshot { full_page: Option<bool> },
    /// Get the visible text content of the page or an element.
    GetText { selector: Option<String> },
    /// Wait for a selector to appear or a timeout.
    Wait {
        selector: Option<String>,
        timeout_ms: Option<u64>,
    },
    /// Fill a form field.
    FormFill { selector: String, value: String },
    /// Get page title and URL.
    GetState {},
    /// Close the browser.
    Close {},
}

/// Result of a browser action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserResult {
    pub success: bool,
    pub output: String,
    /// Base64-encoded screenshot if one was taken.
    pub screenshot: Option<String>,
    /// Current page URL after the action.
    pub current_url: Option<String>,
    /// Current page title.
    pub title: Option<String>,
}

/// State of the managed browser instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserState {
    NotStarted,
    Running,
    Closed,
    Error(String),
}
