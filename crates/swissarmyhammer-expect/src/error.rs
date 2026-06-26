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
