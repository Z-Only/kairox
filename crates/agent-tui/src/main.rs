mod app;
mod app_state;
mod components;
mod keybindings;
mod runtime_dispatch;
mod view;

use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;

use agent_config::Config;
use agent_core::{AppFacade, StartSessionRequest};
use agent_memory::SqliteMemoryStore;
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
use components::{Command, ProjectInfo, SessionInfo, SessionState};
use runtime_dispatch::{
    dispatch_commands, project_info_from_meta, restore_session_draft, session_info_from_meta,
};

// ---------------------------------------------------------------------------
// AppEvent — unified event type for the main loop
// ---------------------------------------------------------------------------

enum AppEvent {
    Key(crossterm::event::KeyEvent),
    DomainEvent(Box<agent_core::DomainEvent>),
    Tick,
}

fn walk_workspace_files(root: &std::path::Path, max: usize) -> Vec<String> {
    let mut paths = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    let skip_dirs: &[&str] = &[
        ".git",
        "node_modules",
        "target",
        ".claude",
        ".kairox",
        "__pycache__",
        ".venv",
        "venv",
        ".tox",
        ".eggs",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
        "dist",
        "build",
        ".next",
        ".nuxt",
        ".output",
    ];

    while let Some(dir) = dirs.pop() {
        if paths.len() >= max {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if paths.len() >= max {
                break;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let is_hidden = name.starts_with('.');
            if file_type.is_dir() {
                if skip_dirs.contains(&name.as_ref())
                    || (is_hidden && name.as_ref() != "." && name.as_ref() != "..")
                {
                    continue;
                }
                dirs.push(entry.path());
            } else if file_type.is_file() || file_type.is_symlink() {
                if is_hidden && !name.starts_with(".env") {
                    continue;
                }
                if let Ok(relative) = entry.path().strip_prefix(root) {
                    paths.push(relative.to_string_lossy().to_string());
                }
            }
        }
    }

    paths.sort();
    paths
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

    // 3. Load config and build runtime
    let mut startup_messages = Vec::new();
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            startup_messages.push(format!("Config warning: {e}, using defaults"));
            Config::defaults()
        }
    };
    let router = config.build_router();
    let profile = config.default_profile();

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let home_dir = std::path::PathBuf::from(home);
    let data_dir = home_dir.join(".kairox");
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
    let workspace_files = walk_workspace_files(&workspace_path, 500);
    let skill_roots = agent_runtime::skills::build_default_skill_roots(&home_dir, &workspace_path);
    let skill_settings_roots =
        agent_runtime::skills::build_default_skill_settings_roots(&home_dir, &workspace_path);
    let skill_registry = agent_skills::FileSkillRegistry::discover(skill_roots).await?;

    let ollama_clients = agent_config::build_ollama_clients(&config);
    let config_arc = std::sync::Arc::new(config);
    let runtime = Arc::new(
        LocalRuntime::new(store, router)
            .with_permission_mode(PermissionMode::Suggest)
            .with_context_limit(100_000)
            .with_memory_store(mem_store)
            .with_config(config_arc)
            .with_ollama_clients(ollama_clients)
            .with_skill_registry(Arc::new(skill_registry))
            .with_skill_settings_roots(skill_settings_roots)
            .with_skill_catalog(Some(data_dir.clone()))
            .with_builtin_tools(workspace_path.clone())
            .await,
    );

    // Try to restore previous workspace and sessions, or create fresh ones
    let workspace_path_str = workspace_path.display().to_string();

    let (workspace_id, mut app_sessions, projects) = {
        // Try to find an existing workspace for this path
        let workspaces = runtime.list_workspaces().await.unwrap_or_default();
        let existing = workspaces.iter().find(|w| w.path == workspace_path_str);

        if let Some(ws) = existing {
            let sessions = runtime
                .list_sessions(&ws.workspace_id)
                .await
                .unwrap_or_default();
            let archived_sessions = runtime
                .list_archived_sessions(&ws.workspace_id)
                .await
                .unwrap_or_default();
            let projects_meta = runtime
                .list_projects(&ws.workspace_id)
                .await
                .unwrap_or_default();
            let mut projects: Vec<ProjectInfo> = projects_meta
                .into_iter()
                .map(project_info_from_meta)
                .collect();
            for project in &mut projects {
                project.git_status = runtime
                    .get_project_git_status(project.id.clone())
                    .await
                    .ok();
                project.instruction_summary = runtime
                    .get_project_instruction_summary(project.id.clone())
                    .await
                    .ok();
            }
            let mut session_infos: Vec<SessionInfo> = sessions
                .into_iter()
                .map(|s| session_info_from_meta(s, false, &None))
                .collect();
            for project in &projects {
                let project_sessions = runtime
                    .list_project_sessions(project.id.clone())
                    .await
                    .unwrap_or_default();
                session_infos.extend(
                    project_sessions
                        .into_iter()
                        .map(|s| session_info_from_meta(s, false, &None)),
                );
            }
            session_infos.extend(
                archived_sessions
                    .into_iter()
                    .map(|s| session_info_from_meta(s, true, &None)),
            );
            (ws.workspace_id.clone(), session_infos, projects)
        } else {
            let ws = runtime.open_workspace(workspace_path_str).await?;
            (ws.workspace_id, Vec::new(), Vec::new())
        }
    };

    // If no sessions exist, create a new one
    if app_sessions.iter().all(|session| session.archived) {
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace_id.clone(),
                model_profile: profile.clone(),
                permission_mode: None,
            })
            .await?;
        app_sessions.push(SessionInfo {
            id: session_id,
            title: format!("Session using {profile}"),
            model_profile: profile.clone(),
            state: SessionState::Idle,
            pinned: false,
            archived: false,
            project_id: None,
            worktree_path: None,
            branch: None,
            visibility: None,
        });
    }

    // 4. Create App with restored sessions
    let active_session_id = app_sessions
        .iter()
        .rfind(|session| !session.archived)
        .expect("at least one active session must exist")
        .id
        .clone();

    let mut app = App::new(&profile, PermissionMode::Suggest, workspace_id.clone());
    app.chat
        .set_workspace_files(workspace_path.clone(), workspace_files);
    app.current_session_id = Some(active_session_id.clone());
    app.state.sessions = app_sessions;
    app.state.projects = projects;

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
    restore_session_draft(runtime.store(), &mut app, &active_session_id).await;

    // Select the current session in the sessions panel
    if !app.state.sessions.is_empty() {
        let rows =
            components::sessions::session_list_rows(&app.state.projects, &app.state.sessions);
        let selected = rows
            .iter()
            .position(|row| {
                matches!(row, components::sessions::SessionListRow::Session(session_id) if session_id == &active_session_id)
            })
            .unwrap_or_else(|| rows.len().saturating_sub(1));
        app.sessions.state.select(Some(selected));
    }

    app.sync_status_bar();
    for message in startup_messages {
        app.state.push_status_message(message.clone());
        app.status_bar.push_notification(message);
    }
    app.sync_component_focus();
    terminal.clear()?;

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
                    let command_palette_was_visible = app.command_palette.is_visible();
                    let commands = app.handle_crossterm_event(&crossterm_event);
                    if !command_palette_was_visible && app.command_palette.is_visible() {
                        app::refresh_command_palette(&runtime, &mut app).await;
                    }
                    dispatch_commands(&runtime, &mut app, commands).await;
                }
                AppEvent::DomainEvent(domain_event) => {
                    // Only process events for the current session
                    if let Some(ref sid) = app.current_session_id {
                        if domain_event.session_id == *sid {
                            app.handle_domain_event(&domain_event);

                            // Drain any messages the user queued while the
                            // session was busy. We drain on
                            // `AssistantMessageCompleted` to mirror the GUI
                            // "end-of-turn" signal — the runtime is ready to
                            // accept the next user turn at that point.
                            if matches!(
                                domain_event.payload,
                                agent_core::EventPayload::AssistantMessageCompleted { .. }
                            ) {
                                let queued = app.chat.drain_queue();
                                if !queued.is_empty() {
                                    if let Some(session_id) = app.current_session_id.clone() {
                                        let workspace_id = app.workspace_id.clone();
                                        let drain_cmds: Vec<Command> = queued
                                            .into_iter()
                                            .map(|q| Command::SendMessage {
                                                workspace_id: workspace_id.clone(),
                                                session_id: session_id.clone(),
                                                content: q.content,
                                                attachments: q.attachments,
                                            })
                                            .collect();
                                        dispatch_commands(&runtime, &mut app, drain_cmds).await;
                                    }
                                }
                            }
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
