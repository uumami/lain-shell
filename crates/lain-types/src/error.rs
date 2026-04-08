#[derive(Debug, thiserror::Error, serde::Serialize, serde::Deserialize, Clone)]
pub enum LainError {
    #[error("not found: {resource} {id}")]
    NotFound { resource: String, id: String },

    #[error("permission denied: {reason}")]
    PermissionDenied { reason: String },

    #[error("invalid config: {detail}")]
    InvalidConfig { detail: String },

    #[error("isolation error: {detail}")]
    IsolationError { detail: String },

    #[error("pty error: {detail}")]
    PtyError { detail: String },

    #[error("bus error: {detail}")]
    BusError { detail: String },

    #[error("internal: {message}")]
    Internal { message: String },
}
