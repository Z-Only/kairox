/// All possible actions produced by resolving a key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    // -- L1 Instant --------------------------------------------------------
    SendInput,
    Escape,
    FocusCycleNext,
    AllowPermission,
    DenyPermission,
    DenyAllPermission,
    ContextMenu,

    // -- L2 Alt ------------------------------------------------------------
    ToggleSessionsSidebar,
    ToggleTraceSidebar,
    ToggleInputMode,
    OpenProfileSelector,
    NewSession,
    Quit,
    FocusChat,
    FocusSessions,
    FocusTrace,

    // -- L3 Ctrl -----------------------------------------------------------
    InterruptOrQuit,
    Redraw,
    /// Ctrl+M — open/close the MCP server overlay.
    ToggleMcpOverlay,
    /// Ctrl+P — open/close the command palette.
    ToggleCommandPalette,

    // -- L4 Function -------------------------------------------------------
    Help,
    RenameSession,
    ToggleTraceDensity,

    // -- Status bar shortcuts ---------------------------------------------
    /// Cycle through permission modes (Shift+P).
    CyclePermissionMode,

    // -- Input -------------------------------------------------------------
    InputCharacter(char),
    InputBackspace,
    InputDelete,
    InputNewline,
    InputHistoryUp,
    InputHistoryDown,
    InputPaste(String),

    // -- Navigation --------------------------------------------------------
    ScrollUp,
    ScrollDown,
    SelectSession,

    /// Key was not bound in the current context.
    Unhandled,
}
