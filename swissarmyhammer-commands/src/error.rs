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

    /// The clipboard's source entity (the thing being pasted) no longer
    /// exists on the board.
    ///
    /// Raised by paste handlers when the entity referenced by the
    /// clipboard payload — typically a tag or actor — has been deleted
    /// between the time the user copied/cut it and the time they pasted.
    /// The message must name the missing entity (e.g. `"tag 'urgent' no
    /// longer exists"`) so the toast surfaces what specifically failed
    /// rather than a generic "paste failed".
    #[error("source entity missing: {0}")]
    SourceEntityMissing(String),

    /// The paste target (column, board, task, or project) is not a
    /// valid destination for the clipboard contents.
    ///
    /// Raised by paste handlers when the moniker resolved by the
    /// dispatcher names an entity that no longer exists, or when the
    /// destination cannot accept the clipboard's entity type (e.g.
    /// pasting a task into a board that has zero columns). The message
    /// must name the offending destination (e.g. `"column 'doing' does
    /// not exist on this board"`) so the toast names the specific
    /// failure instead of a generic "paste failed".
    #[error("destination invalid: {0}")]
    DestinationInvalid(String),

    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CommandError>;
