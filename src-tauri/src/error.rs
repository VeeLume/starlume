//! Shared error type for the IPC boundary. Serializable so the frontend gets
//! a typed `{ kind, message }` instead of an opaque string.

#[derive(Debug, Clone, serde::Serialize, specta::Type, thiserror::Error)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    /// Something the user can fix in Settings (missing server URL, …).
    #[error("{0}")]
    Config(String),
    /// Auth / token problems (keyring access, bad callback, …).
    #[error("{0}")]
    Auth(String),
    /// Everything else — bugs and IO surprises.
    #[error("{0}")]
    Internal(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(format!("{e:#}"))
    }
}
