mod app;
mod view;

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = SqliteEventStore::in_memory().await?;
    let model = FakeModelClient::new(vec!["hello from fake model".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace(std::env::current_dir()?.display().to_string())
        .await?;
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await?;
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
        })
        .await?;

    let projection = runtime.get_session_projection(session_id).await?;
    let mut app = app::TuiApp::default();
    app.set_projection(projection);
    app.input = "hello".into();
    app.set_status(format!("ready (input-bytes: {})", app.input.len()));

    for line in view::render_lines(&app.projection) {
        println!("{line}");
    }

    if !app.status.is_empty() {
        println!("status: {}", app.status);
    }

    Ok(())
}
