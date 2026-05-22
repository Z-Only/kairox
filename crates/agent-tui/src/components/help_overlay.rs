//! Help overlay for global and context-specific TUI shortcuts.

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, FocusTarget, HelpOverlaySnapshot,
};

#[derive(Debug, Clone)]
pub struct HelpOverlay {
    focused: bool,
    visible: bool,
    snapshot: HelpOverlaySnapshot,
}

impl Default for HelpOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            snapshot: HelpOverlaySnapshot {
                focus: FocusTarget::Chat,
            },
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: HelpOverlaySnapshot) {
        self.visible = true;
        self.snapshot = snapshot;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shortcut {
    key: &'static str,
    label: &'static str,
}

pub fn render_help_overlay(area: Rect, frame: &mut Frame, overlay: &HelpOverlay) {
    let modal_width = 84.min(area.width.saturating_sub(4));
    let modal_height = 26.min(area.height.saturating_sub(2));
    if modal_width == 0 || modal_height == 0 {
        return;
    }
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Help / Keybindings ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let paragraph = Paragraph::new(help_lines(overlay.snapshot.focus))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(paragraph, inner);
}

fn help_lines(focus: FocusTarget) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            current_label_prefix(focus),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            current_label(focus),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::default());
    lines.push(section_line("Global shortcuts"));
    lines.extend(shortcut_lines(&[
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
    ]));
    lines.push(Line::default());
    lines.push(section_line(if is_overlay_focus(focus) {
        "Current overlay shortcuts"
    } else {
        "Current focus shortcuts"
    }));
    lines.extend(shortcut_lines(context_shortcuts(focus)));
    lines.push(Line::default());
    lines.push(section_line("Common commands"));
    lines.extend(shortcut_lines(&[
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
    ]));
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("F1", key_style()),
        Span::raw(" or "),
        Span::styled("Esc", key_style()),
        Span::raw(" closes help"),
    ]));
    lines
}

fn section_line(label: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        label,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ))
}

fn shortcut_lines(shortcuts: &[Shortcut]) -> Vec<Line<'static>> {
    shortcuts
        .chunks(2)
        .map(|chunk| {
            let mut spans = Vec::new();
            for (index, shortcut) in chunk.iter().enumerate() {
                if index > 0 {
                    spans.push(Span::raw("    "));
                }
                spans.push(Span::styled(shortcut.key, key_style()));
                spans.push(Span::raw(" "));
                spans.push(Span::raw(shortcut.label));
            }
            Line::from(spans)
        })
        .collect()
}

fn key_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

fn current_label_prefix(focus: FocusTarget) -> &'static str {
    if is_overlay_focus(focus) {
        "Current overlay: "
    } else {
        "Current focus: "
    }
}

fn current_label(focus: FocusTarget) -> &'static str {
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

fn is_overlay_focus(focus: FocusTarget) -> bool {
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

fn context_shortcuts(focus: FocusTarget) -> &'static [Shortcut] {
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

impl Component for HelpOverlay {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        if !self.visible {
            return (Vec::new(), Vec::new());
        }

        match key.code {
            KeyCode::Esc | KeyCode::F(1) => {
                self.hide();
                (vec![CrossPanelEffect::DismissHelpOverlay], Vec::new())
            }
            _ => (Vec::new(), Vec::new()),
        }
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowHelpOverlay(snapshot) => self.show(*snapshot),
            CrossPanelEffect::DismissHelpOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if self.visible {
            render_help_overlay(area, frame, self);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}
