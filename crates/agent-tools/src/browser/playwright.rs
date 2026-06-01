//! Playwright process management.
//!
//! Manages a Playwright browser instance via a Node.js helper script.
//! Currently provides a simulated backend; the real Node.js bridge will be
//! added in a follow-up.

use std::path::PathBuf;

use tokio::process::Child;
use tokio::sync::Mutex;

use super::types::{BrowserAction, BrowserResult, BrowserState};

/// Manages a Playwright browser instance via a Node.js helper script.
pub struct PlaywrightManager {
    state: Mutex<BrowserState>,
    process: Mutex<Option<Child>>,
    #[allow(dead_code)]
    workspace_root: PathBuf,
}

impl PlaywrightManager {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            state: Mutex::new(BrowserState::NotStarted),
            process: Mutex::new(None),
            workspace_root,
        }
    }

    /// Ensure browser is running. Launches if not started.
    pub async fn ensure_running(&self) -> Result<(), String> {
        let mut state = self.state.lock().await;
        match &*state {
            BrowserState::Running => Ok(()),
            BrowserState::NotStarted | BrowserState::Closed | BrowserState::Error(_) => {
                // Launch playwright via npx or bundled script.
                // For now, simulate — real impl would spawn a node process.
                *state = BrowserState::Running;
                Ok(())
            }
        }
    }

    /// Execute a browser action.
    pub async fn execute(&self, action: BrowserAction) -> Result<BrowserResult, String> {
        self.ensure_running().await?;

        // In the real implementation, this sends JSON-RPC to the Node process.
        // For now, provide a structured response based on action type.
        match &action {
            BrowserAction::Navigate { url } => Ok(BrowserResult {
                success: true,
                output: format!("Navigated to {}", url),
                screenshot: None,
                current_url: Some(url.clone()),
                title: Some("Page".into()),
            }),
            BrowserAction::Click { selector } => Ok(BrowserResult {
                success: true,
                output: format!("Clicked element: {}", selector),
                screenshot: None,
                current_url: None,
                title: None,
            }),
            BrowserAction::Type { selector, text } => Ok(BrowserResult {
                success: true,
                output: format!("Typed {:?} into {}", text, selector),
                screenshot: None,
                current_url: None,
                title: None,
            }),
            BrowserAction::Screenshot { .. } => Ok(BrowserResult {
                success: true,
                output: "Screenshot captured".into(),
                screenshot: Some("[base64-placeholder]".into()),
                current_url: None,
                title: None,
            }),
            BrowserAction::GetText { selector } => Ok(BrowserResult {
                success: true,
                output: format!("Text content of {}", selector.as_deref().unwrap_or("page")),
                screenshot: None,
                current_url: None,
                title: None,
            }),
            BrowserAction::GetState {} => Ok(BrowserResult {
                success: true,
                output: "Browser state retrieved".into(),
                screenshot: None,
                current_url: Some("about:blank".into()),
                title: Some(String::new()),
            }),
            BrowserAction::Close {} => {
                let mut state = self.state.lock().await;
                *state = BrowserState::Closed;
                Ok(BrowserResult {
                    success: true,
                    output: "Browser closed".into(),
                    screenshot: None,
                    current_url: None,
                    title: None,
                })
            }
            BrowserAction::Scroll { direction, amount } => Ok(BrowserResult {
                success: true,
                output: format!("Scrolled {} by {} pixels", direction, amount.unwrap_or(300)),
                screenshot: None,
                current_url: None,
                title: None,
            }),
            BrowserAction::Hover { selector } => Ok(BrowserResult {
                success: true,
                output: format!("Hovered over: {}", selector),
                screenshot: None,
                current_url: None,
                title: None,
            }),
            BrowserAction::Wait {
                selector,
                timeout_ms,
            } => Ok(BrowserResult {
                success: true,
                output: format!(
                    "Waited for {} (timeout: {}ms)",
                    selector.as_deref().unwrap_or("page"),
                    timeout_ms.unwrap_or(5000)
                ),
                screenshot: None,
                current_url: None,
                title: None,
            }),
            BrowserAction::FormFill { selector, value } => Ok(BrowserResult {
                success: true,
                output: format!("Filled {} with {:?}", selector, value),
                screenshot: None,
                current_url: None,
                title: None,
            }),
        }
    }

    /// Shut down the browser process.
    pub async fn shutdown(&self) {
        let mut proc = self.process.lock().await;
        if let Some(ref mut child) = *proc {
            let _ = child.kill().await;
        }
        *proc = None;
        let mut state = self.state.lock().await;
        *state = BrowserState::Closed;
    }
}
