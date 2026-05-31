//! Key-event handlers for [`MonitorOverlay`].

use crossterm::event::{Event, KeyCode};

use super::state::MonitorOverlay;
use crate::components::{Command, CrossPanelEffect};

impl MonitorOverlay {
    pub(super) fn move_down(&mut self) {
        let len = self.monitors.len();
        if len == 0 {
            return;
        }
        let next = match self.list_state.selected() {
            Some(i) if i + 1 < len => i + 1,
            Some(_) => len - 1,
            None => 0,
        };
        self.list_state.select(Some(next));
    }

    pub(super) fn move_up(&mut self) {
        if self.monitors.is_empty() {
            return;
        }
        let next = match self.list_state.selected() {
            Some(i) if i > 0 => i - 1,
            _ => 0,
        };
        self.list_state.select(Some(next));
    }

    pub(super) fn handle_key_event(
        &mut self,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        if !self.visible {
            return (Vec::new(), Vec::new());
        }

        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete => {
                if let Some(monitor) = self.selected_monitor() {
                    commands.push(Command::MonitorStop {
                        monitor_id: monitor.monitor_id.clone(),
                    });
                }
            }
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissMonitorOverlay);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                commands.push(Command::OpenMonitorOverlay);
            }
            _ => {}
        }

        (effects, commands)
    }
}
