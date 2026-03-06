use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("command not found: {0}")]
    NotFound(String),

    #[error("command not available: {0}")]
    NotAvailable(String),

    #[error("missing required scope: {0}")]
    MissingScope(String),

    #[error("missing required arg: {0}")]
    MissingArg(String),

    #[error("invalid moniker: {0}")]
    InvalidMoniker(String),

    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CommandError>;
