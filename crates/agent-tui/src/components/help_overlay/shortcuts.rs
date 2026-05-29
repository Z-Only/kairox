//! Static shortcut data for the help overlay.
//!
//! Each function returns a slice of [`Shortcut`] entries or a label
//! string based on the current [`FocusTarget`]. Extracted from
//! [`super::render`] to separate static content from layout logic.

use crate::components::FocusTarget;

use super::types::Shortcut;

pub fn current_label_prefix(focus: FocusTarget) -> &'static str {
    if is_overlay_focus(focus) {
        "Current overlay: "
    } else {
        "Current focus: "
    }
}

pub fn current_label(focus: FocusTarget) -> &'static str {
    match focus {
        FocusTarget::Chat => "Chat composer",
        FocusTarget::Sessions => "Sessions panel",
        FocusTarget::Trace => "Trace panel",
        FocusTarget::PermissionModal => "Permission prompt",
        FocusTarget::McpOverlay => "MCP manager",
        FocusTarget::CommandPalette => "Command palette",
        FocusTarget::SkillsOverlay => "Skills manager",
        FocusTarget::ModelOverlay => "Model selector",
        FocusTarget::AgentOverlay => "Agent settings",
        FocusTarget::PluginOverlay => "Plugin manager",
        FocusTarget::HooksOverlay => "Hooks settings",
        FocusTarget::InstructionsOverlay => "Instructions settings",
    }
}

pub fn is_overlay_focus(focus: FocusTarget) -> bool {
    matches!(
        focus,
        FocusTarget::PermissionModal
            | FocusTarget::McpOverlay
            | FocusTarget::CommandPalette
            | FocusTarget::SkillsOverlay
            | FocusTarget::ModelOverlay
            | FocusTarget::AgentOverlay
            | FocusTarget::PluginOverlay
            | FocusTarget::HooksOverlay
            | FocusTarget::InstructionsOverlay
    )
}

pub fn global_shortcuts() -> &'static [Shortcut] {
    &[
        Shortcut {
            key: "F1",
            label: "toggle this help",
        },
        Shortcut {
            key: "Tab",
            label: "cycle focus",
        },
        Shortcut {
            key: "Esc",
            label: "close overlay or cancel",
        },
        Shortcut {
            key: "Alt+1/2/3",
            label: "focus chat/sessions/trace",
        },
        Shortcut {
            key: "Alt+N",
            label: "new session",
        },
        Shortcut {
            key: "Alt+S / Alt+T",
            label: "toggle sidebars",
        },
        Shortcut {
            key: "Ctrl+P",
            label: "command palette",
        },
        Shortcut {
            key: "Ctrl+M",
            label: "MCP manager",
        },
        Shortcut {
            key: "Ctrl+S",
            label: "skills",
        },
        Shortcut {
            key: "Ctrl+L",
            label: "models",
        },
        Shortcut {
            key: "Ctrl+G",
            label: "plugins",
        },
        Shortcut {
            key: "Alt+I / Alt+H",
            label: "instructions/hooks",
        },
        Shortcut {
            key: "Ctrl+C",
            label: "interrupt, then quit",
        },
    ]
}

pub fn common_commands() -> &'static [Shortcut] {
    &[
        Shortcut {
            key: ":compact",
            label: "summarise older history",
        },
        Shortcut {
            key: ":model <alias>",
            label: "switch model profile",
        },
        Shortcut {
            key: ":attach <path>",
            label: "attach file to next message",
        },
        Shortcut {
            key: ":project draft",
            label: "start draft session",
        },
        Shortcut {
            key: ":skills",
            label: "list native skills",
        },
        Shortcut {
            key: ":monitors",
            label: "list active monitors",
        },
        Shortcut {
            key: ":monitor stop <id>",
            label: "stop a monitor",
        },
    ]
}

pub fn context_shortcuts(focus: FocusTarget) -> &'static [Shortcut] {
    match focus {
        FocusTarget::Chat => &[
            Shortcut {
                key: "Enter",
                label: "send in single-line mode",
            },
            Shortcut {
                key: "Ctrl+Enter",
                label: "send from multi-line mode",
            },
            Shortcut {
                key: "Alt+E",
                label: "toggle input mode",
            },
            Shortcut {
                key: "Up/Down",
                label: "draft history",
            },
            Shortcut {
                key: "Alt+Arrows",
                label: "work with queued messages",
            },
        ],
        FocusTarget::Sessions => &[
            Shortcut {
                key: "Enter",
                label: "switch to selected session",
            },
            Shortcut {
                key: "Up/Down",
                label: "move selection",
            },
            Shortcut {
                key: "F2",
                label: "rename session",
            },
            Shortcut {
                key: "x",
                label: "open session actions",
            },
            Shortcut {
                key: "a",
                label: "archive manager",
            },
        ],
        FocusTarget::Trace => &[
            Shortcut {
                key: "Left/Right",
                label: "cycle trace tabs",
            },
            Shortcut {
                key: "[ / ]",
                label: "cycle trace tabs",
            },
            Shortcut {
                key: "F5",
                label: "toggle trace density",
            },
            Shortcut {
                key: "/",
                label: "search memories tab",
            },
            Shortcut {
                key: "r / c",
                label: "retry or cancel selected task",
            },
            Shortcut {
                key: "s / d",
                label: "memory scope or delete memory",
            },
        ],
        FocusTarget::PermissionModal => &[
            Shortcut {
                key: "y",
                label: "allow once",
            },
            Shortcut {
                key: "n / Esc",
                label: "deny",
            },
            Shortcut {
                key: "d",
                label: "deny all matching",
            },
            Shortcut {
                key: "t",
                label: "trust MCP server when available",
            },
        ],
        FocusTarget::CommandPalette => &[
            Shortcut {
                key: "type",
                label: "filter commands",
            },
            Shortcut {
                key: "Up/Down",
                label: "move selection",
            },
            Shortcut {
                key: "Enter",
                label: "run selected",
            },
            Shortcut {
                key: "Backspace",
                label: "edit filter",
            },
            Shortcut {
                key: "Esc",
                label: "close palette",
            },
        ],
        FocusTarget::McpOverlay => &[
            Shortcut {
                key: "Tab",
                label: "cycle tabs",
            },
            Shortcut {
                key: "j/k",
                label: "move selection",
            },
            Shortcut {
                key: "Enter",
                label: "start, stop, edit, or open item",
            },
            Shortcut {
                key: "h/c/r",
                label: "health, connectivity, reload",
            },
            Shortcut {
                key: "Esc",
                label: "close or back",
            },
        ],
        FocusTarget::SkillsOverlay => &[
            Shortcut {
                key: "Tab",
                label: "cycle tabs",
            },
            Shortcut {
                key: "j/k",
                label: "move selection",
            },
            Shortcut {
                key: "Enter",
                label: "open detail",
            },
            Shortcut {
                key: "a/d",
                label: "activate or deactivate",
            },
            Shortcut {
                key: "i/u/x",
                label: "install, update, delete",
            },
        ],
        FocusTarget::ModelOverlay => &[
            Shortcut {
                key: "j/k",
                label: "move profile",
            },
            Shortcut {
                key: "Enter",
                label: "switch selected profile",
            },
            Shortcut {
                key: "n/u",
                label: "new or edit profile",
            },
            Shortcut {
                key: "e/t/x",
                label: "enable, test, delete",
            },
            Shortcut {
                key: "J/K",
                label: "reorder profiles",
            },
            Shortcut {
                key: "Esc",
                label: "close or back",
            },
        ],
        FocusTarget::AgentOverlay => &[
            Shortcut {
                key: "j/k",
                label: "move selection",
            },
            Shortcut {
                key: "Enter/e",
                label: "edit selected",
            },
            Shortcut {
                key: "n/N",
                label: "new user/project agent",
            },
            Shortcut {
                key: "c/p",
                label: "copy to user/project",
            },
            Shortcut {
                key: "x/o",
                label: "delete or open folder",
            },
            Shortcut {
                key: "Esc",
                label: "close or cancel edit",
            },
        ],
        FocusTarget::PluginOverlay => &[
            Shortcut {
                key: "Tab",
                label: "cycle tabs",
            },
            Shortcut {
                key: "j/k",
                label: "move selection",
            },
            Shortcut {
                key: "e",
                label: "enable or disable",
            },
            Shortcut {
                key: "x",
                label: "delete",
            },
            Shortcut {
                key: "Esc",
                label: "close",
            },
        ],
        FocusTarget::HooksOverlay => &[
            Shortcut {
                key: "Tab",
                label: "cycle tabs",
            },
            Shortcut {
                key: "n/e/x",
                label: "new, edit, delete",
            },
            Shortcut {
                key: "Space",
                label: "toggle enabled",
            },
            Shortcut {
                key: "Esc",
                label: "close or back",
            },
        ],
        FocusTarget::InstructionsOverlay => &[
            Shortcut {
                key: "Tab",
                label: "cycle scopes",
            },
            Shortcut {
                key: "F2",
                label: "save edits",
            },
            Shortcut {
                key: "Enter",
                label: "insert newline",
            },
            Shortcut {
                key: "Esc",
                label: "close",
            },
        ],
    }
}
