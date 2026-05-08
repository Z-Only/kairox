use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid state: {0}")]
    InvalidState(String),

    #[error("session {session_id} is busy: {reason}")]
    SessionBusy { session_id: String, reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_busy_displays_with_session_and_reason() {
        let err = CoreError::SessionBusy {
            session_id: "ses_abc".into(),
            reason: "compacting".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("ses_abc"), "expected session id, got: {msg}");
        assert!(msg.contains("compacting"), "expected reason, got: {msg}");
    }
}
