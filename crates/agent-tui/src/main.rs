mod app;
mod app_state;
mod components;
mod keybindings;
mod view;

use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use anyhow::Result;
use crossterm::event::{Event, EventStream};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use app::App;
use components::{Command, SessionInfo, SessionState};

// ---------------------------------------------------------------------------
// AppEvent — unified event type for the main loop
// ---------------------------------------------------------------------------

enum AppEvent {
    Key(crossterm::event::KeyEvent),
    DomainEvent(Box<agent_core::DomainEvent>),
    Tick,
}

// ---------------------------------------------------------------------------
// Profile detection (preserved from previous main)
// ---------------------------------------------------------------------------

fn detect_profiles() -> Vec<String> {
    let mut profiles = vec!["fake".to_string()];
    if std::env::var("OPENAI_API_KEY").is_ok() {
        profiles.insert(0, "fast".to_string());
    }
    profiles.insert(
        if profiles.len() > 1 { 1 } else { 0 },
        "local-code".to_string(),
    );
    profiles
}

fn choose_profile(profiles: &[String]) -> &str {
    eprintln!("Available model profiles: {:?}", profiles);
    let chosen = if profiles.iter().any(|p| p == "fast") {
        "fast"
    } else if profiles.iter().any(|p| p == "local-code") {
        "local-code"
    } else {
        "fake"
    };
    eprintln!("Using profile: {chosen}");
    chosen
}

// ---------------------------------------------------------------------------
// Command dispatch — executes runtime commands and updates app state
// ---------------------------------------------------------------------------

async fn dispatch_commands(
    runtime: &Arc<LocalRuntime<SqliteEventStore, FakeModelClient>>,
    app: &mut App,
    commands: Vec<Command>,
) {
    for command in commands {
        match command {
            Command::SendMessage {
                workspace_id,
                session_id,
                content,
            } => {
                if let Err(e) = runtime
                    .send_message(SendMessageRequest {
                        workspace_id,
                        session_id,
                        content,
                    })
                    .await
                {
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("[error: {e}]"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::DecidePermission {
                request_id,
                approved,
            } => {
                if let Err(e) = runtime
                    .decide_permission(agent_core::PermissionDecision {
                        request_id,
                        approve: approved,
                        reason: None,
                    })
                    .await
                {
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("[permission error: {e}]"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::CancelSession {
                workspace_id,
                session_id,
            } => {
                if let Err(e) = runtime.cancel_session(workspace_id, session_id).await {
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("[cancel error: {e}]"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty();
                }
            }

            Command::StartSession {
                workspace_id,
                model_profile,
            } => {
                match runtime
                    .start_session(StartSessionRequest {
                        workspace_id,
                        model_profile: model_profile.clone(),
                    })
                    .await
                {
                    Ok(session_id) => {
                        app.current_session_id = Some(session_id.clone());
                        app.state.sessions.push(SessionInfo {
                            id: session_id,
                            title: format!("Session using {model_profile}"),
                            model_profile,
                            state: SessionState::Idle,
                            pinned: false,
                        });
                        app.state.current_session =
                            agent_core::projection::SessionProjection::default();
                        app.domain_events.clear();
                        app.state.render_scheduler.reset();
                    }
                    Err(e) => {
                        app.state.current_session.messages.push(
                            agent_core::projection::ProjectedMessage {
                                role: agent_core::projection::ProjectedRole::Assistant,
                                content: format!("[start session error: {e}]"),
                            },
                        );
                        app.state.render_scheduler.mark_dirty();
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Setup terminal
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    // 2. Check size
    let size = terminal.size()?;
    if size.width < 80 || size.height < 24 {
        disable_raw_mode()?;
        crossterm::execute!(std::io::stdout(), LeaveAlternateScreen)?;
        eprintln!(
            "Terminal too small: {}x{}. Minimum: 80x24.",
            size.width, size.height
        );
        std::process::exit(1);
    }

    // 3. Init runtime
    let store = SqliteEventStore::in_memory().await?;
    let profiles = detect_profiles();
    let profile = choose_profile(&profiles);
    let workspace_path = std::env::current_dir()?;

    let model = FakeModelClient::new(vec!["hello from fake model".into()]);
    let runtime = Arc::new(
        LocalRuntime::new(store, model)
            .with_permission_mode(PermissionMode::Suggest)
            .with_context_limit(100_000)
            .with_builtin_tools(workspace_path.clone())
            .await,
    );

    let workspace = runtime
        .open_workspace(workspace_path.display().to_string())
        .await?;
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: profile.to_string(),
        })
        .await?;

    // 4. Create App
    let mut app = App::new(
        profile,
        PermissionMode::Suggest,
        workspace.workspace_id.clone(),
    );
    app.current_session_id = Some(session_id.clone());
    app.state.sessions.push(SessionInfo {
        id: session_id.clone(),
        title: format!("Session using {profile}"),
        model_profile: profile.to_string(),
        state: SessionState::Idle,
        pinned: false,
    });
    app.sync_status_bar();
    app.sync_component_focus();

    // 5. Create channels + spawn tasks
    let (tx, mut rx) = mpsc::channel::<AppEvent>(256);

    // Domain event forwarder — subscribes to runtime events for the current session
    let tx_events = tx.clone();
    let rt_session_id = session_id.clone();
    let rt_handle = runtime.clone();
    let event_task = tokio::spawn(async move {
        let mut stream = rt_handle.subscribe_session(rt_session_id);
        while let Some(event) = stream.next().await {
            if tx_events
                .send(AppEvent::DomainEvent(Box::new(event)))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Key reader — forwards crossterm key events
    let tx_keys = tx.clone();
    let key_task = tokio::spawn(async move {
        let mut reader = EventStream::new();
        while let Some(Ok(event)) = reader.next().await {
            if let Event::Key(key) = event {
                if tx_keys.send(AppEvent::Key(key)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Tick timer — fires every 16ms for render scheduling
    let tx_tick = tx;
    let tick_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(16));
        loop {
            interval.tick().await;
            if tx_tick.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    // 6. Main loop
    loop {
        if let Some(event) = rx.recv().await {
            match event {
                AppEvent::Key(key) => {
                    let crossterm_event = Event::Key(key);
                    let commands = app.handle_crossterm_event(&crossterm_event);
                    dispatch_commands(&runtime, &mut app, commands).await;
                }
                AppEvent::DomainEvent(domain_event) => {
                    app.handle_domain_event(&domain_event);
                }
                AppEvent::Tick => {
                    if app.state.render_scheduler.should_render() {
                        terminal.draw(|f| app.render(f))?;
                    }
                }
            }

            if app.quitting {
                break;
            }
        }
    }

    // 7. Cleanup
    event_task.abort();
    key_task.abort();
    tick_task.abort();

    drop(rx);

    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
