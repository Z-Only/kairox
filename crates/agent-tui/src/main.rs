mod app;
mod view;

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = SqliteEventStore::in_memory().await?;
    let profiles = detect_profiles();
    let profile = choose_profile(&profiles);

    // Use FakeModelClient for the TUI demo — real model adapters are available
    // via OpenAiCompatibleClient and OllamaClient. Full ModelRouter dispatch
    // will be wired when the interactive TUI (ratatui) is implemented.
    let model = FakeModelClient::new(vec!["hello from fake model".into()]);
    let runtime = LocalRuntime::new(store, model)
        .with_permission_mode(PermissionMode::Suggest)
        .with_context_limit(100_000);

    let workspace = runtime
        .open_workspace(std::env::current_dir()?.display().to_string())
        .await?;
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: profile.to_string(),
        })
        .await?;

    let args: Vec<String> = std::env::args().collect();
    let user_message = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "hello".into()
    };

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: user_message,
        })
        .await?;

    let projection = runtime.get_session_projection(session_id).await?;
    let mut app = app::TuiApp::default();
    app.set_projection(projection);
    app.set_status(format!("ready (profile: {profile})"));

    for line in view::render_lines(&app.projection) {
        println!("{line}");
    }

    if !app.status.is_empty() {
        println!("status: {}", app.status);
    }

    Ok(())
}
