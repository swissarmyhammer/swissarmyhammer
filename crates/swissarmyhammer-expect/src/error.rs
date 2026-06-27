//! The error type for the `expect` engine.

use thiserror::Error;

/// The crate's error type.
#[derive(Debug, Error)]
pub enum ExpectError {
    /// IO error during file operations (reading specs, writing observations).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing or serialization error (golden/received wire forms).
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// TOML parsing error (the `.expect/config.toml` repo-level config).
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    /// An expectation, addressed by its repo-relative path, is malformed or
    /// could not be processed.
    #[error("expectation '{path}' error: {message}")]
    Expectation {
        /// The expectation's repo-relative path (its identity).
        path: String,
        /// What went wrong.
        message: String,
    },

    /// A surface adapter could not provision, drive, or observe the system
    /// under test (e.g. a failed build step, an undetectable project type, or
    /// an empty command).
    #[error("surface error: {0}")]
    Surface(String),

    /// A driven run exceeded its wall-clock budget and was aborted, rather than
    /// allowed to hang.
    #[error("run timed out after {timeout_ms}ms")]
    Timeout {
        /// The exceeded budget, in milliseconds.
        timeout_ms: u64,
    },

    /// The ACP connection to the driving agent could not be stood up, or the
    /// driving pipeline failed over it (see [`crate::drive`]).
    #[error("agent connection error: {0}")]
    Agent(String),

    /// The agent pool abandoned a driven prompt turn: its liveness supervisor
    /// tripped the idle window or the absolute ceiling and cancelled the
    /// session, rather than letting the turn hang. Kept typed (rather than
    /// folded into [`ExpectError::Agent`]) so a wedged-then-abandoned turn is
    /// distinguishable from a genuine agent failure without parsing message
    /// text — the deterministic stall floor `expect` reuses from the pool.
    #[error(transparent)]
    Pool(#[from] swissarmyhammer_validators::PoolError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expectation_error_displays_path_and_message() {
        let err = ExpectError::Expectation {
            path: "src/checkout/coupon".to_string(),
            message: "no acceptance criteria".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "expectation 'src/checkout/coupon' error: no acceptance criteria"
        );
    }

    #[test]
    fn json_error_converts_from_serde_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err: ExpectError = json_err.into();
        assert!(err.to_string().contains("JSON error"));
    }
}
