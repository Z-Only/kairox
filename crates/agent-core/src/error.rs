use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid state: {0}")]
    InvalidState(String),
}
