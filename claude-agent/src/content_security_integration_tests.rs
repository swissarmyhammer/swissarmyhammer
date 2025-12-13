use crate::base64_processor::Base64Processor;
use crate::content_block_processor::ContentBlockProcessor;
use crate::content_security_validator::ContentSecurityValidator;
use agent_client_protocol::{ContentBlock, ImageContent, ResourceLink, TextContent};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    // Test data constants
    const VALID_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
    const VALID_WAV_BASE64: &str = "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAAA";
    const MALICIOUS_PE_BASE64: &str =
        "TVqQAAMAAAAEAAAA//8AALgAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    // Helper functions to create content blocks
    fn image(mime_type: &str, data: &str) -> ContentBlock {
        ContentBlock::Image(ImageContent {
            data: data.to_string(),
            mime_type: mime_type.to_string(),
            uri: None,
            annotations: None,
            meta: None,
        })
    }

    fn audio(mime_type: &str, data: &str) -> ContentBlock {
        ContentBlock::Audio(agent_client_protocol::AudioContent {
            data: data.to_string(),
            mime_type: mime_type.to_string(),
            annotations: None,
            meta: None,
        })
    }

    fn audio_wav() -> ContentBlock {
        audio("audio/wav", VALID_WAV_BASE64)
    }

    /// Create a ContentBlockProcessor with strict security validation
    fn create_strict_secure_processor() -> ContentBlockProcessor {
        let security_validator = ContentSecurityValidator::strict().unwrap();
        let base64_processor = Base64Processor::with_enhanced_security(
            1024 * 1024, // 1MB limit for strict mode
            security_validator.clone(),
        );

        let mut supported_capabilities = HashMap::new();
        supported_capabilities.insert("text".to_string(), true);
        supported_capabilities.insert("image".to_string(), true);
        supported_capabilities.insert("audio".to_string(), true);
        supported_capabilities.insert("resource".to_string(), true);
        supported_capabilities.insert("resource_link".to_string(), true);

        let config = crate::content_block_processor::EnhancedSecurityConfig {
            max_resource_size: 5 * 1024 * 1024, // 5MB resource limit
            enable_uri_validation: true,
            enable_capability_validation: true,
            supported_capabilities,
            enable_batch_recovery: true,
            content_security_validator: security_validator,
        };

        ContentBlockProcessor::with_enhanced_security_config(base64_processor, config)
    }

    /// Create a ContentBlockProcessor with moderate security validation
    fn create_moderate_secure_processor() -> ContentBlockProcessor {
        let security_validator = ContentSecurityValidator::moderate().unwrap();
        let base64_processor = Base64Processor::with_enhanced_security(
            10 * 1024 * 1024, // 10MB limit for moderate mode
            security_validator.clone(),
        );

        let mut supported_capabilities = HashMap::new();
        supported_capabilities.insert("text".to_string(), true);
        supported_capabilities.insert("image".to_string(), true);
        supported_capabilities.insert("audio".to_string(), true);
        supported_capabilities.insert("resource".to_string(), true);
        supported_capabilities.insert("resource_link".to_string(), true);

        let config = crate::content_block_processor::EnhancedSecurityConfig {
            max_resource_size: 50 * 1024 * 1024, // 50MB resource limit
            enable_uri_validation: true,
            enable_capability_validation: true,
            supported_capabilities,
            enable_batch_recovery: true,
            content_security_validator: security_validator,
        };

        ContentBlockProcessor::with_enhanced_security_config(base64_processor, config)
    }

    /// Create a ContentBlockProcessor with permissive security validation
    fn create_permissive_secure_processor() -> ContentBlockProcessor {
        let security_validator = ContentSecurityValidator::permissive().unwrap();
        let base64_processor = Base64Processor::with_enhanced_security(
            100 * 1024 * 1024, // 100MB limit for permissive mode
            security_validator.clone(),
        );

        let mut supported_capabilities = HashMap::new();
        supported_capabilities.insert("text".to_string(), true);
        supported_capabilities.insert("image".to_string(), true);
        supported_capabilities.insert("audio".to_string(), true);
        supported_capabilities.insert("resource".to_string(), true);
        supported_capabilities.insert("resource_link".to_string(), true);

        let config = crate::content_block_processor::EnhancedSecurityConfig {
            max_resource_size: 500 * 1024 * 1024, // 500MB resource limit
            enable_uri_validation: false,         // disable URI validation for permissive mode
            enable_capability_validation: true,
            supported_capabilities,
            enable_batch_recovery: true,
            content_security_validator: security_validator,
        };

        ContentBlockProcessor::with_enhanced_security_config(base64_processor, config)
    }

    #[test]
    fn test_security_policy_levels() {
        let _strict_processor = create_strict_secure_processor();
        let _moderate_processor = create_moderate_secure_processor();
        let _permissive_processor = create_permissive_secure_processor();

        // Test that processors were created successfully without panicking
        // The fact that we reach this point means all three processors were created successfully
    }

    #[test]
    fn test_safe_text_content_processing() {
        let processor = create_moderate_secure_processor();

        let safe_text = ContentBlock::Text(TextContent {
            text: "This is completely safe text content.".to_string(),
            annotations: None,
            meta: None,
        });

        let result = processor.process_content_block(&safe_text);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert_eq!(
            processed.text_representation,
            "This is completely safe text content."
        );
    }

    #[test]
    fn test_malicious_text_content_blocking() {
        let processor = create_strict_secure_processor();

        let dangerous_text = ContentBlock::Text(TextContent {
            text: "<script>alert('XSS attack');</script>".to_string(),
            annotations: None,
            meta: None,
        });

        let result = processor.process_content_block(&dangerous_text);
        assert!(result.is_err());

        // Verify it's a security-related error
        match result.unwrap_err() {
            crate::content_block_processor::ContentBlockProcessorError::ContentSecurityValidationFailed(_) => {
                // Expected security error
            },
            other => panic!("Expected ContentSecurityValidationFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_safe_image_content_processing() {
        let processor = create_moderate_secure_processor();

        let safe_image = image("image/png", VALID_PNG_BASE64);
        let safe_image = match safe_image {
            ContentBlock::Image(mut img) => {
                img.uri = Some("https://example.com/safe-image.png".to_string());
                ContentBlock::Image(img)
            }
            _ => panic!("Expected image content block"),
        };

        let result = processor.process_content_block(&safe_image);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed
            .text_representation
            .contains("Image content: image/png"));
        assert!(processed.binary_data.is_some());
    }

    #[test]
    fn test_malicious_base64_content_blocking() {
        let processor = create_strict_secure_processor();

        let malicious_image = image("image/png", MALICIOUS_PE_BASE64);

        let result = processor.process_content_block(&malicious_image);
        assert!(result.is_err());

        // Should be blocked by security validation
        match result.unwrap_err() {
            crate::content_block_processor::ContentBlockProcessorError::ContentSecurityValidationFailed(_) => {
                // Expected security error
            },
            other => panic!("Expected ContentSecurityValidationFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_oversized_content_blocking() {
        let processor = create_strict_secure_processor();

        // Create base64 data that exceeds strict mode limits (1MB)
        let oversized_data = "A".repeat(2 * 1024 * 1024); // 2MB of 'A' characters

        let oversized_image = ContentBlock::Image(ImageContent {
            data: oversized_data,
            mime_type: "image/png".to_string(),
            uri: None,
            annotations: None,
            meta: None,
        });

        let result = processor.process_content_block(&oversized_image);
        assert!(result.is_err());

        // Should be blocked by size limits
        match result.unwrap_err() {
            crate::content_block_processor::ContentBlockProcessorError::ContentSecurityValidationFailed(_) => {
                // Expected security error for size limit
            },
            other => panic!("Expected ContentSecurityValidationFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_ssrf_protection_blocking() {
        let processor = create_strict_secure_processor();

        let malicious_resource_link = ContentBlock::ResourceLink(ResourceLink {
            uri: "http://localhost:8080/admin".to_string(), // Should be blocked by SSRF protection
            name: "admin_panel".to_string(),
            description: None,
            mime_type: None,
            title: None,
            size: None,
            annotations: None,
            meta: None,
        });

        let result = processor.process_content_block(&malicious_resource_link);
        assert!(result.is_err());

        // Should be blocked by SSRF protection
        match result.unwrap_err() {
            crate::content_block_processor::ContentBlockProcessorError::ContentSecurityValidationFailed(_) => {
                // Expected security error for SSRF
            },
            other => panic!("Expected ContentSecurityValidationFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_safe_uri_processing() {
        let processor = create_moderate_secure_processor();

        let safe_resource_link = ContentBlock::ResourceLink(ResourceLink {
            uri: "https://example.com/document.pdf".to_string(),
            name: "document".to_string(),
            description: None,
            mime_type: None,
            title: None,
            size: None,
            annotations: None,
            meta: None,
        });

        let result = processor.process_content_block(&safe_resource_link);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed
            .text_representation
            .contains("Resource Link: https://example.com/document.pdf"));
    }

    #[test]
    fn test_content_array_size_limits() {
        let processor = create_strict_secure_processor();

        // Create an array that exceeds strict mode limits (10 items)
        let oversized_array = vec![
            ContentBlock::Text(TextContent {
                text: "Item".to_string(),
                annotations: None,
                meta: None,
            });
            20 // Exceeds strict limit of 10
        ];

        let result = processor.process_content_blocks(&oversized_array);
        assert!(result.is_err());

        // Should be blocked by array size limits
        match result.unwrap_err() {
            crate::content_block_processor::ContentBlockProcessorError::ContentSecurityValidationFailed(_) => {
                // Expected security error for array size
            },
            other => panic!("Expected ContentSecurityValidationFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_safe_content_array_processing() {
        let processor = create_moderate_secure_processor();

        let safe_array = vec![
            ContentBlock::Text(TextContent {
                text: "First item".to_string(),
                annotations: None,
                meta: None,
            }),
            ContentBlock::Text(TextContent {
                text: "Second item".to_string(),
                annotations: None,
                meta: None,
            }),
            ContentBlock::Text(TextContent {
                text: "Third item".to_string(),
                annotations: None,
                meta: None,
            }),
        ];

        let result = processor.process_content_blocks(&safe_array);
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert_eq!(summary.processed_contents.len(), 3);
        assert!(summary.combined_text.contains("First item"));
        assert!(summary.combined_text.contains("Second item"));
        assert!(summary.combined_text.contains("Third item"));
    }

    #[test]
    fn test_invalid_base64_format_blocking() {
        let processor = create_moderate_secure_processor();

        let invalid_image = ContentBlock::Image(ImageContent {
            data: "This is not valid base64!@#$%".to_string(),
            mime_type: "image/png".to_string(),
            uri: None,
            annotations: None,
            meta: None,
        });

        let result = processor.process_content_block(&invalid_image);
        assert!(result.is_err());

        // Should be blocked by base64 format validation
        match result.unwrap_err() {
            crate::content_block_processor::ContentBlockProcessorError::ContentSecurityValidationFailed(_) |
            crate::content_block_processor::ContentBlockProcessorError::Base64Error(_) => {
                // Expected validation error
            },
            other => panic!("Expected base64 or security validation error, got {:?}", other),
        }
    }

    #[test]
    fn test_dangerous_uri_schemes_blocking() {
        let processor = create_strict_secure_processor();

        let dangerous_resource = ContentBlock::ResourceLink(ResourceLink {
            uri: "javascript:alert('XSS')".to_string(), // Dangerous scheme
            name: "dangerous".to_string(),
            description: None,
            mime_type: None,
            title: None,
            size: None,
            annotations: None,
            meta: None,
        });

        let result = processor.process_content_block(&dangerous_resource);
        assert!(result.is_err());

        // Should be blocked by URI scheme validation
        match result.unwrap_err() {
            crate::content_block_processor::ContentBlockProcessorError::ContentSecurityValidationFailed(_) => {
                // Expected security error for dangerous URI scheme
            },
            other => panic!("Expected ContentSecurityValidationFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_audio_content_security_validation() {
        let processor = create_moderate_secure_processor();

        let safe_audio = audio_wav();

        let result = processor.process_content_block(&safe_audio);
        assert!(result.is_ok());

        let processed = result.unwrap();
        assert!(processed
            .text_representation
            .contains("Audio content: audio/wav"));
        assert!(processed.binary_data.is_some());
    }

    #[test]
    fn test_permissive_mode_allows_more_content() {
        let strict_processor = create_strict_secure_processor();
        let permissive_processor = create_permissive_secure_processor();

        // Content that should be allowed in permissive but blocked in strict
        let localhost_resource = ContentBlock::ResourceLink(ResourceLink {
            uri: "http://localhost:3000/api".to_string(),
            name: "local_api".to_string(),
            description: None,
            mime_type: None,
            title: None,
            size: None,
            annotations: None,
            meta: None,
        });

        let strict_result = strict_processor.process_content_block(&localhost_resource);
        let permissive_result = permissive_processor.process_content_block(&localhost_resource);

        // Strict should block it
        assert!(strict_result.is_err());

        // Permissive should allow it (SSRF protection disabled)
        assert!(permissive_result.is_ok());
    }

    #[test]
    fn test_security_validation_preserves_functionality() {
        let secure_processor = create_moderate_secure_processor();

        // Test that normal, safe content processing still works correctly
        let mixed_content = vec![
            ContentBlock::Text(TextContent {
                text: "Safe text content".to_string(),
                annotations: None,
                meta: None,
            }),
            ContentBlock::ResourceLink(ResourceLink {
                uri: "https://example.com/safe-resource".to_string(),
                name: "safe_resource".to_string(),
                description: None,
                mime_type: None,
                title: None,
                size: None,
                annotations: None,
                meta: None,
            }),
        ];

        let result = secure_processor.process_content_blocks(&mixed_content);
        assert!(result.is_ok());

        let summary = result.unwrap();
        assert_eq!(summary.processed_contents.len(), 2);
        assert!(summary.combined_text.contains("Safe text content"));
        assert!(summary
            .combined_text
            .contains("Resource Link: https://example.com/safe-resource"));

        // Verify content type counts
        assert_eq!(summary.content_type_counts.get("text"), Some(&1));
        assert_eq!(summary.content_type_counts.get("resource_link"), Some(&1));
    }

    #[test]
    fn test_error_context_preservation() {
        let processor = create_strict_secure_processor();

        let malicious_text = ContentBlock::Text(TextContent {
            text: "javascript:alert('test')".to_string(),
            annotations: None,
            meta: None,
        });

        let result = processor.process_content_block(&malicious_text);
        assert!(result.is_err());

        let error = result.unwrap_err();

        // Verify the error contains meaningful information
        let error_string = format!("{}", error);
        assert!(
            error_string.contains("Content security validation failed")
                || error_string.contains("security")
                || error_string.contains("validation")
        );
    }

    #[test]
    fn test_performance_with_security_validation() {
        use std::time::Instant;

        let processor = create_moderate_secure_processor();

        let test_content = ContentBlock::Text(TextContent {
            text: "Performance test content".to_string(),
            annotations: None,
            meta: None,
        });

        // Measure processing time
        let start = Instant::now();
        for _ in 0..100 {
            let _ = processor.process_content_block(&test_content);
        }
        let duration = start.elapsed();

        // Security validation should not add significant overhead
        // This is a basic performance sanity check
        assert!(
            duration.as_millis() < 1000,
            "Security validation added too much overhead: {}ms",
            duration.as_millis()
        );
    }
}
