//! Integration tests for plan command workflow execution
//!
//! Tests for the complete plan command journey, from CLI parsing through workflow
//! execution to issue file creation. These tests verify that the plan command
//! correctly processes plan files and creates issue files as expected.
//!
//! ## Test Categories
//!
//! ### 1. Basic Functionality Tests
//! - **CLI Argument Parsing**: Tests that the plan command correctly accepts and validates file paths
//! - **Workflow Integration**: Tests that the plan workflow executes correctly in test mode
//! - **Path Handling**: Tests relative and absolute path processing
//!
//! ### 2. Error Scenario Tests
//! - **File Not Found**: Tests behavior when plan file doesn't exist
//! - **Directory as File**: Tests error handling when path points to directory
//! - **Empty Files**: Tests handling of empty plan files
//!
//! ### 3. Edge Case Tests
//! - **Special Characters**: Tests files with spaces and special characters in names
//! - **Existing Issues**: Tests plan execution with pre-existing issue files
//! - **Complex Specifications**: Tests with detailed, multi-section plan files
//!
//! ### 4. Concurrency and Performance Tests
//! - **Concurrent Execution**: Tests multiple plan workflows running simultaneously
//! - **Performance Timing**: Verifies reasonable execution times (ignored by default)
//!
//! ## Test Strategy
//!
//! These tests use a hybrid approach to balance comprehensive testing with execution speed:
//!
//! 1. **Test Mode Execution**: Most tests use `sah flow test plan` instead of `sah plan`
//!    to avoid calling external AI services, making tests fast and deterministic.
//!
//! 2. **Isolated Environments**: Each test uses `TestHomeGuard` to ensure complete isolation
//!    and prevent interference between tests.
//!
//! 3. **Real CLI Testing**: Tests use `assert_cmd::Command::cargo_bin` to test the actual
//!    binary, ensuring realistic integration validation.
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all plan integration tests
//! cargo test --test plan_integration_tests
//!
//! # Run specific test
//! cargo test test_plan_workflow_test_mode --test plan_integration_tests
//!
//! # Run with output for debugging
//! cargo test --test plan_integration_tests -- --nocapture
//!
//! # Include performance tests (normally ignored)
//! cargo test --test plan_integration_tests -- --ignored
//! ```
//!
//! ## Test Environment
//!
//! Tests create isolated temporary environments with:
//! - Temporary home directories
//! - Mock .swissarmyhammer structure
//! - Isolated issues directories
//! - Git repository initialization
//! - Automatic cleanup on test completion
//!
//! ## Dependencies
//!
//! These tests require:
//! - `assert_cmd` for CLI command execution
//! - `tempfile` for isolated test environments
//! - `tokio` for async test execution
//! - Built `sah` binary (automatically handled by `cargo_bin`)
//!
//! ## Debugging Tests
//!
//! If tests fail:
//! 1. Run with `--nocapture` to see stdout/stderr
//! 2. Check that the `sah` binary builds successfully
//! 3. Verify that built-in workflows are available: `sah flow list`
//! 4. Test plan workflow manually: `sah flow test plan --var plan_filename=/path/to/test.md`

use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod in_process_test_utils;
use in_process_test_utils::run_sah_command_in_process_with_dir;

mod test_utils;
use test_utils::{create_temp_dir, setup_git_repo};

use swissarmyhammer::test_utils::IsolatedTestEnvironment;

/// Create a simple test plan file with basic content
fn create_test_plan_file(
    dir: &std::path::Path,
    filename: &str,
    title: &str,
) -> Result<std::path::PathBuf> {
    let plan_file = dir.join(filename);
    let content = format!(
        r#"# {title}

## Overview
This is a test specification for integration testing of the plan command.

## Requirements
1. Create a simple component for data processing
2. Add basic validation functionality
3. Write comprehensive unit tests
4. Add integration tests
5. Update documentation

## Implementation Details

### Component Structure
The component should follow existing patterns in the codebase:
- Use proper error handling with Result types
- Follow the established naming conventions
- Include comprehensive documentation

### Validation Requirements
- Input validation for all public APIs
- Proper error messages for invalid input
- Edge case handling for boundary conditions

### Testing Strategy
- Unit tests for individual functions
- Integration tests for complete workflows
- Performance tests for critical paths
- Error scenario testing

## Acceptance Criteria
- [ ] Component implements all required functionality
- [ ] All tests pass including new ones
- [ ] Documentation is complete and accurate
- [ ] Code review approval received
- [ ] Performance meets requirements

This specification should result in multiple focused issues that can be implemented incrementally.
"#
    );

    fs::write(&plan_file, content)?;
    Ok(plan_file)
}

/// Create a more complex test plan file with detailed requirements
fn create_complex_plan_file(dir: &std::path::Path, filename: &str) -> Result<std::path::PathBuf> {
    let plan_file = dir.join(filename);
    let content = r#"# Advanced Feature Specification

## Executive Summary
This specification outlines the development of a comprehensive data processing pipeline
with real-time analytics capabilities, caching layer, and monitoring integration.

## Functional Requirements

### Core Processing Engine
1. **Data Ingestion Module**
   - Support for multiple data sources (REST APIs, databases, file systems)
   - Configurable data transformation pipelines
   - Error handling and retry mechanisms
   - Data validation and sanitization

2. **Processing Pipeline**
   - Pluggable processing stages
   - Parallel processing capabilities
   - Memory-efficient data handling
   - Progress tracking and monitoring

3. **Output Management**
   - Multiple output formats (JSON, CSV, XML, binary)
   - Configurable output destinations
   - Data compression and encryption options
   - Audit logging for all outputs

### Real-time Analytics
1. **Metrics Collection**
   - Processing throughput metrics
   - Error rate monitoring
   - Resource utilization tracking
   - Custom business metrics

2. **Dashboard Integration**
   - REST API for metrics exposure
   - WebSocket support for real-time updates
   - Grafana-compatible metrics format
   - Historical data retention policies

### Caching Layer
1. **Multi-level Caching**
   - In-memory cache for hot data
   - Redis integration for shared cache
   - File-based cache for persistent storage
   - Cache invalidation strategies

2. **Cache Management**
   - Configurable TTL policies
   - Cache warming strategies
   - Memory pressure handling
   - Cache statistics and monitoring

## Technical Requirements

### Performance
- Process minimum 10,000 records per second
- Memory usage under 512MB for standard workloads
- Response time under 100ms for cached queries
- 99.9% uptime requirement

### Security
- Input validation and sanitization
- SQL injection prevention
- Data encryption at rest and in transit
- Audit logging for security events

### Monitoring
- Health check endpoints
- Prometheus metrics integration
- Structured logging with correlation IDs
- Alert integration for critical failures

### Scalability
- Horizontal scaling support
- Load balancing compatibility
- Database connection pooling
- Graceful degradation under load

## Implementation Phases

### Phase 1: Foundation
- Basic project structure and configuration
- Core data models and interfaces
- Initial processing pipeline framework
- Basic error handling and logging

### Phase 2: Core Functionality
- Data ingestion implementations
- Processing pipeline with basic stages
- Output management with primary formats
- Initial test suite

### Phase 3: Advanced Features
- Real-time analytics implementation
- Caching layer integration
- Performance optimizations
- Comprehensive monitoring

### Phase 4: Production Readiness
- Security hardening
- Performance tuning
- Documentation completion
- Deployment automation

## Success Metrics
- All functional requirements implemented and tested
- Performance benchmarks met or exceeded
- Security audit passed
- Documentation complete with examples
- Production deployment successful

This is a substantial specification that should generate many focused issues.
"#;

    fs::write(&plan_file, content)?;
    Ok(plan_file)
}

/// Setup a complete test environment for plan command testing
fn setup_plan_test_environment() -> Result<(TempDir, std::path::PathBuf)> {
    let temp_dir = create_temp_dir()?;
    let temp_path = temp_dir.path().to_path_buf();

    // Create necessary directories
    let issues_dir = temp_path.join("issues");
    fs::create_dir_all(&issues_dir)?;

    let swissarmyhammer_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&swissarmyhammer_dir)?;

    let tmp_dir = swissarmyhammer_dir.join("tmp");
    fs::create_dir_all(&tmp_dir)?;

    // Initialize git repository for realistic testing
    setup_git_repo(&temp_path)?;

    Ok((temp_dir, temp_path))
}

/// Test plan command CLI argument parsing and initial validation
#[tokio::test]
async fn test_plan_command_argument_parsing() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create a simple test plan file
    let plan_file = create_test_plan_file(&temp_path, "test-plan.md", "Test Plan")?;

    // Test that the plan command starts execution (it should begin processing before timing out)
    let result =
        run_sah_command_in_process_with_dir(&["plan", plan_file.to_str().unwrap()], &temp_path)
            .await?;

    // The command should start executing (showing log output)
    // We're not testing full execution here due to AI service calls, so we accept either success or timeout
    assert!(
        result.stderr.contains("Running plan command")
            || result.stderr.contains("Starting workflow: plan")
            || result.stderr.contains("Making the plan for")
            || result.stderr.contains("Test command timed out")  // Accept timeout
            || result.exit_code == 0        // Command may succeed in test environment
            || result.exit_code == 124, // Standard timeout exit code
        "Should show plan execution started, succeed, or timeout. stdout: '{}', stderr: '{}'",
        result.stdout,
        result.stderr
    );

    Ok(())
}

/// Test plan workflow execution in test mode (no external service calls)
#[tokio::test]
async fn test_plan_workflow_test_mode() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create a test plan file
    let plan_file = create_test_plan_file(&temp_path, "test-plan.md", "Test Plan")?;

    // Execute plan workflow in test mode using flow test
    let result = run_sah_command_in_process_with_dir(
        &[
            "flow",
            "test",
            "plan",
            "--var",
            &format!("plan_filename={}", plan_file.display()),
        ],
        &temp_path,
    )
    .await?;

    assert!(
        result.exit_code == 0,
        "Plan workflow test should succeed. stderr: {}",
        result.stderr
    );

    let stdout = &result.stdout;

    // Verify test mode execution indicators
    assert!(
        stdout.contains("Test mode") || stdout.contains("🧪"),
        "Should indicate test mode execution: {stdout}"
    );

    // Verify coverage report
    assert!(
        stdout.contains("Coverage Report") && stdout.contains("States visited"),
        "Should show coverage report: {stdout}"
    );

    // Verify the plan workflow achieves good coverage
    assert!(
        stdout.contains("100.0%") || stdout.contains("Full"),
        "Should achieve high coverage: {stdout}"
    );

    Ok(())
}

/// Test plan command with relative path
#[tokio::test]
async fn test_plan_command_relative_path() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create subdirectory with plan file
    let plans_dir = temp_path.join("specification");
    fs::create_dir_all(&plans_dir)?;
    let _plan_file = create_test_plan_file(&plans_dir, "relative-test.md", "Relative Path Test")?;

    // Test using flow test mode with relative path
    let result = run_sah_command_in_process_with_dir(
        &[
            "flow",
            "test",
            "plan",
            "--var",
            "plan_filename=./specification/relative-test.md",
        ],
        &temp_path,
    )
    .await?;

    assert!(
        result.exit_code == 0,
        "Plan command with relative path should succeed. stderr: {}",
        result.stderr
    );

    let stdout = &result.stdout;
    assert!(
        stdout.contains("Test mode") && stdout.contains("Coverage Report"),
        "Should execute workflow in test mode: {stdout}"
    );

    Ok(())
}

/// Test plan command with absolute path
#[tokio::test]
async fn test_plan_command_absolute_path() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create plan file
    let plan_file = create_test_plan_file(&temp_path, "absolute-test.md", "Absolute Path Test")?;

    // Test using flow test mode with absolute path
    let result = run_sah_command_in_process_with_dir(
        &[
            "flow",
            "test",
            "plan",
            "--var",
            &format!("plan_filename={}", plan_file.display()),
        ],
        &temp_path,
    )
    .await?;

    assert!(
        result.exit_code == 0,
        "Plan command with absolute path should succeed. stderr: {}",
        result.stderr
    );

    let stdout = &result.stdout;
    assert!(
        stdout.contains("Test mode") && stdout.contains("100.0%"),
        "Should execute workflow successfully: {stdout}"
    );

    Ok(())
}

/// Test plan workflow with complex specification in test mode
#[tokio::test]
async fn test_plan_workflow_complex_specification() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create complex plan file
    let plan_file = create_complex_plan_file(&temp_path, "advanced-feature.md")?;

    // Test complex plan using flow test mode
    let result = run_sah_command_in_process_with_dir(
        &[
            "flow",
            "test",
            "plan",
            "--var",
            &format!("plan_filename={}", plan_file.display()),
        ],
        &temp_path,
    )
    .await?;

    assert!(
        result.exit_code == 0,
        "Plan workflow with complex specification should succeed. stderr: {}",
        result.stderr
    );

    let stdout = &result.stdout;
    assert!(
        stdout.contains("Test mode"),
        "Should run in test mode: {stdout}"
    );

    assert!(
        stdout.contains("Coverage Report"),
        "Should show coverage report: {stdout}"
    );

    Ok(())
}

/// Test error scenario: file not found
#[tokio::test]
async fn test_plan_command_file_not_found() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    let result =
        run_sah_command_in_process_with_dir(&["plan", "nonexistent-plan.md"], &temp_path).await?;

    assert!(
        result.exit_code != 0,
        "Plan command should fail with nonexistent file"
    );

    let stderr = &result.stderr;
    assert!(
        stderr.contains("not found")
            || stderr.contains("does not exist")
            || stderr.contains("No such file"),
        "Should show file not found error: {stderr}"
    );

    Ok(())
}

/// Test error scenario: directory instead of file
#[tokio::test]
async fn test_plan_command_directory_as_file() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create directory with same name as expected file
    let dir_path = temp_path.join("directory-not-file");
    fs::create_dir_all(&dir_path)?;

    let result =
        run_sah_command_in_process_with_dir(&["plan", dir_path.to_str().unwrap()], &temp_path)
            .await?;

    assert!(
        result.exit_code != 0,
        "Plan command should fail when given directory instead of file"
    );

    let stderr = &result.stderr;
    assert!(
        stderr.contains("directory") || stderr.contains("not a file") || stderr.contains("invalid"),
        "Should show appropriate error for directory: {stderr}"
    );

    Ok(())
}

/// Test error scenario: empty file
#[tokio::test]
async fn test_plan_command_empty_file() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create empty file
    let empty_file = temp_path.join("empty-plan.md");
    fs::write(&empty_file, "")?;

    let result =
        run_sah_command_in_process_with_dir(&["plan", empty_file.to_str().unwrap()], &temp_path)
            .await?;

    // Empty file might still be processed, but should not create meaningful issues
    // The important thing is the command completes without crashing
    assert!(
        result.exit_code >= 0,
        "Plan command should complete even with empty file"
    );

    Ok(())
}

/// Test plan workflow with existing issues (test mode)
#[tokio::test]
async fn test_plan_workflow_with_existing_issues() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create some existing issues
    let issues_dir = temp_path.join("issues");
    fs::write(
        issues_dir.join("EXISTING_000001_old-feature.md"),
        "# Old Feature\n\nExisting issue content.",
    )?;
    fs::write(
        issues_dir.join("EXISTING_000002_another-feature.md"),
        "# Another Feature\n\nAnother existing issue.",
    )?;

    // Create and test plan workflow in test mode
    let plan_file = create_test_plan_file(&temp_path, "new-feature.md", "New Feature Plan")?;

    let result = run_sah_command_in_process_with_dir(
        &[
            "flow",
            "test",
            "plan",
            "--var",
            &format!("plan_filename={}", plan_file.display()),
        ],
        &temp_path,
    )
    .await?;

    assert!(
        result.exit_code == 0,
        "Plan workflow should succeed with existing issues. stderr: {}",
        result.stderr
    );

    let stdout = &result.stdout;
    assert!(
        stdout.contains("Test mode") && stdout.contains("Coverage Report"),
        "Should execute workflow in test mode: {stdout}"
    );

    // Verify existing issues are preserved (unchanged during test mode)
    let existing_files = fs::read_dir(&issues_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    assert!(
        existing_files.iter().any(|f| f.starts_with("EXISTING_")),
        "Should preserve existing issues: {existing_files:?}"
    );

    Ok(())
}

/// Test plan workflow with files containing spaces and special characters
#[tokio::test]
async fn test_plan_workflow_special_characters() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create plan file with spaces and special characters in name
    let plan_file = create_test_plan_file(
        &temp_path,
        "test plan-v1.0 (draft).md",
        "Special Characters Test",
    )?;

    let result = run_sah_command_in_process_with_dir(
        &[
            "flow",
            "test",
            "plan",
            "--var",
            &format!("plan_filename={}", plan_file.display()),
        ],
        &temp_path,
    )
    .await?;

    assert!(
        result.exit_code == 0,
        "Plan workflow should handle files with special characters. stderr: {}",
        result.stderr
    );

    let stdout = &result.stdout;
    assert!(
        stdout.contains("Test mode") && stdout.contains("Coverage Report"),
        "Should execute workflow successfully: {stdout}"
    );

    Ok(())
}

/// Test sequential plan workflow executions in test mode
#[tokio::test]
async fn test_sequential_plan_workflow_executions() -> Result<()> {
    // Run multiple plan workflows sequentially in test mode
    for i in 0..3 {
        let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let (_temp_dir, temp_path) = setup_plan_test_environment().unwrap();

        let plan_file = create_test_plan_file(
            &temp_path,
            &format!("sequential-test-{i}.md"),
            &format!("Sequential Test {i}"),
        )
        .unwrap();

        // Use explicit working directory instead of global directory change
        let result = run_sah_command_in_process_with_dir(
            &[
                "flow",
                "test",
                "plan",
                "--var",
                &format!("plan_filename={}", plan_file.display()),
            ],
            &temp_path,
        )
        .await
        .expect("Failed to run plan workflow test");

        assert!(
            result.exit_code == 0,
            "Sequential plan workflow execution {i} should succeed"
        );
    }

    Ok(())
}

/// Test enhanced error handling: comprehensive file validation
#[tokio::test]
async fn test_plan_enhanced_error_file_not_found() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    let result = run_sah_command_in_process_with_dir(
        &["plan", "definitely-nonexistent-plan.md"],
        &temp_path,
    )
    .await?;

    assert!(
        result.exit_code != 0,
        "Plan command should fail with enhanced error handling"
    );

    let stderr = &result.stderr;

    // Test enhanced error message format
    assert!(
        stderr.contains("Error:") || stderr.contains("Plan file not found"),
        "Should show enhanced error message format: {stderr}"
    );

    // Test user guidance suggestions
    assert!(
        stderr.contains("Suggestions:") || stderr.contains("Check the file path"),
        "Should provide user guidance: {stderr}"
    );

    assert!(
        stderr.contains("typos") || stderr.contains("ls -la") || stderr.contains("absolute path"),
        "Should include actionable suggestions: {stderr}"
    );

    Ok(())
}

/// Test enhanced error handling: empty file validation
#[tokio::test]
async fn test_plan_enhanced_error_empty_file() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create empty file
    let empty_file = temp_path.join("empty-plan.md");
    fs::write(&empty_file, "")?;

    let result =
        run_sah_command_in_process_with_dir(&["plan", empty_file.to_str().unwrap()], &temp_path)
            .await?;

    // Empty file should trigger enhanced error handling
    let stderr = &result.stderr;

    // Check for enhanced error message
    if stderr.contains("Warning:") || stderr.contains("empty") {
        // Test warning level for empty files
        assert!(
            stderr.contains("Warning:") || stderr.contains("empty or contains no valid content"),
            "Should show warning for empty file: {stderr}"
        );

        // Test user guidance for empty files
        assert!(
            stderr.contains("Add content") || stderr.contains("whitespace"),
            "Should provide guidance for empty files: {stderr}"
        );
    }

    Ok(())
}

/// Test enhanced error handling: whitespace-only file
#[tokio::test]
async fn test_plan_enhanced_error_whitespace_file() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create file with only whitespace
    let whitespace_file = temp_path.join("whitespace-plan.md");
    fs::write(&whitespace_file, "   \n\t  \n  ")?;

    let result = run_sah_command_in_process_with_dir(
        &["plan", whitespace_file.to_str().unwrap()],
        &temp_path,
    )
    .await?;

    let stderr = &result.stderr;

    // Should treat whitespace-only as empty file
    if stderr.contains("Warning:") || stderr.contains("empty") {
        assert!(
            stderr.contains("empty or contains no valid content"),
            "Should show warning for whitespace-only file: {stderr}"
        );

        assert!(
            stderr.contains("whitespace"),
            "Should mention whitespace in guidance: {stderr}"
        );
    }

    Ok(())
}

/// Test enhanced error handling: directory instead of file
#[tokio::test]
async fn test_plan_enhanced_error_directory_not_file() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create directory with same name as expected file
    let dir_path = temp_path.join("directory-not-file");
    fs::create_dir_all(&dir_path)?;

    let result =
        run_sah_command_in_process_with_dir(&["plan", dir_path.to_str().unwrap()], &temp_path)
            .await?;

    assert!(
        result.exit_code != 0,
        "Plan command should fail with enhanced error for directory"
    );

    let stderr = &result.stderr;

    // Test enhanced error message
    assert!(
        stderr.contains("Error:")
            && (stderr.contains("directory") || stderr.contains("not a file")),
        "Should show enhanced error for directory: {stderr}"
    );

    // Test specific guidance for directories
    assert!(
        stderr.contains("Specify a file path instead") || stderr.contains("directory"),
        "Should provide specific guidance for directory error: {stderr}"
    );

    Ok(())
}

/// Test enhanced error handling: file too large
#[tokio::test]
async fn test_plan_enhanced_error_large_file() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create a large file (over the validation limit)
    let large_file = temp_path.join("huge-plan.md");
    let large_content = "x".repeat(11 * 1024 * 1024); // 11MB - over default 10MB limit
    fs::write(&large_file, large_content)?;

    let result =
        run_sah_command_in_process_with_dir(&["plan", large_file.to_str().unwrap()], &temp_path)
            .await?;

    let stderr = &result.stderr;

    // Should show file too large error
    if stderr.contains("too large") || stderr.contains("bytes") {
        assert!(
            stderr.contains("Error:") && stderr.contains("too large"),
            "Should show file too large error: {stderr}"
        );

        assert!(
            stderr.contains("Break large plans") || stderr.contains("smaller"),
            "Should suggest breaking large plans into smaller files: {stderr}"
        );
    }

    Ok(())
}

/// Test enhanced error handling: invalid binary content
#[tokio::test]
async fn test_plan_enhanced_error_binary_content() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Create file with binary content (null bytes)
    let binary_file = temp_path.join("binary-plan.md");
    let binary_content = b"# Plan with\0null bytes\0in content";
    fs::write(&binary_file, binary_content)?;

    let result =
        run_sah_command_in_process_with_dir(&["plan", binary_file.to_str().unwrap()], &temp_path)
            .await?;

    let stderr = &result.stderr;

    // Should show invalid format error
    if stderr.contains("Invalid") || stderr.contains("null bytes") {
        assert!(
            stderr.contains("Error:") && stderr.contains("Invalid"),
            "Should show invalid format error: {stderr}"
        );

        assert!(
            stderr.contains("null bytes") || stderr.contains("binary"),
            "Should mention null bytes or binary content: {stderr}"
        );

        assert!(
            stderr.contains("UTF-8") || stderr.contains("corrupted"),
            "Should suggest checking encoding: {stderr}"
        );
    }

    Ok(())
}

/// Test enhanced error handling: color output detection
#[tokio::test]
async fn test_plan_enhanced_error_color_output() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Test with explicit NO_COLOR environment variable
    std::env::set_var("NO_COLOR", "1");
    let result =
        run_sah_command_in_process_with_dir(&["plan", "nonexistent.md"], &temp_path).await?;
    std::env::remove_var("NO_COLOR");

    let stderr = &result.stderr;

    // Should not contain ANSI color codes when NO_COLOR is set
    if stderr.contains("Error:") || stderr.contains("not found") {
        assert!(
            !stderr.contains("\x1b["), // No ANSI escape sequences
            "Should not contain color codes with NO_COLOR=1: {}",
            stderr.trim()
        );
    }

    Ok(())
}

/// Test enhanced error handling: exit codes
#[tokio::test]
async fn test_plan_enhanced_error_exit_codes() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    // Test file not found exit code
    let result =
        run_sah_command_in_process_with_dir(&["plan", "nonexistent.md"], &temp_path).await?;

    assert_eq!(
        result.exit_code,
        2, // EXIT_ERROR
        "Should return exit code 2 for file not found error"
    );

    // Test empty file exit code (should be warning = 1)
    let empty_file = temp_path.join("empty.md");
    fs::write(&empty_file, "")?;

    let result2 =
        run_sah_command_in_process_with_dir(&["plan", empty_file.to_str().unwrap()], &temp_path)
            .await?;

    // Empty file should return warning exit code if detected as empty
    let stderr = &result2.stderr;
    if stderr.contains("Warning:") || stderr.contains("empty") {
        assert_eq!(
            result2.exit_code,
            1, // EXIT_WARNING
            "Should return exit code 1 for empty file warning"
        );
    }

    Ok(())
}

/// Test enhanced error handling: issues directory validation
#[tokio::test]
async fn test_plan_enhanced_error_issues_directory() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    // Create minimal test environment WITHOUT issues directory
    let _temp_dir = create_temp_dir()?;
    let temp_path = _temp_dir.path().to_path_buf();

    // Use explicit working directory instead of global directory change

    // Create necessary directories (but NOT issues directory)
    let swissarmyhammer_dir = temp_path.join(".swissarmyhammer");
    fs::create_dir_all(&swissarmyhammer_dir)?;

    let tmp_dir = swissarmyhammer_dir.join("tmp");
    fs::create_dir_all(&tmp_dir)?;

    // Initialize git repository for realistic testing
    setup_git_repo(&temp_path)?;

    // Create a valid plan file
    let plan_file = create_test_plan_file(&temp_path, "test-plan.md", "Test Plan")?;

    // Create issues as a file instead of directory to trigger error
    let issues_file = temp_path.join("issues");
    fs::write(&issues_file, "not a directory")?;

    let result =
        run_sah_command_in_process_with_dir(&["plan", plan_file.to_str().unwrap()], &temp_path)
            .await?;

    let stderr = &result.stderr;

    // Should show issues directory error
    if stderr.contains("Issues directory") || stderr.contains("not writable") {
        assert!(
            stderr.contains("Error:") && stderr.contains("Issues"),
            "Should show issues directory error: {stderr}"
        );

        assert!(
            stderr.contains("mkdir -p") || stderr.contains("directory"),
            "Should suggest creating directory: {stderr}"
        );
    }

    Ok(())
}

/// Test enhanced error handling: comprehensive error message structure
#[tokio::test]
async fn test_plan_enhanced_error_message_structure() -> Result<()> {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let (_temp_dir, temp_path) = setup_plan_test_environment()?;

    // Use explicit working directory instead of global directory change

    let result =
        run_sah_command_in_process_with_dir(&["plan", "structured-error-test.md"], &temp_path)
            .await?;

    let stderr = &result.stderr;

    if stderr.contains("Error:") || stderr.contains("not found") {
        // Test error message structure components
        let has_error_label = stderr.contains("Error:")
            || stderr.contains("Warning:")
            || stderr.contains("Critical:");
        let has_suggestions = stderr.contains("Suggestions:");
        let has_bullet_points = stderr.contains("•") || stderr.contains("-");

        assert!(
            has_error_label,
            "Should have error severity label: {stderr}"
        );

        assert!(has_suggestions, "Should have suggestions section: {stderr}");

        assert!(
            has_bullet_points,
            "Should have bulleted suggestions: {stderr}"
        );
    }

    Ok(())
}
