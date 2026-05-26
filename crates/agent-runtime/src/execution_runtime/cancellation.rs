use std::collections::HashMap;
use std::sync::Arc;

use agent_core::SessionId;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Default)]
pub(crate) struct CancellationRegistry {
    tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl CancellationRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn register(&self, session_id: &SessionId, token: CancellationToken) {
        self.tokens
            .lock()
            .await
            .insert(session_id.to_string(), token);
    }

    pub(crate) async fn cancel(&self, session_id: &SessionId) -> bool {
        let token = self.tokens.lock().await.remove(session_id.as_str());
        if let Some(token) = token {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub(crate) async fn unregister(&self, session_id: &SessionId) {
        self.tokens.lock().await.remove(session_id.as_str());
    }
}
