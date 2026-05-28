//! Tests for generation configuration validation.
//!
//! These tests focus on GenerationConfig validation and related functionality
//! without requiring model loading or mock implementations.

use super::*;

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_generation_config_default() {
        let config = GenerationConfig::default();

        assert_eq!(config.max_tokens, 4096);
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.top_p, 0.9);
        assert!(config.stop_tokens.is_empty());
        assert_eq!(config.seed, 1234);
        assert!(config.use_greedy);
    }

    #[test]
    fn test_generation_config_for_batch() {
        let config = GenerationConfig::for_batch_generation();

        assert_eq!(config.max_tokens, 4096);
        assert!(!config.use_greedy); // Should allow flexible sampling
    }

    #[test]
    fn test_generation_config_for_streaming() {
        let config = GenerationConfig::for_streaming();

        assert_eq!(config.max_tokens, 4096);
        assert!(!config.use_greedy); // Should allow flexible sampling
    }

    #[test]
    fn test_generation_config_for_compaction() {
        let config = GenerationConfig::for_compaction();

        assert_eq!(config.max_tokens, 512);
        assert_eq!(config.temperature, 0.0); // Deterministic
        assert!(config.use_greedy); // Matches existing compaction behavior
    }

    #[test]
    fn test_generation_config_validation_success() {
        let config = GenerationConfig {
            max_tokens: 100,
            temperature: 0.7,
            top_p: 0.9,
            stop_tokens: vec!["stop".to_string()],
            seed: 1234,
            use_greedy: false,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_generation_config_validation_zero_tokens() {
        let config = GenerationConfig {
            max_tokens: 0,
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("max_tokens must be greater than 0"));
    }

    #[test]
    fn test_generation_config_validation_excessive_tokens() {
        let config = GenerationConfig {
            max_tokens: 200_000,
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("cannot exceed 100,000"));
    }

    #[test]
    fn test_generation_config_validation_invalid_temperature() {
        let config = GenerationConfig {
            temperature: 3.0,
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("temperature must be between 0.0 and 2.0"));
    }

    #[test]
    fn test_generation_config_validation_invalid_top_p() {
        let config = GenerationConfig {
            top_p: 1.5,
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("top_p must be between 0.0 and 1.0"));
    }

    #[test]
    fn test_generation_config_validation_too_many_stop_tokens() {
        let config = GenerationConfig {
            stop_tokens: (0..15).map(|i| format!("stop{}", i)).collect(),
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("Cannot specify more than 10 stop tokens"));
    }

    #[test]
    fn test_generation_config_validation_empty_stop_token() {
        let config = GenerationConfig {
            stop_tokens: vec!["".to_string()],
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("Stop tokens cannot be empty"));
    }

    #[test]
    fn test_generation_config_validation_long_stop_token() {
        let config = GenerationConfig {
            stop_tokens: vec!["a".repeat(100)],
            ..Default::default()
        };

        assert!(config.validate().is_err());
        assert!(config
            .validate()
            .unwrap_err()
            .contains("Stop tokens cannot exceed 50 characters"));
    }

    #[test]
    fn test_generation_error_creation() {
        let err = std::io::Error::other("test error");

        let gen_err = GenerationError::tokenization(err);
        assert!(matches!(gen_err, GenerationError::TokenizationFailed(_)));

        let err = std::io::Error::other("batch error");
        let gen_err = GenerationError::batch(err);
        assert!(matches!(gen_err, GenerationError::BatchFailed(_)));

        let err = std::io::Error::other("decode error");
        let gen_err = GenerationError::decoding(err);
        assert!(matches!(gen_err, GenerationError::DecodingFailed(_)));

        let err = std::io::Error::other("conversion error");
        let gen_err = GenerationError::token_conversion(err);
        assert!(matches!(gen_err, GenerationError::TokenConversionFailed(_)));

        let err = std::io::Error::other("context error");
        let gen_err = GenerationError::context(err);
        assert!(matches!(gen_err, GenerationError::ContextFailed(_)));

        let err = std::io::Error::other("generation error");
        let gen_err = GenerationError::generation(err);
        assert!(matches!(gen_err, GenerationError::GenerationFailed(_)));
    }

    #[test]
    fn test_generation_error_from_string() {
        let error_msg = "Configuration error".to_string();
        let gen_err: GenerationError = error_msg.into();

        match gen_err {
            GenerationError::InvalidConfig(msg) => {
                assert_eq!(msg, "Configuration error");
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }
}

#[cfg(test)]
mod template_offset_tests {
    //! Tests for the template-offset decision the production decode loop makes.
    //!
    //! These exercise the real `budget::template_offset_exhausted` predicate that
    //! both offset variants call to decide whether to enter the decode loop or
    //! return an empty response — not inline copies of the arithmetic. The
    //! decode loop itself binds a model and is covered by the small-model
    //! integration tests.

    use crate::generation::budget::template_offset_exhausted;

    #[test]
    fn zero_offset_with_tokens_is_not_exhausted() {
        // Zero offset means "no template" — every token is new, so the loop runs.
        assert!(!template_offset_exhausted(0, 150));
    }

    #[test]
    fn partial_offset_leaves_new_tokens() {
        // 100-token template prefix, 150-token prompt -> 50 new tokens to decode.
        assert!(!template_offset_exhausted(100, 150));
    }

    #[test]
    fn offset_equal_to_total_is_exhausted() {
        // The cached template covers the whole prompt -> nothing new to process.
        assert!(template_offset_exhausted(100, 100));
    }

    #[test]
    fn offset_exceeding_total_is_exhausted_without_underflow() {
        // An offset larger than the prompt must report "exhausted" rather than
        // letting the production `skip(offset)` underflow.
        assert!(template_offset_exhausted(100, 50));
    }
}
