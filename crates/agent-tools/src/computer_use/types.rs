use serde::{Deserialize, Serialize};

/// Actions the computer use tool can perform.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ComputerAction {
    /// Take a screenshot of the entire screen or a region.
    Screenshot {
        /// Optional region [x, y, width, height]. None = full screen.
        region: Option<[u32; 4]>,
    },
    /// Move the mouse to coordinates.
    MouseMove { x: u32, y: u32 },
    /// Click at coordinates (or current position if not specified).
    MouseClick {
        x: Option<u32>,
        y: Option<u32>,
        button: Option<String>,
        click_count: Option<u32>,
    },
    /// Drag from one position to another.
    MouseDrag {
        from_x: u32,
        from_y: u32,
        to_x: u32,
        to_y: u32,
    },
    /// Type text (keyboard input).
    KeyboardType { text: String },
    /// Press a key combination (e.g., "cmd+c", "ctrl+shift+t").
    KeyPress { keys: String },
    /// Scroll at current position or specified coordinates.
    Scroll {
        x: Option<u32>,
        y: Option<u32>,
        direction: String,
        amount: u32,
    },
    /// Wait for a specified duration.
    Wait { duration_ms: u64 },
    /// Get current screen dimensions.
    GetScreenSize {},
    /// Get the position of the mouse cursor.
    GetCursorPosition {},
}

/// Result of a computer action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputerResult {
    pub success: bool,
    pub output: String,
    /// Base64-encoded screenshot if one was taken/requested.
    pub screenshot: Option<String>,
    /// Screen dimensions if requested.
    pub screen_size: Option<ScreenSize>,
    /// Cursor position if requested.
    pub cursor_position: Option<CursorPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPosition {
    pub x: u32,
    pub y: u32,
}

/// The model-expected coordinate space for screenshot resolution.
/// Screenshots are resized to fit within this to improve coordinate accuracy.
pub const MODEL_SCREENSHOT_MAX_WIDTH: u32 = 1280;
pub const MODEL_SCREENSHOT_MAX_HEIGHT: u32 = 800;
