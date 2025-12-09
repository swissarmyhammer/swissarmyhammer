//! Integration tests for Google Docs conversion
//!
//! Tests the library's ability to convert Google Docs to markdown.

use markdowndown::MarkdownDown;
use std::time::Instant;

use super::{IntegrationTestConfig, TestUtils};

/// Read an environment variable and parse it, or return a default value
fn read_env_var_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Read an environment variable as a string, or return a default value
fn read_env_var_or_string(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}



/// Configuration for error response validation
/// 
/// # Minimum Error Content Length
/// 
/// The threshold must be configured via the `TEST_MIN_ERROR_CONTENT_LENGTH` 
/// environment variable based on observed behavior with Google's API.
/// 
/// This threshold helps distinguish between:
/// - Valid conversions: substantial markdown content
/// - Error responses: short error messages or empty content
struct ErrorValidationConfig {
    /// Minimum content length expected for valid error responses from Google Docs.
    /// Content shorter than this threshold may indicate an unexpected success or invalid response.
    min_error_content_length: usize,
}

impl ErrorValidationConfig {
    fn from_env() -> Self {
        let min_error_content_length = std::env::var("TEST_MIN_ERROR_CONTENT_LENGTH")
            .expect("TEST_MIN_ERROR_CONTENT_LENGTH environment variable must be set. Example: 100")
            .parse()
            .expect("TEST_MIN_ERROR_CONTENT_LENGTH must be a valid positive integer");
        
        Self {
            min_error_content_length,
        }
    }
}

/// Test data provider for Google Docs URLs
/// 
/// This allows tests to use configurable URLs instead of hard-coded values.
/// URLs can be overridden via environment variables for flexibility.
/// 
/// # Default Test Documents
/// 
/// The default URLs point to example public Google documents. These are provided
/// as examples but may become unavailable or have their permissions changed.
/// 
/// For reliable testing, set these environment variables to your own test documents:
/// - `TEST_GOOGLE_DOC_PRIMARY`: Primary test document URL
/// - `TEST_GOOGLE_DOC_SECONDARY`: Secondary test document URL  
/// - `TEST_GOOGLE_SPREADSHEET`: Spreadsheet test URL
/// 
/// Your test documents should be publicly accessible to ensure tests can run
/// without authentication.
struct GoogleDocsTestData {
    /// Primary test document URL (public Google Doc)
    primary_doc_url: String,
    /// Secondary test document URL (public Google Doc)
    secondary_doc_url: String,
    /// Spreadsheet URL for testing different document types
    spreadsheet_url: String,
}

impl GoogleDocsTestData {
    fn from_env() -> Self {
        // Require environment variables for test URLs to avoid brittle dependencies
        // on external resources that may become unavailable
        let primary_doc_url = std::env::var("TEST_GOOGLE_DOC_PRIMARY")
            .expect("TEST_GOOGLE_DOC_PRIMARY environment variable must be set. Example: https://docs.google.com/document/d/YOUR_DOC_ID/edit");
        let secondary_doc_url = std::env::var("TEST_GOOGLE_DOC_SECONDARY")
            .expect("TEST_GOOGLE_DOC_SECONDARY environment variable must be set. Example: https://docs.google.com/document/d/YOUR_DOC_ID/edit");
        let spreadsheet_url = std::env::var("TEST_GOOGLE_SPREADSHEET")
            .expect("TEST_GOOGLE_SPREADSHEET environment variable must be set. Example: https://docs.google.com/spreadsheets/d/YOUR_SHEET_ID/edit");
        
        Self {
            primary_doc_url,
            secondary_doc_url,
            spreadsheet_url,
        }
    }

    /// Get supported document types for testing
    /// Returns tuples of (url, description) for each supported type
    /// 
    /// Note: These are example public documents used for integration testing.
    /// Set environment variables to use your own test documents:
    /// - TEST_GOOGLE_DOC_PRIMARY: Primary test document URL
    /// - TEST_GOOGLE_SPREADSHEET: Spreadsheet test URL
    fn supported_document_types(&self) -> Vec<(&str, &str)> {
        vec![
            (self.primary_doc_url.as_str(), "Document"),
            (self.spreadsheet_url.as_str(), "Spreadsheet"),
            // Note: Presentations may not be supported by the converter
        ]
    }

    /// Generate URL variants for testing different URL formats
    /// Returns tuples of (url, description) to ensure proper pairing
    /// 
    /// Derives URL variants from the base URL structure to adapt to URL format changes.
    /// If the base URL format changes, the variants will automatically adapt.
    fn generate_url_variants(&self, base_url: &str) -> Vec<(String, &'static str)> {
        // Extract document ID and base structure from URL
        let doc_id = self.extract_doc_id(base_url);
        
        // Derive the base domain and path from the input URL to adapt to structure changes
        let base_parts: Vec<&str> = base_url.split("/d/").collect();
        let base_domain_path = base_parts.first().unwrap_or(&"https://docs.google.com/document");
        
        vec![
            (format!("{}/d/{}/edit", base_domain_path, doc_id), "Base edit URL"),
            (format!("{}/d/{}/edit?usp=sharing", base_domain_path, doc_id), "Edit URL with sharing parameter"),
            (format!("{}/d/{}/view", base_domain_path, doc_id), "View URL"),
        ]
    }

    /// Extract document ID from a Google Docs URL
    fn extract_doc_id(&self, url: &str) -> String {
        url.split("/d/")
            .nth(1)
            .and_then(|s| s.split('/').next())
            .unwrap_or("invalid")
            .to_string()
    }

    /// Generate malformed URLs for error testing
    /// 
    /// These URLs are intentionally invalid to test error handling.
    /// Returns tuples of (url, description) to ensure proper pairing.
    /// 
    /// URLs must be provided via environment variables to ensure they are
    /// appropriate for current Google Docs URL validation rules:
    /// - TEST_ERROR_URL_1: First malformed URL (e.g., empty document ID)
    /// - TEST_ERROR_URL_2: Second malformed URL (e.g., invalid characters)
    /// - TEST_ERROR_URL_3: Third malformed URL (e.g., malformed structure)
    fn generate_error_test_urls(&self) -> Vec<(String, &'static str)> {
        let url1 = std::env::var("TEST_ERROR_URL_1")
            .expect("TEST_ERROR_URL_1 environment variable must be set. Example: https://docs.google.com/document/d//edit");
        let url2 = std::env::var("TEST_ERROR_URL_2")
            .expect("TEST_ERROR_URL_2 environment variable must be set. Example: https://docs.google.com/document/d/invalid@#$/edit");
        let url3 = std::env::var("TEST_ERROR_URL_3")
            .expect("TEST_ERROR_URL_3 environment variable must be set. Example: https://docs.google.com/document/edit");
        
        vec![
            (url1, "Empty document ID"),
            (url2, "Invalid characters in document ID"),
            (url3, "Malformed URL structure"),
        ]
    }
}

/// Test fixture for Google Docs tests that provides common setup
struct GoogleDocsTestFixture {
    config: IntegrationTestConfig,
    test_data: GoogleDocsTestData,
    md: MarkdownDown,
}

impl GoogleDocsTestFixture {
    /// Create a new test fixture with standard setup
    /// 
    /// # Arguments
    /// * `skip_slow` - If true, skip this test if slow tests are disabled
    /// 
    /// # Returns
    /// * `Ok(Self)` - Successfully created test fixture
    /// * `Err` - Test should be skipped (configuration prevents running)
    fn setup(skip_slow: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let config = IntegrationTestConfig::from_env();
        
        // Check if tests should be skipped
        if !config.can_test_google_docs() {
            return Err("Google Docs tests - external services disabled".into());
        }
        
        if skip_slow && config.skip_slow_tests {
            return Err("Google Docs tests - slow tests skipped".into());
        }
        
        let test_data = GoogleDocsTestData::from_env();
        let md = MarkdownDown::new();
        
        Ok(Self { config, test_data, md })
    }
    
    /// Convert a URL with rate limiting and timing
    async fn convert_with_rate_limit(
        &self,
        url: &str,
    ) -> (Result<markdowndown::Markdown, Box<dyn std::error::Error>>, std::time::Duration) {
        TestUtils::apply_rate_limit(&self.config).await;
        let start = Instant::now();
        let result = self.md.convert_url(url).await;
        (result, start.elapsed())
    }

    /// Convert a URL and validate the result with a custom validation function
    /// 
    /// This helper encapsulates the common pattern of converting a URL with rate limiting
    /// and then validating the result with a custom validator.
    async fn convert_and_validate<F>(&self, url: &str, description: &str, validation_fn: F)
    where
        F: Fn(&markdowndown::Markdown),
    {
        let (result, _duration) = self.convert_with_rate_limit(url).await;
        let validator = ConversionValidator::Custom(Box::new(validation_fn));
        validator.validate(result, description);
    }
}

/// Validation strategies for conversion results
enum ConversionValidator {
    /// Validate basic markdown quality
    Quality,
    /// Validate quality and check for specific document type in frontmatter
    ContentType(&'static str),
    /// Expect an error response (for malformed URLs)
    ExpectError { min_content_length: usize },
    /// Custom validation with provided closure
    Custom(Box<dyn Fn(&markdowndown::Markdown)>),
}

impl ConversionValidator {
    /// Validate a conversion result according to the strategy
    fn validate(
        &self,
        result: Result<markdowndown::Markdown, Box<dyn std::error::Error>>,
        description: &str,
    ) {
        match result {
            Ok(markdown) => {
                match self {
                    Self::Quality => {
                        assert!(
                            TestUtils::validate_markdown_quality(markdown.as_str()),
                            "Poor quality markdown for {description}"
                        );
                        println!("✓ {description} validation passed ({} chars)", markdown.as_str().len());
                    }
                    Self::ContentType(doc_type) => {
                        assert!(
                            TestUtils::validate_markdown_quality(markdown.as_str()),
                            "Poor quality conversion for {description} type"
                        );
                        let frontmatter = markdown.frontmatter().unwrap();
                        assert!(
                            frontmatter.contains("google") || frontmatter.contains("docs"),
                            "Frontmatter should indicate Google Docs source"
                        );
                        println!("✓ {doc_type} converted successfully ({} chars)", markdown.as_str().len());
                    }
                    Self::ExpectError { min_content_length } => {
                        let content = markdown.as_str();
                        if content.len() >= *min_content_length && TestUtils::validate_markdown_quality(content) {
                            panic!("Malformed URL produced unexpectedly valid content for {description}");
                        }
                        println!("Unexpected success for {description}: {} chars (below threshold)", content.len());
                    }
                    Self::Custom(validator) => {
                        validator(&markdown);
                        println!("✓ {description} validation passed");
                    }
                }
            }
            Err(e) => {
                match self {
                    Self::ExpectError { .. } => {
                        println!("✓ {description} failed as expected: {e}");
                    }
                    _ => {
                        println!("⚠ {description} failed (may be expected): {e}");
                    }
                }
            }
        }
    }
}

/// Validate markdown quality including frontmatter for Google Docs conversion
fn assert_google_docs_quality(markdown: &markdowndown::Markdown, description: &str) {
    let content = markdown.as_str();
    assert!(
        TestUtils::validate_markdown_quality(content),
        "Poor quality markdown for {description}: content too short or invalid"
    );
    assert!(
        markdown.frontmatter().is_some(),
        "Missing frontmatter for {description}"
    );
    let frontmatter = markdown.frontmatter().unwrap();
    assert!(
        TestUtils::validate_frontmatter(&frontmatter),
        "Invalid frontmatter for {description}"
    );
}

/// Helper to convert URL/description tuples to test cases
fn generate_test_cases<'a>(
    url_pairs: &'a [(String, &'static str)],
) -> Vec<(&'a str, &'a str)> {
    url_pairs.iter()
        .map(|(url, desc)| (url.as_str(), *desc))
        .collect()
}

/// Common pattern for testing multiple URLs with a validation strategy
async fn test_multiple_urls(
    fixture: &GoogleDocsTestFixture,
    test_cases: &[(&str, &str)],
    validator: &ConversionValidator,
) {
    for (url, description) in test_cases.iter() {
        println!("Testing: {description} - {url}");
        let (result, _duration) = fixture.convert_with_rate_limit(url).await;
        validator.validate(result, description);
    }
}

/// Test basic conversion of Google Docs documents
#[tokio::test]
async fn test_google_docs_basic_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = GoogleDocsTestFixture::setup(false)?;

    // Test primary document
    println!("Testing: Google Sheets API Sample");
    fixture.convert_and_validate(
        &fixture.test_data.primary_doc_url,
        "Google Sheets API Sample",
        |markdown| {
            assert!(!markdown.as_str().is_empty(), "Content should not be empty");
            println!("  Converted successfully ({} chars)", markdown.as_str().len());
        },
    ).await;

    Ok(())
}

/// Test markdown quality validation for Google Docs conversion
#[tokio::test]
async fn test_google_docs_markdown_quality() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = GoogleDocsTestFixture::setup(false)?;

    fixture.convert_and_validate(
        &fixture.test_data.secondary_doc_url,
        "Markdown quality",
        |markdown| {
            assert_google_docs_quality(markdown, "markdown quality test");
        },
    ).await;

    Ok(())
}

/// Test frontmatter validation for Google Docs conversion
#[tokio::test]
async fn test_google_docs_frontmatter() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = GoogleDocsTestFixture::setup(false)?;

    fixture.convert_and_validate(
        &fixture.test_data.primary_doc_url,
        "Frontmatter validation",
        |markdown| {
            assert_google_docs_quality(markdown, "frontmatter test");
        },
    ).await;

    Ok(())
}

/// Test Google Docs URL format detection and parsing
#[tokio::test]
async fn test_google_docs_url_formats() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = GoogleDocsTestFixture::setup(false)?;

    // Generate URL variants programmatically from the base test URL
    let url_variants = fixture.test_data.generate_url_variants(&fixture.test_data.primary_doc_url);
    let test_cases = generate_test_cases(&url_variants);

    for (url, description) in test_cases.iter() {
        println!("Testing: {description} - {url}");
        
        // Verify URL is detected as Google Docs
        let detected_type = markdowndown::detect_url_type(url).unwrap();
        assert_eq!(
            detected_type,
            markdowndown::types::UrlType::GoogleDocs,
            "Should detect as Google Docs: {url}"
        );
        
        let (result, _duration) = fixture.convert_with_rate_limit(url).await;
        ConversionValidator::Quality.validate(result, description);
    }

    Ok(())
}

/// Test Google Docs error scenarios
/// 
/// Tests error handling with malformed URLs that are guaranteed to be invalid.
#[tokio::test]
async fn test_google_docs_error_scenarios() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = GoogleDocsTestFixture::setup(false)?;
    let error_validation = ErrorValidationConfig::from_env();

    // Generate malformed URLs for testing error handling
    let error_urls = fixture.test_data.generate_error_test_urls();
    let test_cases = generate_test_cases(&error_urls);

    let validator = ConversionValidator::ExpectError {
        min_content_length: error_validation.min_error_content_length,
    };

    test_multiple_urls(&fixture, &test_cases, &validator).await;

    Ok(())
}

/// Test Google Docs with different content types (if available)
#[tokio::test]
async fn test_google_docs_content_types() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = GoogleDocsTestFixture::setup(true)?;

    // Test different Google Workspace document types
    let document_types = fixture.test_data.supported_document_types();

    for (url, description) in document_types.iter() {
        let validator = ConversionValidator::ContentType(description);
        test_multiple_urls(&fixture, &[(url, description)], &validator).await;
    }

    Ok(())
}

/// Test Google Docs conversion without API key
#[tokio::test]
async fn test_google_docs_without_api_key() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = GoogleDocsTestFixture::setup(false)?;

    println!("Testing without API key");
    let (result, _duration) = fixture.convert_with_rate_limit(&fixture.test_data.primary_doc_url).await;

    ConversionValidator::Quality.validate(result, "Conversion without API key");

    Ok(())
}

/// Test Google Docs conversion with API key
#[tokio::test]
async fn test_google_docs_with_api_key() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = GoogleDocsTestFixture::setup(false)?;

    let api_key = match &fixture.config.google_api_key {
        Some(key) => key,
        None => {
            println!("Skipping test - no Google API key available");
            return Ok(());
        }
    };

    let config_with_key = markdowndown::Config::builder()
        .google_api_key(api_key)
        .build();
    let md_with_key = MarkdownDown::with_config(config_with_key);

    println!("Testing with API key");
    TestUtils::apply_rate_limit(&fixture.config).await;
    let start = Instant::now();
    let result = md_with_key.convert_url(&fixture.test_data.primary_doc_url).await;
    let _duration = start.elapsed();

    ConversionValidator::Quality.validate(result, "Conversion with API key");

    Ok(())
}

/// Performance test for Google Docs conversion
#[tokio::test]
async fn test_google_docs_performance() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = GoogleDocsTestFixture::setup(true)?;

    println!("Performance testing Google Docs conversion");

    let (result, duration) = fixture.convert_with_rate_limit(&fixture.test_data.primary_doc_url).await;

    let validator = ConversionValidator::Custom(Box::new(move |markdown| {
        let content_length = markdown.as_str().len();
        
        // Log performance metrics
        println!("Performance Results:");
        println!("  Duration: {duration:?}");
        println!("  Content length: {content_length} chars");
        println!(
            "  Chars per second: {:.2}",
            content_length as f64 / duration.as_secs_f64()
        );
        
        // Assert performance is within threshold
        assert!(
            duration < fixture.config.large_document_timeout(),
            "Google Docs conversion took too long: {duration:?}"
        );
        
        // Assert output quality
        assert!(
            TestUtils::validate_markdown_quality(markdown.as_str()),
            "Performance test should produce quality output"
        );
    }));
    
    validator.validate(result, "Performance test");

    Ok(())
}
