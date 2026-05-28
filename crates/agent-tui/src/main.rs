mod app;
mod app_state;
mod components;
mod focus;
mod keybindings;
mod runtime_dispatch;
mod scheduler;
mod view;
mod workspace_recovery;

use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;

use agent_core::AppFacade;
use agent_runtime::ui_bootstrap::{
    build_ui_runtime_from_store, connect_ui_event_store, default_data_dir, default_home_dir,
    ensure_workspace_session, load_catalog_sources, load_ui_config, spawn_runtime_event_forwarder,
    UiRuntimeOptions,
};
use agent_tools::{ApprovalPolicy, SandboxPolicy};
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
use components::{Command, ProjectInfo, SessionInfo};
use runtime_dispatch::{
    dispatch_commands, project_info_from_meta, restore_session_draft, session_info_from_meta,
};
use workspace_recovery::{
    format_known_workspaces, parse_workspace_args, prompt_workspace_selector,
    resolve_workspace_selector, workspace_usage, KnownWorkspace, WorkspaceCliMode,
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
    let cli = parse_workspace_args(std::env::args().skip(1)).map_err(anyhow::Error::msg)?;
    if matches!(&cli.mode, WorkspaceCliMode::Help) {
        println!("{}", workspace_usage());
        return Ok(());
    }

    let mut startup_messages = Vec::new();
    let home_dir = default_home_dir();
    let data_dir = default_data_dir(&home_dir);
    let store = connect_ui_event_store(&data_dir, "kairox.sqlite").await?;
    let known_workspaces: Vec<KnownWorkspace> = store
        .list_workspaces()
        .await?
        .into_iter()
        .map(|workspace| KnownWorkspace {
            workspace_id: workspace.workspace_id,
            path: workspace.path,
        })
        .collect();

    match cli.mode {
        WorkspaceCliMode::CurrentDir | WorkspaceCliMode::Help => {}
        WorkspaceCliMode::List => {
            print!("{}", format_known_workspaces(&known_workspaces));
            return Ok(());
        }
        WorkspaceCliMode::Select => {
            if known_workspaces.is_empty() {
                print!("{}", format_known_workspaces(&known_workspaces));
                return Ok(());
            }
            let Some(selector) =
                prompt_workspace_selector(&known_workspaces).map_err(anyhow::Error::msg)?
            else {
                return Ok(());
            };
            let path = resolve_workspace_selector(&known_workspaces, &selector)
                .map_err(anyhow::Error::msg)?;
            std::env::set_current_dir(&path)?;
            startup_messages.push(format!("Recovered workspace {}", path.display()));
        }
        WorkspaceCliMode::Use(selector) => {
            let path = resolve_workspace_selector(&known_workspaces, &selector)
                .map_err(anyhow::Error::msg)?;
            std::env::set_current_dir(&path)?;
            startup_messages.push(format!("Recovered workspace {}", path.display()));
        }
    }

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
    let workspace_path = std::env::current_dir()?;
    let workspace_files = walk_workspace_files(&workspace_path, 500);
    let config_load = load_ui_config(&data_dir);
    startup_messages.extend(config_load.warnings);
    let catalog_load = load_catalog_sources(&data_dir);
    startup_messages.extend(catalog_load.warnings);
    let profile = config_load.config.default_profile();
    let runtime_bootstrap = build_ui_runtime_from_store(
        store,
        UiRuntimeOptions::new(
            home_dir.clone(),
            data_dir.clone(),
            "kairox.sqlite",
            workspace_path.clone(),
            ApprovalPolicy::default(),
            SandboxPolicy::default(),
            config_load.config,
            catalog_load.sources,
        ),
    )
    .await?;
    let runtime = Arc::new(runtime_bootstrap.runtime);

    // Try to restore previous workspace and sessions, or create fresh ones
    let workspace_path_str = workspace_path.display().to_string();

    let workspace_bootstrap =
        ensure_workspace_session(runtime.as_ref(), workspace_path_str, profile.clone()).await?;
    let workspace_id = workspace_bootstrap.workspace.workspace_id.clone();
    let active_session_id = workspace_bootstrap.session_id.clone();

    let sessions = runtime
        .list_sessions(&workspace_id)
        .await
        .unwrap_or_default();
    let archived_sessions = runtime
        .list_archived_sessions(&workspace_id)
        .await
        .unwrap_or_default();
    let projects_meta = runtime
        .list_projects(&workspace_id)
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
    let current = Some(active_session_id.clone());
    let mut app_sessions: Vec<SessionInfo> = sessions
        .into_iter()
        .map(|s| session_info_from_meta(s, false, &current))
        .collect();
    for project in &projects {
        let project_sessions = runtime
            .list_project_sessions(project.id.clone())
            .await
            .unwrap_or_default();
        app_sessions.extend(
            project_sessions
                .into_iter()
                .map(|s| session_info_from_meta(s, false, &current)),
        );
    }
    app_sessions.extend(
        archived_sessions
            .into_iter()
            .map(|s| session_info_from_meta(s, true, &current)),
    );

    // 4. Create App with restored sessions
    let mut app = App::new(&profile, workspace_id.clone());
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
    let event_task = spawn_runtime_event_forwarder(runtime.as_ref(), move |event| {
        let tx_events = tx_events.clone();
        async move {
            tx_events
                .send(AppEvent::DomainEvent(Box::new(event)))
                .await
                .is_ok()
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
