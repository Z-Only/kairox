use agent_tui::app::App;
use agent_tui::components::Component;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

pub fn render_app(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal should be created");
    terminal
        .draw(|frame| app.render(frame))
        .expect("app should render");
    terminal.backend().to_string()
}

pub fn render_component(component: &impl Component, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal should be created");
    terminal
        .draw(|frame| component.render(frame.area(), frame))
        .expect("component should render");
    terminal.backend().to_string()
}
