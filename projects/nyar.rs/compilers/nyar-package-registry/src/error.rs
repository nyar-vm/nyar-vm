/// Registry-layer errors.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// HTTP client or transport failure.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// Registry responded with a non-success status code.
    #[error("registry request failed with status {status}: {message}")]
    Status {
        /// HTTP status code returned by the registry.
        status: u16,
        /// Human-readable error body or summary.
        message: String,
    },
    /// JSON serialization or deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Local filesystem operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Requested package or version does not exist.
    #[error("package not found: {0}")]
    NotFound(String),
    /// Registry adapter does not implement the requested operation.
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    /// General registry failure with a caller-provided message.
    #[error("{0}")]
    Message(String),
}

impl RegistryError {
    /// Build a status-backed error from an HTTP response code and message.
    pub fn status(status: u16, message: impl Into<String>) -> Self {
        Self::Status { status, message: message.into() }
    }

    /// Build a generic message-only error.
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    /// Build an unsupported-operation error.
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }
}
