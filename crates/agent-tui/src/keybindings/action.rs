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
    ToggleInstructionsOverlay,
    ToggleHooksOverlay,

    // -- L3 Ctrl -----------------------------------------------------------
    InterruptOrQuit,
    /// Ctrl+M — open/close the MCP server overlay.
    ToggleMcpOverlay,
    /// Ctrl+P — open/close the command palette.
    ToggleCommandPalette,
    /// Ctrl+S — open/close the native skills overlay.
    ToggleSkillsOverlay,
    /// Ctrl+L — open/close the model profile selector overlay.
    ToggleModelOverlay,
    /// Ctrl+G — open/close the plugin manager overlay.
    TogglePluginsOverlay,

    // -- L4 Function -------------------------------------------------------
    Help,
    RenameSession,
    ToggleTraceDensity,
    CycleTraceTabNext,
    CycleTraceTabPrevious,
    CycleMemoryScope,
    StartMemorySearch,
    RetrySelectedTask,
    CancelSelectedTask,
    DeleteSelectedMemory,
    ConfirmMemoryDelete,
    OpenArchiveManager,

    // -- Status bar shortcuts ---------------------------------------------
    /// Cycle through approval-axis policies (Shift+A — outside Sessions focus).
    CycleApprovalPolicy,
    /// Cycle through sandbox-axis policies (Shift+B).
    CycleSandboxPolicy,
    /// Toggle the context usage details popover (Alt+C).
    ToggleContextDetails,

    // -- Input -------------------------------------------------------------
    InputCharacter(char),
    InputBackspace,
    InputDelete,
    InputNewline,
    InputHistoryUp,
    InputHistoryDown,
    InputPaste(String),
    ApplyQueueAction(crate::components::QueueAction),

    // -- Navigation --------------------------------------------------------
    ScrollUp,
    ScrollDown,
    SelectSession,

    /// Key was not bound in the current context.
    Unhandled,
}
