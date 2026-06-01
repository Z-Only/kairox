use super::types::{ComputerAction, ComputerResult, CursorPosition, ScreenSize};

/// Platform-specific desktop interaction backend.
/// Currently simulated — real implementations will use:
/// - macOS: CoreGraphics/ApplicationServices
/// - Linux: X11/XDG or Wayland
pub struct DesktopBackend {
    screen_width: u32,
    screen_height: u32,
}

impl Default for DesktopBackend {
    fn default() -> Self {
        Self {
            screen_width: 1920,
            screen_height: 1080,
        }
    }
}

impl DesktopBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn execute(&self, action: ComputerAction) -> Result<ComputerResult, String> {
        match action {
            ComputerAction::Screenshot { region } => {
                let desc = match region {
                    Some([x, y, w, h]) => {
                        format!("Screenshot of region ({}, {}, {}x{})", x, y, w, h)
                    }
                    None => "Full screen screenshot captured".into(),
                };
                Ok(ComputerResult {
                    success: true,
                    output: desc,
                    screenshot: Some("[base64-screenshot-placeholder]".into()),
                    screen_size: None,
                    cursor_position: None,
                })
            }
            ComputerAction::MouseMove { x, y } => Ok(ComputerResult {
                success: true,
                output: format!("Mouse moved to ({}, {})", x, y),
                screenshot: None,
                screen_size: None,
                cursor_position: None,
            }),
            ComputerAction::MouseClick {
                x,
                y,
                button,
                click_count,
            } => {
                let btn = button.unwrap_or_else(|| "left".into());
                let count = click_count.unwrap_or(1);
                let pos = match (x, y) {
                    (Some(x), Some(y)) => format!("({}, {})", x, y),
                    _ => "current position".into(),
                };
                Ok(ComputerResult {
                    success: true,
                    output: format!("{} click ({}) at {}", btn, count, pos),
                    screenshot: None,
                    screen_size: None,
                    cursor_position: None,
                })
            }
            ComputerAction::MouseDrag {
                from_x,
                from_y,
                to_x,
                to_y,
            } => Ok(ComputerResult {
                success: true,
                output: format!(
                    "Dragged from ({}, {}) to ({}, {})",
                    from_x, from_y, to_x, to_y
                ),
                screenshot: None,
                screen_size: None,
                cursor_position: None,
            }),
            ComputerAction::KeyboardType { ref text } => Ok(ComputerResult {
                success: true,
                output: format!("Typed {} characters", text.len()),
                screenshot: None,
                screen_size: None,
                cursor_position: None,
            }),
            ComputerAction::KeyPress { ref keys } => Ok(ComputerResult {
                success: true,
                output: format!("Pressed keys: {}", keys),
                screenshot: None,
                screen_size: None,
                cursor_position: None,
            }),
            ComputerAction::Scroll {
                x,
                y,
                ref direction,
                amount,
            } => {
                let pos = match (x, y) {
                    (Some(x), Some(y)) => format!("({}, {})", x, y),
                    _ => "current position".into(),
                };
                Ok(ComputerResult {
                    success: true,
                    output: format!("Scrolled {} by {} at {}", direction, amount, pos),
                    screenshot: None,
                    screen_size: None,
                    cursor_position: None,
                })
            }
            ComputerAction::Wait { duration_ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(duration_ms.min(5000))).await;
                Ok(ComputerResult {
                    success: true,
                    output: format!("Waited {}ms", duration_ms),
                    screenshot: None,
                    screen_size: None,
                    cursor_position: None,
                })
            }
            ComputerAction::GetScreenSize {} => Ok(ComputerResult {
                success: true,
                output: format!("Screen size: {}x{}", self.screen_width, self.screen_height),
                screenshot: None,
                screen_size: Some(ScreenSize {
                    width: self.screen_width,
                    height: self.screen_height,
                }),
                cursor_position: None,
            }),
            ComputerAction::GetCursorPosition {} => Ok(ComputerResult {
                success: true,
                output: "Cursor at (960, 540)".into(),
                screenshot: None,
                screen_size: None,
                cursor_position: Some(CursorPosition { x: 960, y: 540 }),
            }),
        }
    }
}
