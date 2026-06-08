use super::types::{ComputerAction, ComputerResult, CursorPosition, ScreenSize};
use enigo::{Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

/// Platform-specific desktop interaction backend.
///
/// Screenshot and screen-size queries use `xcap` for real system data.
/// Input-control actions (mouse, keyboard, scroll) use `enigo` for real
/// system events.
#[derive(Default)]
pub struct DesktopBackend {
    _private: (),
}

impl DesktopBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a fresh `Enigo` instance.
    /// `Enigo` is not `Send`/`Sync`, so we construct it per-call.
    fn create_enigo() -> Result<Enigo, String> {
        Enigo::new(&Settings::default())
            .map_err(|e| format!("Failed to initialize input controller: {}", e))
    }

    pub async fn execute(&self, action: ComputerAction) -> Result<ComputerResult, String> {
        match action {
            ComputerAction::Screenshot { region } => self.take_screenshot(region).await,
            ComputerAction::GetScreenSize {} => self.get_screen_size(),
            ComputerAction::GetCursorPosition {} => {
                let enigo = Self::create_enigo()?;
                let (x, y) = enigo
                    .location()
                    .map_err(|e| format!("Failed to get cursor position: {}", e))?;
                let cursor_x = x.max(0) as u32;
                let cursor_y = y.max(0) as u32;
                Ok(ComputerResult {
                    success: true,
                    output: format!("Cursor at ({}, {})", cursor_x, cursor_y),
                    screenshot: None,
                    screen_size: None,
                    cursor_position: Some(CursorPosition {
                        x: cursor_x,
                        y: cursor_y,
                    }),
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
            ComputerAction::MouseMove { x, y } => {
                let mut enigo = Self::create_enigo()?;
                enigo
                    .move_mouse(x as i32, y as i32, Coordinate::Abs)
                    .map_err(|e| format!("Mouse move failed: {}", e))?;
                Ok(ComputerResult {
                    success: true,
                    output: format!("Mouse moved to ({}, {})", x, y),
                    screenshot: None,
                    screen_size: None,
                    cursor_position: None,
                })
            }
            ComputerAction::MouseClick {
                x,
                y,
                button,
                click_count,
            } => {
                let mut enigo = Self::create_enigo()?;
                if let (Some(cx), Some(cy)) = (x, y) {
                    enigo
                        .move_mouse(cx as i32, cy as i32, Coordinate::Abs)
                        .map_err(|e| format!("Mouse move failed: {}", e))?;
                }
                let btn = parse_mouse_button(button.as_deref());
                let count = click_count.unwrap_or(1);
                for _ in 0..count {
                    enigo
                        .button(btn, Direction::Click)
                        .map_err(|e| format!("Mouse click failed: {}", e))?;
                }
                let btn_name = button.as_deref().unwrap_or("left");
                let pos = match (x, y) {
                    (Some(px), Some(py)) => format!("({}, {})", px, py),
                    _ => "current position".into(),
                };
                Ok(ComputerResult {
                    success: true,
                    output: format!("{} click ({}) at {}", btn_name, count, pos),
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
            } => {
                let mut enigo = Self::create_enigo()?;
                enigo
                    .move_mouse(from_x as i32, from_y as i32, Coordinate::Abs)
                    .map_err(|e| format!("Drag start move failed: {}", e))?;
                enigo
                    .button(Button::Left, Direction::Press)
                    .map_err(|e| format!("Drag press failed: {}", e))?;
                // Small delay to let the press register before moving.
                // Using std::thread::sleep because Enigo is not Send across .await.
                std::thread::sleep(std::time::Duration::from_millis(50));
                enigo
                    .move_mouse(to_x as i32, to_y as i32, Coordinate::Abs)
                    .map_err(|e| format!("Drag move failed: {}", e))?;
                enigo
                    .button(Button::Left, Direction::Release)
                    .map_err(|e| format!("Drag release failed: {}", e))?;
                Ok(ComputerResult {
                    success: true,
                    output: format!(
                        "Dragged from ({}, {}) to ({}, {})",
                        from_x, from_y, to_x, to_y
                    ),
                    screenshot: None,
                    screen_size: None,
                    cursor_position: None,
                })
            }
            ComputerAction::KeyboardType { ref text } => {
                let mut enigo = Self::create_enigo()?;
                enigo
                    .text(text)
                    .map_err(|e| format!("Keyboard type failed: {}", e))?;
                Ok(ComputerResult {
                    success: true,
                    output: format!("Typed {} characters", text.len()),
                    screenshot: None,
                    screen_size: None,
                    cursor_position: None,
                })
            }
            ComputerAction::KeyPress { ref keys } => {
                let mut enigo = Self::create_enigo()?;
                let (modifiers, primary) = parse_key_combination(keys)?;
                // Hold modifiers
                for modifier in &modifiers {
                    enigo
                        .key(*modifier, Direction::Press)
                        .map_err(|e| format!("Modifier press failed: {}", e))?;
                }
                // Press primary key
                enigo
                    .key(primary, Direction::Click)
                    .map_err(|e| format!("Key press failed: {}", e))?;
                // Release modifiers in reverse order
                for modifier in modifiers.iter().rev() {
                    enigo
                        .key(*modifier, Direction::Release)
                        .map_err(|e| format!("Modifier release failed: {}", e))?;
                }
                Ok(ComputerResult {
                    success: true,
                    output: format!("Pressed keys: {}", keys),
                    screenshot: None,
                    screen_size: None,
                    cursor_position: None,
                })
            }
            ComputerAction::Scroll {
                x,
                y,
                ref direction,
                amount,
            } => {
                let mut enigo = Self::create_enigo()?;
                if let (Some(sx), Some(sy)) = (x, y) {
                    enigo
                        .move_mouse(sx as i32, sy as i32, Coordinate::Abs)
                        .map_err(|e| format!("Scroll position move failed: {}", e))?;
                }
                let scroll_amount = amount as i32;
                match direction.as_str() {
                    "up" => enigo
                        .scroll(scroll_amount, enigo::Axis::Vertical)
                        .map_err(|e| format!("Scroll failed: {}", e))?,
                    "down" => enigo
                        .scroll(-scroll_amount, enigo::Axis::Vertical)
                        .map_err(|e| format!("Scroll failed: {}", e))?,
                    "left" => enigo
                        .scroll(-scroll_amount, enigo::Axis::Horizontal)
                        .map_err(|e| format!("Scroll failed: {}", e))?,
                    "right" => enigo
                        .scroll(scroll_amount, enigo::Axis::Horizontal)
                        .map_err(|e| format!("Scroll failed: {}", e))?,
                    other => {
                        return Err(format!(
                            "Invalid scroll direction '{}'. Use up/down/left/right.",
                            other
                        ))
                    }
                }
                let pos = match (x, y) {
                    (Some(px), Some(py)) => format!("({}, {})", px, py),
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

/// Parse a key combination string like "cmd+c" or "ctrl+shift+a" into
/// (modifier keys, primary key).
pub(crate) fn parse_key_combination(combo: &str) -> Result<(Vec<Key>, Key), String> {
    let parts: Vec<&str> = combo.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
        return Err("Empty key combination".into());
    }
    let mut modifiers = Vec::new();
    for part in &parts[..parts.len() - 1] {
        let modifier = match part.to_lowercase().as_str() {
            "cmd" | "command" | "meta" | "super" | "win" => Key::Meta,
            "ctrl" | "control" => Key::Control,
            "shift" => Key::Shift,
            "alt" | "option" | "opt" => Key::Alt,
            other => return Err(format!("Unknown modifier key: '{}'", other)),
        };
        modifiers.push(modifier);
    }
    let primary_str = parts.last().unwrap();
    let primary = parse_single_key(primary_str)?;
    Ok((modifiers, primary))
}

pub(crate) fn parse_single_key(name: &str) -> Result<Key, String> {
    match name.to_lowercase().as_str() {
        "enter" | "return" => Ok(Key::Return),
        "tab" => Ok(Key::Tab),
        "space" => Ok(Key::Space),
        "backspace" => Ok(Key::Backspace),
        "delete" | "del" => Ok(Key::Delete),
        "escape" | "esc" => Ok(Key::Escape),
        "up" => Ok(Key::UpArrow),
        "down" => Ok(Key::DownArrow),
        "left" => Ok(Key::LeftArrow),
        "right" => Ok(Key::RightArrow),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" => Ok(Key::PageUp),
        "pagedown" => Ok(Key::PageDown),
        "capslock" => Ok(Key::CapsLock),
        "f1" => Ok(Key::F1),
        "f2" => Ok(Key::F2),
        "f3" => Ok(Key::F3),
        "f4" => Ok(Key::F4),
        "f5" => Ok(Key::F5),
        "f6" => Ok(Key::F6),
        "f7" => Ok(Key::F7),
        "f8" => Ok(Key::F8),
        "f9" => Ok(Key::F9),
        "f10" => Ok(Key::F10),
        "f11" => Ok(Key::F11),
        "f12" => Ok(Key::F12),
        s if s.chars().count() == 1 => Ok(Key::Unicode(s.chars().next().unwrap())),
        other => Err(format!("Unknown key: '{}'. Use single characters or named keys (enter, tab, space, backspace, delete, escape, up, down, left, right, home, end, pageup, pagedown, f1-f12).", other)),
    }
}

pub(crate) fn parse_mouse_button(button: Option<&str>) -> Button {
    match button.unwrap_or("left") {
        "right" => Button::Right,
        "middle" => Button::Middle,
        _ => Button::Left,
    }
}
