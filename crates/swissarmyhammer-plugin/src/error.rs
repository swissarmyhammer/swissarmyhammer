//! Error types for the plugin platform.
//!
//! Every fallible operation in this crate surfaces failures through the
//! [`Error`] enum and the [`Result`] alias defined here.

use thiserror::Error;

/// Errors produced by the plugin platform.
///
/// These variants cover the failure modes of registering MCP servers and
/// dispatching operations to them: a caller naming a server, tool, or
/// operation that does not exist, a name collision at registration time,
/// a server that is registered but currently not serving requests, and the
/// transient case where a plugin was reloaded out from under an in-flight
/// dispatch.
#[derive(Debug, Error)]
pub enum Error {
    /// The named MCP server is not present in the registry.
    #[error("unknown server")]
    UnknownServer,

    /// The requested tool is not exposed by the target server.
    #[error("unknown tool")]
    UnknownTool,

    /// The requested operation is not recognized by the target tool.
    #[error("unknown operation")]
    UnknownOperation,

    /// A server with this name is already registered; names must be unique.
    #[error("server name '{0}' is already taken")]
    ServerNameTaken(String),

    /// The server is registered but is currently not able to serve requests.
    #[error("server is unavailable")]
    ServerUnavailable,

    /// The plugin backing this server was reloaded; the request must be retried.
    #[error("plugin was reloaded; retry the request")]
    PluginReloaded,
}

/// Convenience alias for results produced by the plugin platform.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_name_taken_displays_non_empty_message() {
        let error = Error::ServerNameTaken("x".into());
        let message = error.to_string();
        assert!(
            !message.is_empty(),
            "ServerNameTaken should Display a non-empty message"
        );
    }
}
