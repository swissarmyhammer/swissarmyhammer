use llama_common::error::{ErrorCategory, LlamaError};
use thiserror::Error;

/// Errors that can occur during ANE embedding operations
#[derive(Error, Debug)]
pub enum EmbeddingError {
    /// Error from the model loader
    #[error("Model loading error: {0}")]
    ModelLoader(#[from] model_loader::ModelError),

    /// Error from CoreML runtime
    #[error("CoreML error: {0}")]
    CoreML(String),

    /// Error during tokenization
    #[error("Tokenization error: {0}")]
    Tokenization(String),

    /// Error during text processing
    #[error("Text processing error: {0}")]
    TextProcessing(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Error when model is not loaded
    #[error("Model not loaded - call load() first")]
    ModelNotLoaded,

    /// Error when embedding dimensions don't match expectations
    #[error("Embedding dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

impl EmbeddingError {
    pub fn coreml<S: Into<String>>(message: S) -> Self {
        Self::CoreML(message.into())
    }

    pub fn tokenization<S: Into<String>>(message: S) -> Self {
        Self::Tokenization(message.into())
    }

    pub fn text_processing<S: Into<String>>(message: S) -> Self {
        Self::TextProcessing(message.into())
    }

    pub fn configuration<S: Into<String>>(message: S) -> Self {
        Self::Configuration(message.into())
    }
}

impl LlamaError for EmbeddingError {
    fn category(&self) -> ErrorCategory {
        match self {
            EmbeddingError::ModelLoader(e) => e.category(),
            EmbeddingError::CoreML(_) => ErrorCategory::System,
            EmbeddingError::Tokenization(_) => ErrorCategory::User,
            EmbeddingError::TextProcessing(_) => ErrorCategory::System,
            EmbeddingError::Configuration(_) => ErrorCategory::User,
            EmbeddingError::Io(_) => ErrorCategory::System,
            EmbeddingError::ModelNotLoaded => ErrorCategory::User,
            EmbeddingError::DimensionMismatch { .. } => ErrorCategory::User,
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            EmbeddingError::ModelLoader(_) => "ANE_EMBEDDING_MODEL_LOADER",
            EmbeddingError::CoreML(_) => "ANE_EMBEDDING_COREML",
            EmbeddingError::Tokenization(_) => "ANE_EMBEDDING_TOKENIZATION",
            EmbeddingError::TextProcessing(_) => "ANE_EMBEDDING_TEXT_PROCESSING",
            EmbeddingError::Configuration(_) => "ANE_EMBEDDING_CONFIGURATION",
            EmbeddingError::Io(_) => "ANE_EMBEDDING_IO",
            EmbeddingError::ModelNotLoaded => "ANE_EMBEDDING_MODEL_NOT_LOADED",
            EmbeddingError::DimensionMismatch { .. } => "ANE_EMBEDDING_DIMENSION_MISMATCH",
        }
    }

    fn user_friendly_message(&self) -> String {
        match self {
            EmbeddingError::ModelLoader(e) => {
                format!("Model loader: {}", e.user_friendly_message())
            }
            EmbeddingError::CoreML(msg) => format!("CoreML error: {msg}"),
            EmbeddingError::Tokenization(msg) => format!("Tokenization error: {msg}"),
            EmbeddingError::TextProcessing(msg) => format!("Text processing error: {msg}"),
            EmbeddingError::Configuration(msg) => format!("Configuration error: {msg}"),
            EmbeddingError::Io(e) => format!("IO error: {e}"),
            EmbeddingError::ModelNotLoaded => "Model not loaded - call load() first".to_string(),
            EmbeddingError::DimensionMismatch { expected, actual } => {
                format!("Dimension mismatch: expected {expected}, got {actual}")
            }
        }
    }
}

/// Convenience conversion from model-embedding's EmbeddingError
impl From<EmbeddingError> for model_embedding::EmbeddingError {
    fn from(e: EmbeddingError) -> Self {
        match e {
            EmbeddingError::ModelNotLoaded => model_embedding::EmbeddingError::ModelNotLoaded,
            EmbeddingError::Configuration(msg) => {
                model_embedding::EmbeddingError::Configuration(msg)
            }
            EmbeddingError::Io(e) => model_embedding::EmbeddingError::Io(e),
            EmbeddingError::DimensionMismatch { expected, actual } => {
                model_embedding::EmbeddingError::DimensionMismatch { expected, actual }
            }
            other => model_embedding::EmbeddingError::Backend(Box::new(other)),
        }
    }
}

pub type Result<T> = std::result::Result<T, EmbeddingError>;

#[cfg(test)]
mod tests {
    use super::*;

    // ── Constructor helpers ──────────────────────────────────────────

    #[test]
    fn test_error_creation() {
        let e = EmbeddingError::coreml("session failed");
        assert!(matches!(e, EmbeddingError::CoreML(_)));
        assert_eq!(e.to_string(), "CoreML error: session failed");

        let e = EmbeddingError::tokenization("bad input");
        assert!(matches!(e, EmbeddingError::Tokenization(_)));

        let e = EmbeddingError::ModelNotLoaded;
        assert_eq!(e.to_string(), "Model not loaded - call load() first");
    }

    #[test]
    fn test_text_processing_constructor() {
        let e = EmbeddingError::text_processing("truncated output");
        assert!(matches!(e, EmbeddingError::TextProcessing(_)));
        assert_eq!(e.to_string(), "Text processing error: truncated output");
    }

    #[test]
    fn test_configuration_constructor() {
        let e = EmbeddingError::configuration("bad seq_length");
        assert!(matches!(e, EmbeddingError::Configuration(_)));
        assert_eq!(e.to_string(), "Configuration error: bad seq_length");
    }

    #[test]
    fn test_constructors_accept_string_and_str() {
        // &str
        let _ = EmbeddingError::coreml("str input");
        let _ = EmbeddingError::tokenization("str input");
        let _ = EmbeddingError::text_processing("str input");
        let _ = EmbeddingError::configuration("str input");

        // String
        let _ = EmbeddingError::coreml(String::from("String input"));
        let _ = EmbeddingError::tokenization(String::from("String input"));
        let _ = EmbeddingError::text_processing(String::from("String input"));
        let _ = EmbeddingError::configuration(String::from("String input"));
    }

    // ── Display (thiserror) ──────────────────────────────────────────

    #[test]
    fn test_display_all_variants() {
        assert_eq!(
            EmbeddingError::CoreML("boom".into()).to_string(),
            "CoreML error: boom"
        );
        assert_eq!(
            EmbeddingError::Tokenization("bad".into()).to_string(),
            "Tokenization error: bad"
        );
        assert_eq!(
            EmbeddingError::TextProcessing("fail".into()).to_string(),
            "Text processing error: fail"
        );
        assert_eq!(
            EmbeddingError::Configuration("wrong".into()).to_string(),
            "Configuration error: wrong"
        );
        assert_eq!(
            EmbeddingError::ModelNotLoaded.to_string(),
            "Model not loaded - call load() first"
        );
        assert_eq!(
            EmbeddingError::DimensionMismatch {
                expected: 384,
                actual: 768,
            }
            .to_string(),
            "Embedding dimension mismatch: expected 384, got 768"
        );

        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let e = EmbeddingError::Io(io_err);
        assert!(e.to_string().starts_with("IO error:"));
    }

    #[test]
    fn test_display_model_loader_variant() {
        let loader_err = model_loader::ModelError::NotFound("missing.gguf".into());
        let e = EmbeddingError::ModelLoader(loader_err);
        assert!(
            e.to_string().contains("missing.gguf"),
            "ModelLoader display should propagate inner message"
        );
    }

    // ── From impls ───────────────────────────────────────────────────

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let e: EmbeddingError = io_err.into();
        assert!(matches!(e, EmbeddingError::Io(_)));
        assert!(e.to_string().contains("access denied"));
    }

    #[test]
    fn test_from_model_loader_error() {
        let loader_err = model_loader::ModelError::LoadingFailed("oom".into());
        let e: EmbeddingError = loader_err.into();
        assert!(matches!(e, EmbeddingError::ModelLoader(_)));
    }

    // ── ErrorCategory (LlamaError::category) ─────────────────────────

    #[test]
    fn test_error_categories() {
        assert_eq!(
            EmbeddingError::ModelNotLoaded.category(),
            ErrorCategory::User
        );
        assert_eq!(
            EmbeddingError::coreml("fail").category(),
            ErrorCategory::System
        );
        assert_eq!(
            EmbeddingError::configuration("bad").category(),
            ErrorCategory::User
        );
    }

    #[test]
    fn test_error_categories_all_variants() {
        // System variants
        assert_eq!(
            EmbeddingError::CoreML("x".into()).category(),
            ErrorCategory::System
        );
        assert_eq!(
            EmbeddingError::TextProcessing("x".into()).category(),
            ErrorCategory::System
        );
        assert_eq!(
            EmbeddingError::Io(std::io::Error::other("x")).category(),
            ErrorCategory::System
        );

        // User variants
        assert_eq!(
            EmbeddingError::Tokenization("x".into()).category(),
            ErrorCategory::User
        );
        assert_eq!(
            EmbeddingError::Configuration("x".into()).category(),
            ErrorCategory::User
        );
        assert_eq!(
            EmbeddingError::ModelNotLoaded.category(),
            ErrorCategory::User
        );
        assert_eq!(
            EmbeddingError::DimensionMismatch {
                expected: 1,
                actual: 2
            }
            .category(),
            ErrorCategory::User
        );

        // ModelLoader delegates to inner error's category
        let loader_err = model_loader::ModelError::NotFound("x".into());
        let expected_cat = loader_err.category();
        let e = EmbeddingError::ModelLoader(loader_err);
        assert_eq!(e.category(), expected_cat);
    }

    // ── error_code ───────────────────────────────────────────────────

    #[test]
    fn test_error_codes() {
        assert_eq!(
            EmbeddingError::coreml("x").error_code(),
            "ANE_EMBEDDING_COREML"
        );
        assert_eq!(
            EmbeddingError::ModelNotLoaded.error_code(),
            "ANE_EMBEDDING_MODEL_NOT_LOADED"
        );
    }

    #[test]
    fn test_error_codes_all_variants() {
        let cases: Vec<(EmbeddingError, &str)> = vec![
            (
                EmbeddingError::ModelLoader(model_loader::ModelError::NotFound("x".into())),
                "ANE_EMBEDDING_MODEL_LOADER",
            ),
            (EmbeddingError::coreml("x"), "ANE_EMBEDDING_COREML"),
            (
                EmbeddingError::tokenization("x"),
                "ANE_EMBEDDING_TOKENIZATION",
            ),
            (
                EmbeddingError::text_processing("x"),
                "ANE_EMBEDDING_TEXT_PROCESSING",
            ),
            (
                EmbeddingError::configuration("x"),
                "ANE_EMBEDDING_CONFIGURATION",
            ),
            (
                EmbeddingError::Io(std::io::Error::other("x")),
                "ANE_EMBEDDING_IO",
            ),
            (
                EmbeddingError::ModelNotLoaded,
                "ANE_EMBEDDING_MODEL_NOT_LOADED",
            ),
            (
                EmbeddingError::DimensionMismatch {
                    expected: 1,
                    actual: 2,
                },
                "ANE_EMBEDDING_DIMENSION_MISMATCH",
            ),
        ];
        for (err, expected_code) in cases {
            assert_eq!(err.error_code(), expected_code, "Failed for: {err}");
        }
    }

    // ── user_friendly_message ────────────────────────────────────────

    #[test]
    fn test_user_friendly_message_all_variants() {
        let e = EmbeddingError::CoreML("gpu timeout".into());
        assert!(e
            .user_friendly_message()
            .contains("CoreML error: gpu timeout"));

        let e = EmbeddingError::Tokenization("invalid utf8".into());
        assert!(e
            .user_friendly_message()
            .contains("Tokenization error: invalid utf8"));

        let e = EmbeddingError::TextProcessing("empty".into());
        assert!(e
            .user_friendly_message()
            .contains("Text processing error: empty"));

        let e = EmbeddingError::Configuration("bad path".into());
        assert!(e
            .user_friendly_message()
            .contains("Configuration error: bad path"));

        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no file");
        let e = EmbeddingError::Io(io_err);
        assert!(e.user_friendly_message().contains("IO error:"));

        let e = EmbeddingError::ModelNotLoaded;
        assert_eq!(
            e.user_friendly_message(),
            "Model not loaded - call load() first"
        );

        let e = EmbeddingError::DimensionMismatch {
            expected: 512,
            actual: 1024,
        };
        let msg = e.user_friendly_message();
        assert!(msg.contains("512"));
        assert!(msg.contains("1024"));
    }

    #[test]
    fn test_user_friendly_message_model_loader_delegates() {
        let loader_err = model_loader::ModelError::NotFound("qwen.mlpackage".into());
        let expected_inner = loader_err.user_friendly_message();
        let e = EmbeddingError::ModelLoader(loader_err);
        let msg = e.user_friendly_message();
        assert!(
            msg.contains(&expected_inner),
            "ModelLoader user_friendly_message should contain inner: got {msg}"
        );
    }

    // ── Conversion to model_embedding::EmbeddingError ────────────────

    #[test]
    fn test_conversion_to_model_embedding_error() {
        let e: model_embedding::EmbeddingError = EmbeddingError::ModelNotLoaded.into();
        assert!(matches!(e, model_embedding::EmbeddingError::ModelNotLoaded));

        let e: model_embedding::EmbeddingError = EmbeddingError::coreml("runtime fail").into();
        assert!(matches!(e, model_embedding::EmbeddingError::Backend(_)));
    }

    #[test]
    fn test_conversion_configuration_preserves_message() {
        let e: model_embedding::EmbeddingError =
            EmbeddingError::configuration("bad batch size").into();
        assert!(matches!(
            e,
            model_embedding::EmbeddingError::Configuration(_)
        ));
        assert!(e.to_string().contains("bad batch size"));
    }

    #[test]
    fn test_conversion_io_preserves_kind() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let e: model_embedding::EmbeddingError = EmbeddingError::Io(io_err).into();
        assert!(matches!(e, model_embedding::EmbeddingError::Io(_)));
    }

    #[test]
    fn test_conversion_dimension_mismatch() {
        let e: model_embedding::EmbeddingError = EmbeddingError::DimensionMismatch {
            expected: 384,
            actual: 768,
        }
        .into();
        match e {
            model_embedding::EmbeddingError::DimensionMismatch { expected, actual } => {
                assert_eq!(expected, 384);
                assert_eq!(actual, 768);
            }
            other => panic!("Expected DimensionMismatch, got: {other}"),
        }
    }

    #[test]
    fn test_conversion_remaining_variants_become_backend() {
        // Tokenization -> Backend
        let e: model_embedding::EmbeddingError = EmbeddingError::tokenization("bad tokens").into();
        assert!(matches!(e, model_embedding::EmbeddingError::Backend(_)));

        // TextProcessing -> Backend
        let e: model_embedding::EmbeddingError =
            EmbeddingError::text_processing("truncated").into();
        assert!(matches!(e, model_embedding::EmbeddingError::Backend(_)));

        // ModelLoader -> Backend
        let loader_err = model_loader::ModelError::LoadingFailed("oom".into());
        let e: model_embedding::EmbeddingError = EmbeddingError::ModelLoader(loader_err).into();
        assert!(matches!(e, model_embedding::EmbeddingError::Backend(_)));
    }

    // ── Debug impl ───────────────────────────────────────────────────

    #[test]
    fn test_debug_impl() {
        let e = EmbeddingError::CoreML("test".into());
        let debug = format!("{e:?}");
        assert!(
            debug.contains("CoreML"),
            "Debug output should contain variant name"
        );
    }

    // ── Result type alias ────────────────────────────────────────────

    #[test]
    fn test_result_type_alias() {
        fn returns_ok() -> super::Result<u32> {
            Ok(42)
        }
        fn returns_err() -> super::Result<u32> {
            Err(EmbeddingError::ModelNotLoaded)
        }
        assert_eq!(returns_ok().unwrap(), 42);
        assert!(returns_err().is_err());
    }
}
