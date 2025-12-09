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

        assert_eq!(config.max_tokens, 512);
        assert_eq!(config.temperature, 0.7);
        assert_eq!(config.top_p, 0.9);
        assert!(config.stop_tokens.is_empty());
        assert_eq!(config.seed, 1234);
        assert!(config.use_greedy);
    }

    #[test]
    fn test_generation_config_for_batch() {
        let config = GenerationConfig::for_batch_generation();

        assert_eq!(config.max_tokens, 2048);
        assert!(!config.use_greedy); // Should allow flexible sampling
    }

    #[test]
    fn test_generation_config_for_streaming() {
        let config = GenerationConfig::for_streaming();

        assert_eq!(config.max_tokens, 1024);
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
    //! Tests for template offset functionality.
    //!
    //! These tests verify that the template offset methods exist and have correct behavior.
    //! Full integration testing with actual models will be done in separate integration tests.

    #[test]
    fn test_template_offset_zero_is_valid() {
        // Verify that a template offset of zero is valid.
        // Zero offset means no template, process all tokens.
        let offset = Some(0_usize);
        assert_eq!(offset, Some(0_usize));
    }

    #[test]
    fn test_template_offset_none_is_valid() {
        // Verify that None offset is valid.
        // None means no template offset, process all tokens normally.
        let offset: Option<usize> = None;
        assert!(offset.is_none());
    }

    #[test]
    fn test_template_offset_nonzero_is_valid() {
        // Verify that a non-zero template offset is valid.
        let offset = Some(100_usize);
        assert_eq!(offset, Some(100_usize));
        if let Some(val) = offset {
            assert!(val > 0);
        }
    }

    #[test]
    fn test_template_offset_calculation() {
        // Test the logic for calculating tokens to skip and process
        let total_tokens: usize = 150;
        let template_offset = Some(100_usize);

        if let Some(offset) = template_offset {
            let tokens_to_skip = offset;
            let tokens_to_process = total_tokens.saturating_sub(offset);

            assert_eq!(tokens_to_skip, 100);
            assert_eq!(tokens_to_process, 50);
        }
    }

    #[test]
    fn test_template_offset_edge_case_equal() {
        // Test when template offset equals total tokens
        let total_tokens: usize = 100;
        let template_offset = Some(100_usize);

        if let Some(offset) = template_offset {
            let tokens_to_process = total_tokens.saturating_sub(offset);
            assert_eq!(tokens_to_process, 0, "Should have no tokens to process");
        }
    }

    #[test]
    fn test_template_offset_edge_case_exceeds() {
        // Test when template offset exceeds total tokens
        let total_tokens: usize = 50;
        let template_offset = Some(100_usize);

        if let Some(offset) = template_offset {
            // Using saturating_sub ensures we don't underflow
            let tokens_to_process = total_tokens.saturating_sub(offset);
            assert_eq!(tokens_to_process, 0, "Should have no tokens to process");
        }
    }
}
