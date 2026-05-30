use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid state: {0}")]
    InvalidState(String),

    #[error("session {session_id} is busy: {reason}")]
    SessionBusy { session_id: String, reason: String },
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
