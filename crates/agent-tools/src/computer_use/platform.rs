use super::types::{ComputerAction, ComputerResult, CursorPosition, ScreenSize};

/// Platform-specific desktop interaction backend.
///
/// Screenshot and screen-size queries use `xcap` for real system data.
/// Input-control actions (mouse, keyboard) are still simulated — real
/// implementations via `enigo` will land in a follow-up PR.
#[derive(Default)]
pub struct DesktopBackend {
    _private: (),
}

impl DesktopBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn execute(&self, action: ComputerAction) -> Result<ComputerResult, String> {
        match action {
            ComputerAction::Screenshot { region } => self.take_screenshot(region).await,
            ComputerAction::GetScreenSize {} => self.get_screen_size(),
            ComputerAction::GetCursorPosition {} => {
                // Cursor position requires platform-specific APIs beyond xcap.
                // Simulated for now — real implementation comes with input-control PR.
                Ok(ComputerResult {
                    success: true,
                    output: "Cursor position query (simulated)".into(),
                    screenshot: None,
                    screen_size: None,
                    cursor_position: Some(CursorPosition { x: 0, y: 0 }),
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
            // Input-control actions are simulated — real enigo integration in follow-up PR.
            ComputerAction::MouseMove { x, y } => Ok(ComputerResult {
                success: true,
                output: format!("Mouse moved to ({}, {}) (simulated)", x, y),
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
                    output: format!("{} click ({}) at {} (simulated)", btn, count, pos),
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
                    "Dragged from ({}, {}) to ({}, {}) (simulated)",
                    from_x, from_y, to_x, to_y
                ),
                screenshot: None,
                screen_size: None,
                cursor_position: None,
            }),
            ComputerAction::KeyboardType { ref text } => Ok(ComputerResult {
                success: true,
                output: format!("Typed {} characters (simulated)", text.len()),
                screenshot: None,
                screen_size: None,
                cursor_position: None,
            }),
            ComputerAction::KeyPress { ref keys } => Ok(ComputerResult {
                success: true,
                output: format!("Pressed keys: {} (simulated)", keys),
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
                    output: format!(
                        "Scrolled {} by {} at {} (simulated)",
                        direction, amount, pos
                    ),
                    screenshot: None,
                    screen_size: None,
                    cursor_position: None,
                })
            }
        }
    }

    /// Capture a real screenshot using `xcap`.
    async fn take_screenshot(&self, region: Option<[u32; 4]>) -> Result<ComputerResult, String> {
        use xcap::Monitor;

        // Get the primary monitor
        let monitors =
            Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {}", e))?;
        let monitor = monitors
            .into_iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .or_else(|| {
                // Fallback: just take the first monitor
                Monitor::all().ok().and_then(|m| m.into_iter().next())
            })
            .ok_or_else(|| "No monitors found".to_string())?;

        // Capture the screen
        let captured = monitor
            .capture_image()
            .map_err(|e| format!("Screenshot capture failed: {}", e))?;

        // Crop to region if specified
        let final_image: image::RgbaImage = if let Some([x, y, w, h]) = region {
            let img_width = captured.width();
            let img_height = captured.height();
            // Clamp region to image bounds
            let crop_x = x.min(img_width.saturating_sub(1));
            let crop_y = y.min(img_height.saturating_sub(1));
            let crop_w = w.min(img_width.saturating_sub(crop_x));
            let crop_h = h.min(img_height.saturating_sub(crop_y));

            if crop_w == 0 || crop_h == 0 {
                return Err(format!(
                    "Invalid crop region: ({}, {}, {}x{}) on {}x{} image",
                    x, y, w, h, img_width, img_height
                ));
            }

            image::imageops::crop_imm(&captured, crop_x, crop_y, crop_w, crop_h).to_image()
        } else {
            captured
        };

        // Encode to PNG and base64
        let mut png_buffer = std::io::Cursor::new(Vec::new());
        final_image
            .write_to(&mut png_buffer, image::ImageFormat::Png)
            .map_err(|e| format!("Failed to encode PNG: {}", e))?;

        let png_bytes = png_buffer.into_inner();

        let base64_data = base64_encode(&png_bytes);

        let desc = match region {
            Some([x, y, w, h]) => format!("Screenshot of region ({}, {}, {}x{})", x, y, w, h),
            None => "Full screen screenshot captured".into(),
        };

        Ok(ComputerResult {
            success: true,
            output: desc,
            screenshot: Some(base64_data),
            screen_size: None,
            cursor_position: None,
        })
    }

    /// Get real screen dimensions from `xcap`.
    fn get_screen_size(&self) -> Result<ComputerResult, String> {
        use xcap::Monitor;

        let monitors =
            Monitor::all().map_err(|e| format!("Failed to enumerate monitors: {}", e))?;
        let monitor = monitors
            .into_iter()
            .find(|m| m.is_primary().unwrap_or(false))
            .or_else(|| Monitor::all().ok().and_then(|m| m.into_iter().next()))
            .ok_or_else(|| "No monitors found".to_string())?;

        let width = monitor
            .width()
            .map_err(|e| format!("Failed to get screen width: {}", e))?;
        let height = monitor
            .height()
            .map_err(|e| format!("Failed to get screen height: {}", e))?;

        Ok(ComputerResult {
            success: true,
            output: format!("Screen size: {}x{}", width, height),
            screenshot: None,
            screen_size: Some(ScreenSize { width, height }),
            cursor_position: None,
        })
    }
}

/// Simple base64 encoder (no external crate dependency).
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let combined = (b0 << 16) | (b1 << 8) | b2;
        result.push(ALPHABET[((combined >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((combined >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(ALPHABET[((combined >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(combined & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
