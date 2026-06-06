/// ```rust,no_run
/// use agent_sdk::KairoxSdk;
/// use futures::StreamExt;
///
/// #[tokio::main]
/// async fn main() -> Result<(), agent_sdk::SdkError> {
///     let sdk = KairoxSdk::builder()
///         .workspace("/path/to/project")
///         .build()
///         .await?;
///
///     let session = sdk.create_session().await?;
///     let mut stream = session.send_message("Explain this codebase").await?;
///
///     while let Some(event) = stream.next().await {
///         println!("{:?}", event);
///     }
///
///     Ok(())
/// }
/// ```
mod builder;
mod config;
mod error;
mod hooks;
mod session;

pub use builder::SdkBuilder;
pub use config::{SdkApprovalPolicy, SdkConfig, SdkSandboxPolicy};
pub use error::{SdkError, SdkResult};
pub use hooks::{HookAction, SdkHook, ToolHookContext};
pub use session::{MessageStream, SdkSession, StreamEvent};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use agent_core::AppFacade;
use agent_runtime::ui_bootstrap::UiRuntime;

/// The primary entry point for the Kairox SDK.
///
/// Holds a fully-wired [`LocalRuntime`](agent_runtime::LocalRuntime) and
/// exposes a simplified API for programmatic agent interaction.
pub struct KairoxSdk {
    runtime: Arc<UiRuntime>,
    workspace_path: PathBuf,
    hooks: Vec<Arc<dyn SdkHook>>,
}

impl std::fmt::Debug for KairoxSdk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KairoxSdk")
            .field("workspace_path", &self.workspace_path)
            .field("hook_count", &self.hooks.len())
            .finish_non_exhaustive()
    }
}

impl KairoxSdk {
    /// Start building a new SDK instance.
    pub fn builder() -> SdkBuilder {
        SdkBuilder::new()
    }

    /// Access the underlying [`AppFacade`] for advanced use cases.
    pub fn facade(&self) -> &dyn AppFacade {
        self.runtime.as_ref()
    }

    /// The workspace root path this SDK instance is bound to.
    pub fn workspace_path(&self) -> &std::path::Path {
        &self.workspace_path
    }

    /// Create a new agent session in the workspace.
    pub async fn create_session(&self) -> SdkResult<SdkSession> {
        SdkSession::create(self.runtime.clone(), &self.workspace_path, &self.hooks).await
    }

    /// Create a session with a specific profile alias override.
    pub async fn create_session_with_profile(&self, profile_alias: &str) -> SdkResult<SdkSession> {
        SdkSession::create_with_profile(
            self.runtime.clone(),
            &self.workspace_path,
            &self.hooks,
            profile_alias,
        )
        .await
    }

    /// List existing sessions in the workspace.
    pub async fn list_sessions(&self) -> SdkResult<Vec<agent_core::facade::SessionMeta>> {
        let workspace = self
            .runtime
            .open_workspace(self.workspace_path.display().to_string())
            .await?;
        let sessions = self.runtime.list_sessions(&workspace.workspace_id).await?;
        Ok(sessions)
    }
}
