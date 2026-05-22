use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

pub fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
    Event::Key(KeyEvent::new(code, modifiers))
}
