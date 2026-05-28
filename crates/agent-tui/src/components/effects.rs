use agent_core::SessionId;

use super::{
    AgentOverlaySnapshot, CommandPaletteSnapshot, FocusTarget, HelpOverlaySnapshot,
    McpConnectivityEntry, McpOverlaySnapshot, McpPromptEntry, McpResourceEntry, McpToolEntry,
    ModelOverlaySnapshot, ModelProfileTestResult, PermissionRequest, PluginOverlaySnapshot,
    SessionInfo, SkillOverlaySnapshot, StatusInfo,
};

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum CrossPanelEffect {
    SwitchFocus(FocusTarget),
    ShowPermissionPrompt(PermissionRequest),
    ResolvePermissionPrompt {
        request_id: String,
        approved: bool,
    },
    DismissPermissionPrompt,
    UpdateSessionList(Vec<SessionInfo>),
    SetStatus(StatusInfo),
    NavigateToSession(SessionId),
    StartStreaming,
    StopStreaming,
    ShowMcpOverlay(McpOverlaySnapshot),
    DismissMcpOverlay,
    McpToolsLoaded {
        server_id: String,
        tools: Vec<McpToolEntry>,
        healthy: bool,
        error: Option<String>,
    },
    McpConnectivityChecked(McpConnectivityEntry),
    McpResourcesLoaded {
        server_id: String,
        resources: Vec<McpResourceEntry>,
    },
    McpPromptsLoaded {
        server_id: String,
        prompts: Vec<McpPromptEntry>,
    },
    McpResourceRead {
        server_id: String,
        uri: String,
        preview: String,
    },
    /// Open the command palette overlay.
    ShowCommandPalette,
    /// Refresh command palette dynamic rows before opening or while visible.
    UpdateCommandPalette(CommandPaletteSnapshot),
    /// Close the command palette overlay.
    DismissCommandPalette,
    /// Insert the given text at the start of the chat input and place the
    /// cursor at the end. Used by the command palette to hand back a slash
    /// prefix that needs an argument.
    PrefillChatInput(String),
    /// Build/refresh the skills overlay snapshot and open it.
    ShowSkillsOverlay(SkillOverlaySnapshot),
    /// Close the skills overlay.
    DismissSkillsOverlay,
    /// Deliver a skill's rendered markdown body to the open overlay so it
    /// can switch to inline detail view.
    ShowSkillBody {
        skill_id: String,
        body: String,
    },
    ShowModelOverlay(ModelOverlaySnapshot),
    ModelProfileTested(ModelProfileTestResult),
    DismissModelOverlay,
    ShowAgentSettingsOverlay(AgentOverlaySnapshot),
    DismissAgentSettingsOverlay,
    ShowPluginsOverlay(PluginOverlaySnapshot),
    DismissPluginsOverlay,
    ShowHooksOverlay(agent_core::facade::HooksSettingsView),
    DismissHooksOverlay,
    ShowInstructionsOverlay(agent_core::facade::InstructionsView),
    ShowSystemPromptOverlay(agent_core::facade::InstructionsView),
    DismissInstructionsOverlay,
    ShowHelpOverlay(HelpOverlaySnapshot),
    DismissHelpOverlay,
}
