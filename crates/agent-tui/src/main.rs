mod app;
mod app_state;
mod components;
mod keybindings;
mod view;

use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;

use agent_config::Config;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_memory::SqliteMemoryStore;
use agent_models::ModelRouter;
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
// Command dispatch — executes runtime commands and updates app state
// ---------------------------------------------------------------------------

async fn dispatch_commands(
    runtime: &Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
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
                    .resolve_permission(
                        &request_id,
                        agent_core::PermissionDecision {
                            request_id: request_id.clone(),
                            approve: approved,
                            reason: None,
                        },
                    )
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
                workspace_id: ws_id,
                model_profile: mp,
            } => {
                match runtime
                    .start_session(StartSessionRequest {
                        workspace_id: ws_id,
                        model_profile: mp.clone(),
                    })
                    .await
                {
                    Ok(session_id) => {
                        app.current_session_id = Some(session_id.clone());
                        app.state.sessions.push(SessionInfo {
                            id: session_id,
                            title: format!("Session using {mp}"),
                            model_profile: mp,
                            state: SessionState::Idle,
                            pinned: false,
                        });
                        app.state.current_session =
                            agent_core::projection::SessionProjection::default();
                        app.domain_events.clear();
                        app.state.render_scheduler.reset();
                        // Select the new session in the sessions panel
                        app.sessions
                            .state
                            .select(Some(app.state.sessions.len() - 1));
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

            Command::SwitchSession { session_id } => {
                let sid = session_id.clone();
                app.current_session_id = Some(sid.clone());

                // Update session states
                for session in &mut app.state.sessions {
                    if session.id == sid {
                        session.state = SessionState::Active;
                    } else if session.state == SessionState::Active {
                        session.state = SessionState::Idle;
                    }
                }

                // Load historical data for the switched-to session
                let projection = runtime.get_session_projection(sid.clone()).await;
                let trace = runtime.get_trace(sid.clone()).await;

                if let Ok(proj) = projection {
                    app.state.current_session = proj;
                }
                if let Ok(trc) = trace {
                    app.domain_events = trc.into_iter().map(|t| t.event).collect();
                }

                app.state.render_scheduler.mark_dirty_immediate();
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

    eprintln!(
        "Kairox TUI {}",
        agent_core::build_info::BuildInfo::from_env()
    );

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

    // 3. Load config and build runtime
    let config = Config::load().unwrap_or_else(|e| {
        eprintln!("Config warning: {e}, using defaults");
        Config::defaults()
    });
    let router = config.build_router();
    let profiles = config.profile_names();
    let profile = config.default_profile();

    eprintln!("Available model profiles: {:?}", profiles);
    eprintln!("Using profile: {profile}");

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let data_dir = std::path::PathBuf::from(home).join(".kairox");
    tokio::fs::create_dir_all(&data_dir).await?;
    let db_path = data_dir.join("kairox.sqlite");
    let database_url = format!(
        "sqlite:///{}",
        db_path.display().to_string().trim_start_matches('/')
    );
    let store = SqliteEventStore::connect(&database_url).await?;
    let mem_store = std::sync::Arc::new(SqliteMemoryStore::new(store.pool().clone()).await?)
        as std::sync::Arc<dyn agent_memory::MemoryStore>;
    let workspace_path = std::env::current_dir()?;

    let runtime = Arc::new(
        LocalRuntime::new(store, router)
            .with_permission_mode(PermissionMode::Suggest)
            .with_context_limit(100_000)
            .with_memory_store(mem_store)
            .with_builtin_tools(workspace_path.clone())
            .await,
    );

    // Try to restore previous workspace and sessions, or create fresh ones
    let workspace_path_str = workspace_path.display().to_string();

    let (workspace_id, mut app_sessions) = {
        // Try to find an existing workspace for this path
        let workspaces = runtime.list_workspaces().await.unwrap_or_default();
        let existing = workspaces.iter().find(|w| w.path == workspace_path_str);

        if let Some(ws) = existing {
            let sessions = runtime
                .list_sessions(&ws.workspace_id)
                .await
                .unwrap_or_default();
            let session_infos: Vec<SessionInfo> = sessions
                .iter()
                .map(|s| SessionInfo {
                    id: s.session_id.clone(),
                    title: s.title.clone(),
                    model_profile: s.model_profile.clone(),
                    state: SessionState::Idle,
                    pinned: false,
                })
                .collect();
            (ws.workspace_id.clone(), session_infos)
        } else {
            let ws = runtime.open_workspace(workspace_path_str).await?;
            (ws.workspace_id, Vec::new())
        }
    };

    // If no sessions exist, create a new one
    if app_sessions.is_empty() {
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace_id.clone(),
                model_profile: profile.clone(),
            })
            .await?;
        app_sessions.push(SessionInfo {
            id: session_id,
            title: format!("Session using {profile}"),
            model_profile: profile.clone(),
            state: SessionState::Idle,
            pinned: false,
        });
    }

    // 4. Create App with restored sessions
    let active_session_id = app_sessions.last().unwrap().id.clone();

    let mut app = App::new(&profile, PermissionMode::Suggest, workspace_id.clone());
    app.current_session_id = Some(active_session_id.clone());
    app.state.sessions = app_sessions;

    // Load the initial session projection and trace
    if let Ok(projection) = runtime
        .get_session_projection(active_session_id.clone())
        .await
    {
        app.state.current_session = projection;
    }
    if let Ok(trace) = runtime.get_trace(active_session_id.clone()).await {
        app.domain_events = trace.into_iter().map(|t| t.event).collect();
    }

    // Select the current session in the sessions panel
    if !app.state.sessions.is_empty() {
        app.sessions
            .state
            .select(Some(app.state.sessions.len() - 1));
    }

    app.sync_status_bar();
    app.sync_component_focus();

    // 5. Create channels + spawn tasks
    let (tx, mut rx) = mpsc::channel::<AppEvent>(256);

    // Domain event forwarder — subscribes to ALL runtime events
    let tx_events = tx.clone();
    let rt_handle = runtime.clone();
    let event_task = tokio::spawn(async move {
        let mut stream = rt_handle.subscribe_all();
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
                    // Only process events for the current session
                    if let Some(ref sid) = app.current_session_id {
                        if domain_event.session_id == *sid {
                            app.handle_domain_event(&domain_event);
                        }
                    }
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
