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
    /// The named MCP server was never registered.
    ///
    /// Raised when a call names a server the registry has no record of —
    /// neither live nor disposed. A server that *was* registered and has since
    /// been disposed surfaces as [`ServerUnavailable`] instead, so this variant
    /// is reserved for a name that never existed.
    ///
    /// [`ServerUnavailable`]: Self::ServerUnavailable
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
    ///
    /// Raised when a call names a server that *was* registered but has since
    /// been disposed — its backing plugin was unloaded, or the plugin
    /// unregistered it. This is deliberately distinct from [`UnknownServer`]:
    /// it lets a consumer tell "the server I was using was disposed out from
    /// under me" apart from "I named a server that never existed".
    ///
    /// [`UnknownServer`]: Self::UnknownServer
    #[error("server is unavailable")]
    ServerUnavailable,

    /// The plugin backing this server was reloaded; the request must be retried.
    #[error("plugin was reloaded; retry the request")]
    PluginReloaded,

    /// A registered server accepted the call but its handler reported a failure.
    ///
    /// Carries the JSON-RPC error code and message the handler returned (for
    /// example `-32602` invalid params or `-32603` internal error). This is
    /// deliberately distinct from [`ServerUnavailable`]: the server *is*
    /// serving — it ran the request and returned an error — so flattening it to
    /// "server is unavailable" would hide the actual fault. Preserving the code
    /// and message keeps a failing tool call legible all the way back to the
    /// caller.
    ///
    /// [`ServerUnavailable`]: Self::ServerUnavailable
    #[error("call failed (code {code}): {message}")]
    CallFailed {
        /// The JSON-RPC error code the handler reported.
        code: i32,
        /// The human-readable error message the handler reported.
        message: String,
    },

    /// No plugin is loaded under the named plugin id.
    ///
    /// Raised when an operation — such as [`unload`] — names a plugin that the
    /// host never loaded, or has already unloaded. Distinct from
    /// [`UnknownServer`], which names a missing *server* rather than a missing
    /// *plugin*.
    ///
    /// [`unload`]: crate::PluginHost::unload
    /// [`UnknownServer`]: Self::UnknownServer
    #[error("unknown plugin")]
    UnknownPlugin,

    /// A plugin runtime could not be started.
    ///
    /// Raised when the dedicated worker thread or its supporting Tokio runtime
    /// cannot be created.
    #[error("failed to start plugin runtime: {0}")]
    RuntimeStartup(String),

    /// The plugin runtime's worker thread is no longer running.
    ///
    /// Raised when a command is sent to a runtime whose worker has stopped, or
    /// when that worker panicked during teardown.
    #[error("plugin runtime has stopped")]
    RuntimeStopped,

    /// A plugin runtime command exceeded its time budget.
    ///
    /// Raised when the worker does not answer within the command timeout —
    /// typically a plugin stuck in an infinite loop or an unsettled promise.
    #[error("plugin runtime command timed out")]
    RuntimeTimeout,

    /// JavaScript executing in a plugin isolate failed.
    ///
    /// Carries the V8 exception message: a thrown error, a syntax error in an
    /// evaluated snippet, or a missing or non-callable lifecycle export.
    #[error("plugin runtime error: {0}")]
    Runtime(String),

    /// TypeScript source could not be transpiled to JavaScript.
    ///
    /// Raised for a genuine *syntax* error in a `.ts` module. Type errors are
    /// not transpilation failures: a type-incorrect but syntactically valid
    /// module transpiles cleanly.
    #[error("plugin transpilation error: {0}")]
    Transpile(String),

    /// A plugin bundle could not be resolved or its entry module could not be
    /// located.
    ///
    /// Raised when a bundle directory has no `index.ts` or `index.js` entry,
    /// or when the entry path cannot be canonicalized or escapes the bundle
    /// directory. The message names the offending plugin bundle and the
    /// precise problem, so a broken bundle fails loudly at discovery rather
    /// than mid-load.
    #[error("plugin bundle error: {0}")]
    BundleError(String),
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
